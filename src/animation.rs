//! Safe decoding and evaluation of the HSD animation subset used by `MnMaAll.dat`.
//!
//! Descriptor integers and floats use the GameCube's big-endian byte order. FObj
//! payload scalars use the little-endian encoding implemented by Melee's pinned
//! `sysdolphin/baselib/fobj.c`. Every [`DataOffset`] in this module is relative to
//! the beginning of an HSD DAT data section, not the beginning of the DAT file.

use std::collections::HashSet;

use thiserror::Error;

use crate::spline;

const AOBJ_DESC_SIZE: usize = 0x10;
const FOBJ_DESC_SIZE: usize = 0x14;

const AOBJ_NO_UPDATE: u32 = 1 << 28;
const AOBJ_LOOP: u32 = 1 << 29;

/// Resource ceilings for decoding one AObj descriptor graph.
///
/// The defaults are intentionally far above the pinned menu archives while
/// preventing a modded DAT from amplifying a small descriptor graph into an
/// unbounded number of owned keyframes. Callers that accept other authored
/// formats may supply tighter limits with [`HsdDataSection::with_limits`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DecodeLimits {
    pub max_tracks: usize,
    pub max_keys_per_track: usize,
    pub max_total_keys: usize,
}

impl Default for DecodeLimits {
    fn default() -> Self {
        Self {
            max_tracks: 16_384,
            max_keys_per_track: 1_048_576,
            max_total_keys: 2_097_152,
        }
    }
}

/// A 32-bit pointer value relative to the beginning of an HSD DAT data section.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct DataOffset(u32);

impl DataOffset {
    pub const fn new(value: u32) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u32 {
        self.0
    }
}

/// The object-update callback that receives an FObj's channel value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChannelTarget {
    Joint,
    Material,
    Texture,
}

/// A channel present in the currently modeled `MnMaAll.dat` animation slices.
///
/// This includes every scalar channel needed by the background and panel roots,
/// plus the Main-selection `ConTop` subtree. Cursor texture scale and its other
/// color-register channels remain explicit decoding errors until their consumer
/// state is modeled.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Channel {
    JointRotationX,
    JointRotationY,
    JointRotationZ,
    JointTranslationX,
    JointTranslationY,
    JointTranslationZ,
    JointScaleX,
    JointScaleY,
    JointScaleZ,
    JointBranchVisibility,
    MaterialDiffuseR,
    MaterialDiffuseG,
    MaterialDiffuseB,
    MaterialAlpha,
    TextureImage,
    TextureTranslationU,
    TextureTranslationV,
    TextureBlend,
    TextureKonstAlpha,
    TextureTev0Alpha,
}

impl Channel {
    fn decode(target: ChannelTarget, raw: u8) -> Option<Self> {
        match (target, raw) {
            (ChannelTarget::Joint, 1) => Some(Self::JointRotationX),
            (ChannelTarget::Joint, 2) => Some(Self::JointRotationY),
            (ChannelTarget::Joint, 3) => Some(Self::JointRotationZ),
            (ChannelTarget::Joint, 5) => Some(Self::JointTranslationX),
            (ChannelTarget::Joint, 6) => Some(Self::JointTranslationY),
            (ChannelTarget::Joint, 7) => Some(Self::JointTranslationZ),
            (ChannelTarget::Joint, 8) => Some(Self::JointScaleX),
            (ChannelTarget::Joint, 9) => Some(Self::JointScaleY),
            (ChannelTarget::Joint, 10) => Some(Self::JointScaleZ),
            (ChannelTarget::Joint, 12) => Some(Self::JointBranchVisibility),
            (ChannelTarget::Material, 4) => Some(Self::MaterialDiffuseR),
            (ChannelTarget::Material, 5) => Some(Self::MaterialDiffuseG),
            (ChannelTarget::Material, 6) => Some(Self::MaterialDiffuseB),
            (ChannelTarget::Material, 10) => Some(Self::MaterialAlpha),
            (ChannelTarget::Texture, 1) => Some(Self::TextureImage),
            (ChannelTarget::Texture, 2) => Some(Self::TextureTranslationU),
            (ChannelTarget::Texture, 3) => Some(Self::TextureTranslationV),
            (ChannelTarget::Texture, 9) => Some(Self::TextureBlend),
            (ChannelTarget::Texture, 15) => Some(Self::TextureKonstAlpha),
            (ChannelTarget::Texture, 19) => Some(Self::TextureTev0Alpha),
            _ => None,
        }
    }
}

/// The outgoing interpolation operation attached to an HSD FObj key.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Interpolation {
    Constant,
    Linear,
    /// Cubic Hermite interpolation whose encoded tangent is zero.
    SplineZero,
    /// Cubic Hermite interpolation with an encoded tangent.
    Spline,
}

impl Interpolation {
    fn decode(raw: u8) -> Option<Self> {
        match raw {
            1 => Some(Self::Constant),
            2 => Some(Self::Linear),
            3 => Some(Self::SplineZero),
            4 => Some(Self::Spline),
            _ => None,
        }
    }
}

/// The scalar packing selected by an HSD FObj descriptor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScalarEncoding {
    Float32,
    Signed16 { fractional_bits: u8 },
    Unsigned16 { fractional_bits: u8 },
    Signed8 { fractional_bits: u8 },
    Unsigned8 { fractional_bits: u8 },
}

impl ScalarEncoding {
    fn decode(raw: u8) -> Option<Self> {
        let fractional_bits = raw & 0x1f;
        if fractional_bits == 31 {
            return None;
        }
        match raw & 0xe0 {
            0x00 if raw == 0 => Some(Self::Float32),
            0x20 => Some(Self::Signed16 { fractional_bits }),
            0x40 => Some(Self::Unsigned16 { fractional_bits }),
            0x60 => Some(Self::Signed8 { fractional_bits }),
            0x80 => Some(Self::Unsigned8 { fractional_bits }),
            _ => None,
        }
    }

    fn read(self, cursor: &mut StreamCursor<'_>) -> Result<f32, DecodeError> {
        let value = match self {
            Self::Float32 => f32::from_le_bytes(cursor.array()?),
            Self::Signed16 { fractional_bits } => {
                f32::from(i16::from_le_bytes(cursor.array()?)) / denominator(fractional_bits)
            }
            Self::Unsigned16 { fractional_bits } => {
                f32::from(u16::from_le_bytes(cursor.array()?)) / denominator(fractional_bits)
            }
            Self::Signed8 { fractional_bits } => {
                f32::from(i8::from_le_bytes(cursor.array()?)) / denominator(fractional_bits)
            }
            Self::Unsigned8 { fractional_bits } => {
                f32::from(u8::from_le_bytes(cursor.array()?)) / denominator(fractional_bits)
            }
        };
        if !value.is_finite() {
            return Err(DecodeError::NonFinitePayload {
                stream_offset: cursor.stream_offset.get(),
            });
        }
        Ok(value)
    }
}

fn denominator(fractional_bits: u8) -> f32 {
    (1_u32 << fractional_bits) as f32
}

/// One decoded key. `wait` is the integral duration before the following key.
/// The final key carries the observed zero-wait stream terminator.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Keyframe {
    pub value: f32,
    pub slope: f32,
    pub interpolation: Interpolation,
    pub wait: u16,
}

/// A decoded HSD FObj descriptor and its packed animation program.
#[derive(Clone, Debug, PartialEq)]
pub struct FObjTrack {
    pub descriptor_offset: DataOffset,
    pub data_offset: DataOffset,
    pub data_length: u32,
    pub start_frame: f32,
    pub channel: Channel,
    pub value_encoding: ScalarEncoding,
    pub slope_encoding: ScalarEncoding,
    pub keys: Vec<Keyframe>,
}

impl FObjTrack {
    /// Evaluate the value delivered to HSD's object-update callback.
    ///
    /// `requested_frame` has the same coordinate as `HSD_FObjReqAnim`: the
    /// descriptor's `startframe` is added before interpreting the key stream.
    /// A negative resulting time emits no update. The observed zero-wait
    /// terminator makes the source interpreter hold the final key thereafter.
    pub fn sample(&self, requested_frame: f32) -> Result<Option<f32>, EvaluationError> {
        if !requested_frame.is_finite() {
            return Err(EvaluationError::NonFiniteFrame);
        }
        // HSD_FObj stores the descriptor's f32 startframe in an s16 field.
        let time = requested_frame + f32::from(self.start_frame as i16);
        if !time.is_finite() {
            return Err(EvaluationError::NonFiniteFrame);
        }
        if time < 0.0 || self.keys.len() < 2 {
            return Ok(None);
        }

        let mut segment_start = 0.0_f32;
        for index in 0..self.keys.len() - 1 {
            let duration = self.keys[index].wait;
            let segment_end = segment_start + f32::from(duration);
            if time < segment_end {
                return Ok(Some(evaluate_segment(
                    self.keys[index],
                    self.keys[index + 1],
                    time - segment_start,
                    duration,
                )));
            }
            segment_start = segment_end;
        }
        Ok(Some(
            self.keys.last().expect("track has at least two keys").value,
        ))
    }
}

/// An HSD AObj descriptor plus its linked FObj descriptors.
#[derive(Clone, Debug, PartialEq)]
pub struct AObjAnimation {
    pub descriptor_offset: DataOffset,
    pub flags: u32,
    pub end_frame: f32,
    pub object_id: u32,
    pub tracks: Vec<FObjTrack>,
}

/// One FObj omitted from a partially decoded AObj because its behavior is not
/// modeled yet.
///
/// The descriptor identity is retained separately because some decode errors,
/// notably an unsupported stream opcode, identify the program byte rather than
/// the FObj that owns it.
#[derive(Debug, PartialEq, Eq)]
pub struct SkippedFObjTrack {
    pub descriptor_offset: DataOffset,
    pub error: DecodeError,
}

/// A decoded AObj whose supported FObjs remain usable independently.
#[derive(Debug, PartialEq)]
pub struct AObjDecodeReport {
    pub animation: AObjAnimation,
    pub skipped_tracks: Vec<SkippedFObjTrack>,
}

/// One channel update produced by [`AObjAnimation::sample_requested_frame`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ChannelValue {
    pub channel: Channel,
    pub value: f32,
}

impl AObjAnimation {
    /// Reproduce the first update after `HSD_AObjReqAnim(aobj, frame)`.
    ///
    /// The source request clears `AOBJ_NO_ANIM`; loop wrapping and
    /// `AOBJ_NO_UPDATE` still apply. The AObj's end frame is deliberately not a
    /// generic curve clamp: a non-looping AObj evaluates once before it stops.
    pub fn sample_requested_frame(&self, frame: f32) -> Result<Vec<ChannelValue>, EvaluationError> {
        if !frame.is_finite() {
            return Err(EvaluationError::NonFiniteFrame);
        }
        if self.flags & AOBJ_NO_UPDATE != 0 {
            return Ok(Vec::new());
        }

        let mut current_frame = frame;
        if self.flags & AOBJ_LOOP != 0 && self.end_frame <= current_frame {
            current_frame = if 0.0 < self.end_frame {
                current_frame % self.end_frame
            } else {
                self.end_frame
            };
        }

        self.tracks
            .iter()
            .filter_map(|track| match track.sample(current_frame) {
                Ok(Some(value)) => Some(Ok(ChannelValue {
                    channel: track.channel,
                    value,
                })),
                Ok(None) => None,
                Err(error) => Some(Err(error)),
            })
            .collect()
    }
}

/// A validated view of bytes beginning at offset zero of an HSD DAT data section.
#[derive(Clone, Copy, Debug)]
pub struct HsdDataSection<'a> {
    bytes: &'a [u8],
    limits: DecodeLimits,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum UnsupportedTrackPolicy {
    Reject,
    Skip,
}

impl<'a> HsdDataSection<'a> {
    pub fn new(bytes: &'a [u8]) -> Result<Self, DecodeError> {
        Self::with_limits(bytes, DecodeLimits::default())
    }

    /// Build a data-section view with explicit per-AObj decoding ceilings.
    pub fn with_limits(bytes: &'a [u8], limits: DecodeLimits) -> Result<Self, DecodeError> {
        if bytes.len() > u32::MAX as usize {
            return Err(DecodeError::DataSectionTooLarge {
                length: bytes.len(),
            });
        }
        Ok(Self { bytes, limits })
    }

    /// Decode one AObj and its linked FObj list from data-section-relative offsets.
    pub fn decode_aobj(
        &self,
        descriptor_offset: DataOffset,
        target: ChannelTarget,
    ) -> Result<AObjAnimation, DecodeError> {
        self.decode_aobj_with_policy(descriptor_offset, target, UnsupportedTrackPolicy::Reject)
            .map(|report| report.animation)
    }

    /// Decode every supported FObj in one AObj while retaining precise
    /// diagnostics for independently unsupported tracks.
    ///
    /// Structural corruption remains fatal. Only capability gaps that are
    /// local to one FObj (channel, scalar encoding, or program opcode) are
    /// skipped, so one future channel cannot suppress already modeled motion
    /// on the same source object.
    pub fn decode_aobj_supported(
        &self,
        descriptor_offset: DataOffset,
        target: ChannelTarget,
    ) -> Result<AObjDecodeReport, DecodeError> {
        self.decode_aobj_with_policy(descriptor_offset, target, UnsupportedTrackPolicy::Skip)
    }

    fn decode_aobj_with_policy(
        &self,
        descriptor_offset: DataOffset,
        target: ChannelTarget,
        unsupported: UnsupportedTrackPolicy,
    ) -> Result<AObjDecodeReport, DecodeError> {
        self.require_aligned(descriptor_offset, "AObj descriptor")?;
        let descriptor = self.range(descriptor_offset, AOBJ_DESC_SIZE, "AObj descriptor")?;
        let flags = be_u32(descriptor, 0);
        let end_frame =
            finite_descriptor_float(be_f32(descriptor, 4), descriptor_offset, "AObj end_frame")?;
        let first_fobj = pointer(be_u32(descriptor, 8));
        let object_id = be_u32(descriptor, 12);

        let descriptor_capacity = self.limits.max_tracks.min(self.bytes.len().div_ceil(4));
        let mut tracks = Vec::new();
        tracks
            .try_reserve(descriptor_capacity)
            .map_err(|_| DecodeError::AllocationFailed { resource: "tracks" })?;
        let mut skipped_tracks = Vec::new();
        let mut seen = HashSet::new();
        seen.try_reserve(descriptor_capacity)
            .map_err(|_| DecodeError::AllocationFailed {
                resource: "visited FObj descriptors",
            })?;
        let mut total_keys = 0_usize;
        let mut current = first_fobj;
        while let Some(fobj_offset) = current {
            if seen.len() >= self.limits.max_tracks {
                return Err(DecodeError::LimitExceeded {
                    resource: "FObj tracks per AObj",
                    limit: self.limits.max_tracks,
                });
            }
            if !seen.insert(fobj_offset) {
                return Err(DecodeError::FObjCycle {
                    descriptor_offset: fobj_offset.get(),
                });
            }

            // Read the link independently so an unsupported track cannot hide
            // the supported descriptors that follow it.
            self.require_aligned(fobj_offset, "FObj descriptor")?;
            let descriptor = self.range(fobj_offset, FOBJ_DESC_SIZE, "FObj descriptor")?;
            let next = pointer(be_u32(descriptor, 0));
            let remaining_total_keys = self.limits.max_total_keys.saturating_sub(total_keys);
            match self.decode_fobj(fobj_offset, target, remaining_total_keys) {
                Ok((decoded_next, track)) => {
                    debug_assert_eq!(decoded_next, next);
                    total_keys = total_keys
                        .checked_add(track.keys.len())
                        .expect("decoded key total is bounded by usize limits");
                    tracks.push(track);
                }
                Err(error)
                    if unsupported == UnsupportedTrackPolicy::Skip
                        && error.is_unsupported_track() =>
                {
                    skipped_tracks.push(SkippedFObjTrack {
                        descriptor_offset: fobj_offset,
                        error,
                    });
                }
                Err(error) => return Err(error),
            }
            current = next;
        }

        Ok(AObjDecodeReport {
            animation: AObjAnimation {
                descriptor_offset,
                flags,
                end_frame,
                object_id,
                tracks,
            },
            skipped_tracks,
        })
    }

    fn decode_fobj(
        &self,
        descriptor_offset: DataOffset,
        target: ChannelTarget,
        remaining_total_keys: usize,
    ) -> Result<(Option<DataOffset>, FObjTrack), DecodeError> {
        self.require_aligned(descriptor_offset, "FObj descriptor")?;
        let descriptor = self.range(descriptor_offset, FOBJ_DESC_SIZE, "FObj descriptor")?;
        let next = pointer(be_u32(descriptor, 0));
        let data_length = be_u32(descriptor, 4);
        let start_frame =
            finite_descriptor_float(be_f32(descriptor, 8), descriptor_offset, "FObj start_frame")?;
        let truncated_start_frame = start_frame.trunc();
        if !(f32::from(i16::MIN)..=f32::from(i16::MAX)).contains(&truncated_start_frame) {
            return Err(DecodeError::StartFrameOutOfRange {
                descriptor_offset: descriptor_offset.get(),
            });
        }
        let raw_channel = descriptor[12];
        let channel =
            Channel::decode(target, raw_channel).ok_or(DecodeError::UnsupportedChannel {
                target,
                channel: raw_channel,
                descriptor_offset: descriptor_offset.get(),
            })?;
        let value_encoding = ScalarEncoding::decode(descriptor[13]).ok_or(
            DecodeError::UnsupportedFractionEncoding {
                encoding: descriptor[13],
                descriptor_offset: descriptor_offset.get(),
                field: "value",
            },
        )?;
        let slope_encoding = ScalarEncoding::decode(descriptor[14]).ok_or(
            DecodeError::UnsupportedFractionEncoding {
                encoding: descriptor[14],
                descriptor_offset: descriptor_offset.get(),
                field: "slope",
            },
        )?;
        let data_offset = DataOffset::new(be_u32(descriptor, 16));
        if data_length == 0 {
            return Err(DecodeError::EmptyFObjProgram {
                descriptor_offset: descriptor_offset.get(),
            });
        }
        if data_offset.get() == 0 {
            return Err(DecodeError::NullOffset {
                context: "FObj animation program",
            });
        }
        let program = self.range(data_offset, data_length as usize, "FObj animation program")?;
        let keys = decode_program(
            program,
            data_offset,
            value_encoding,
            slope_encoding,
            self.limits.max_keys_per_track,
            remaining_total_keys,
        )?;

        Ok((
            next,
            FObjTrack {
                descriptor_offset,
                data_offset,
                data_length,
                start_frame,
                channel,
                value_encoding,
                slope_encoding,
                keys,
            },
        ))
    }

    fn require_aligned(
        &self,
        offset: DataOffset,
        context: &'static str,
    ) -> Result<(), DecodeError> {
        if !offset.get().is_multiple_of(4) {
            return Err(DecodeError::MisalignedDescriptor {
                offset: offset.get(),
                context,
            });
        }
        Ok(())
    }

    fn range(
        &self,
        offset: DataOffset,
        length: usize,
        context: &'static str,
    ) -> Result<&'a [u8], DecodeError> {
        let start = offset.get() as usize;
        let end = start
            .checked_add(length)
            .filter(|&end| end <= self.bytes.len())
            .ok_or(DecodeError::OutOfBounds {
                offset: offset.get(),
                length,
                data_length: self.bytes.len(),
                context,
            })?;
        Ok(&self.bytes[start..end])
    }
}

fn pointer(raw: u32) -> Option<DataOffset> {
    (raw != 0).then(|| DataOffset::new(raw))
}

fn finite_descriptor_float(
    value: f32,
    offset: DataOffset,
    field: &'static str,
) -> Result<f32, DecodeError> {
    if !value.is_finite() {
        return Err(DecodeError::NonFiniteDescriptor {
            descriptor_offset: offset.get(),
            field,
        });
    }
    Ok(value)
}

fn be_u32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_be_bytes(
        bytes[offset..offset + 4]
            .try_into()
            .expect("validated descriptor"),
    )
}

fn be_f32(bytes: &[u8], offset: usize) -> f32 {
    f32::from_bits(be_u32(bytes, offset))
}

fn decode_program(
    program: &[u8],
    stream_offset: DataOffset,
    value_encoding: ScalarEncoding,
    slope_encoding: ScalarEncoding,
    max_keys_per_track: usize,
    remaining_total_keys: usize,
) -> Result<Vec<Keyframe>, DecodeError> {
    let mut cursor = StreamCursor::new(program, stream_offset);
    let mut keys = Vec::new();
    let key_capacity = program
        .len()
        .div_ceil(2)
        .min(max_keys_per_track)
        .min(remaining_total_keys);
    keys.try_reserve(key_capacity)
        .map_err(|_| DecodeError::AllocationFailed {
            resource: "FObj keyframes",
        })?;
    let mut packed_remaining = 0_u16;
    let mut interpolation = Interpolation::Constant;

    while !cursor.is_empty() {
        if keys.len() >= max_keys_per_track {
            return Err(DecodeError::LimitExceeded {
                resource: "keyframes per FObj track",
                limit: max_keys_per_track,
            });
        }
        if keys.len() >= remaining_total_keys {
            return Err(DecodeError::LimitExceeded {
                resource: "keyframes per AObj",
                limit: remaining_total_keys,
            });
        }
        if packed_remaining == 0 {
            let header = cursor.byte()?;
            interpolation =
                Interpolation::decode(header & 0x0f).ok_or(DecodeError::UnsupportedOpcode {
                    opcode: header & 0x0f,
                    stream_offset: cursor.absolute_position().saturating_sub(1),
                })?;
            packed_remaining = parse_pack_info(&mut cursor, header)?;
        }

        let value = value_encoding.read(&mut cursor)?;
        let slope = if interpolation == Interpolation::Spline {
            slope_encoding.read(&mut cursor)?
        } else {
            0.0
        };
        packed_remaining -= 1;

        if cursor.is_empty() {
            return Err(DecodeError::MissingWait {
                stream_offset: stream_offset.get(),
            });
        }
        let wait = parse_wait(&mut cursor)?;
        keys.push(Keyframe {
            value,
            slope,
            interpolation,
            wait,
        });
    }

    if packed_remaining != 0 {
        return Err(DecodeError::IncompletePack {
            stream_offset: stream_offset.get(),
            missing_values: packed_remaining,
        });
    }
    if keys.len() < 2 {
        return Err(DecodeError::TooFewKeys {
            stream_offset: stream_offset.get(),
            key_count: keys.len(),
        });
    }
    if keys.last().is_some_and(|key| key.wait != 0) {
        return Err(DecodeError::NonZeroTerminalWait {
            stream_offset: stream_offset.get(),
            wait: keys.last().expect("nonempty checked above").wait,
        });
    }

    Ok(keys)
}

fn parse_pack_info(cursor: &mut StreamCursor<'_>, first: u8) -> Result<u16, DecodeError> {
    let mut count = u64::from((first >> 4) & 7) + 1;
    let mut byte = first;
    let mut shift = 3_u32;
    let mut continuation_bytes = 0_u8;
    while byte & 0x80 != 0 {
        byte = cursor.byte()?;
        continuation_bytes += 1;
        if continuation_bytes > 5 {
            return Err(DecodeError::VarintOverflow {
                stream_offset: cursor.stream_offset.get(),
                kind: "pack count",
            });
        }
        let extra =
            u64::from(byte & 0x7f)
                .checked_shl(shift)
                .ok_or(DecodeError::VarintOverflow {
                    stream_offset: cursor.stream_offset.get(),
                    kind: "pack count",
                })?;
        count = count
            .checked_add(extra)
            .filter(|&value| value <= u64::from(u16::MAX))
            .ok_or(DecodeError::VarintOverflow {
                stream_offset: cursor.stream_offset.get(),
                kind: "pack count",
            })?;
        shift += 7;
    }
    Ok(count as u16)
}

fn parse_wait(cursor: &mut StreamCursor<'_>) -> Result<u16, DecodeError> {
    let mut wait = 0_u64;
    let mut shift = 0_u32;
    for _ in 0..5 {
        let byte = cursor.byte()?;
        let value =
            u64::from(byte & 0x7f)
                .checked_shl(shift)
                .ok_or(DecodeError::VarintOverflow {
                    stream_offset: cursor.stream_offset.get(),
                    kind: "wait",
                })?;
        wait = wait
            .checked_add(value)
            .filter(|&value| value <= u64::from(u16::MAX))
            .ok_or(DecodeError::VarintOverflow {
                stream_offset: cursor.stream_offset.get(),
                kind: "wait",
            })?;
        if byte & 0x80 == 0 {
            return Ok(wait as u16);
        }
        shift += 7;
    }
    Err(DecodeError::VarintOverflow {
        stream_offset: cursor.stream_offset.get(),
        kind: "wait",
    })
}

fn evaluate_segment(from: Keyframe, to: Keyframe, time: f32, duration: u16) -> f32 {
    match from.interpolation {
        Interpolation::Constant => {
            if time >= f32::from(duration) {
                to.value
            } else {
                from.value
            }
        }
        Interpolation::Linear => {
            if duration == 0 {
                to.value
            } else {
                ((to.value - from.value) / f32::from(duration)) * time + from.value
            }
        }
        Interpolation::SplineZero | Interpolation::Spline => {
            if duration == 0 {
                to.value
            } else {
                spline::hermite(
                    1.0 / f32::from(duration),
                    time,
                    from.value,
                    to.value,
                    from.slope,
                    to.slope,
                )
            }
        }
    }
}

struct StreamCursor<'a> {
    bytes: &'a [u8],
    position: usize,
    stream_offset: DataOffset,
}

impl<'a> StreamCursor<'a> {
    fn new(bytes: &'a [u8], stream_offset: DataOffset) -> Self {
        Self {
            bytes,
            position: 0,
            stream_offset,
        }
    }

    fn is_empty(&self) -> bool {
        self.position == self.bytes.len()
    }

    fn absolute_position(&self) -> u32 {
        self.stream_offset
            .get()
            .saturating_add(self.position as u32)
    }

    fn byte(&mut self) -> Result<u8, DecodeError> {
        Ok(self.array::<1>()?[0])
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N], DecodeError> {
        let end = self
            .position
            .checked_add(N)
            .filter(|&end| end <= self.bytes.len())
            .ok_or(DecodeError::TruncatedProgram {
                stream_offset: self.stream_offset.get(),
                position: self.position,
                needed: N,
                remaining: self.bytes.len().saturating_sub(self.position),
            })?;
        let value = self.bytes[self.position..end]
            .try_into()
            .expect("validated fixed-size slice");
        self.position = end;
        Ok(value)
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum DecodeError {
    #[error("HSD data section is too large for 32-bit offsets ({length} bytes)")]
    DataSectionTooLarge { length: usize },
    #[error("cannot allocate storage for decoded {resource}")]
    AllocationFailed { resource: &'static str },
    #[error("decoded {resource} exceeds configured limit {limit}")]
    LimitExceeded {
        resource: &'static str,
        limit: usize,
    },
    #[error("{context} uses the null HSD data-section offset")]
    NullOffset { context: &'static str },
    #[error(
        "{context} at HSD data-section offset {offset:#x} with length {length} exceeds the {data_length}-byte data section"
    )]
    OutOfBounds {
        offset: u32,
        length: usize,
        data_length: usize,
        context: &'static str,
    },
    #[error("{context} at HSD data-section offset {offset:#x} is not four-byte aligned")]
    MisalignedDescriptor { offset: u32, context: &'static str },
    #[error(
        "{field} is non-finite in descriptor at HSD data-section offset {descriptor_offset:#x}"
    )]
    NonFiniteDescriptor {
        descriptor_offset: u32,
        field: &'static str,
    },
    #[error(
        "FObj start_frame cannot be represented by the source s16 field in descriptor at HSD data-section offset {descriptor_offset:#x}"
    )]
    StartFrameOutOfRange { descriptor_offset: u32 },
    #[error("FObj descriptor list cycles at HSD data-section offset {descriptor_offset:#x}")]
    FObjCycle { descriptor_offset: u32 },
    #[error(
        "unsupported {target:?} channel {channel} in FObj descriptor at HSD data-section offset {descriptor_offset:#x}"
    )]
    UnsupportedChannel {
        target: ChannelTarget,
        channel: u8,
        descriptor_offset: u32,
    },
    #[error(
        "unsupported {field} fraction encoding {encoding:#04x} in FObj descriptor at HSD data-section offset {descriptor_offset:#x}"
    )]
    UnsupportedFractionEncoding {
        encoding: u8,
        descriptor_offset: u32,
        field: &'static str,
    },
    #[error("empty FObj program in descriptor at HSD data-section offset {descriptor_offset:#x}")]
    EmptyFObjProgram { descriptor_offset: u32 },
    #[error("unsupported FObj opcode {opcode} at HSD data-section offset {stream_offset:#x}")]
    UnsupportedOpcode { opcode: u8, stream_offset: u32 },
    #[error(
        "truncated FObj program at HSD data-section offset {stream_offset:#x}: byte {position} needs {needed} bytes, only {remaining} remain"
    )]
    TruncatedProgram {
        stream_offset: u32,
        position: usize,
        needed: usize,
        remaining: usize,
    },
    #[error(
        "{kind} varint overflows HSD's 16-bit field in FObj program at data-section offset {stream_offset:#x}"
    )]
    VarintOverflow {
        stream_offset: u32,
        kind: &'static str,
    },
    #[error("non-finite scalar in FObj program at HSD data-section offset {stream_offset:#x}")]
    NonFinitePayload { stream_offset: u32 },
    #[error(
        "FObj program at HSD data-section offset {stream_offset:#x} ends with {missing_values} packed values missing"
    )]
    IncompletePack {
        stream_offset: u32,
        missing_values: u16,
    },
    #[error("FObj program at HSD data-section offset {stream_offset:#x} has only {key_count} keys")]
    TooFewKeys {
        stream_offset: u32,
        key_count: usize,
    },
    #[error("FObj program at HSD data-section offset {stream_offset:#x} omits its final wait")]
    MissingWait { stream_offset: u32 },
    #[error(
        "FObj program at HSD data-section offset {stream_offset:#x} has nonzero terminal wait {wait}"
    )]
    NonZeroTerminalWait { stream_offset: u32, wait: u16 },
}

impl DecodeError {
    const fn is_unsupported_track(&self) -> bool {
        matches!(
            self,
            Self::UnsupportedChannel { .. }
                | Self::UnsupportedFractionEncoding { .. }
                | Self::UnsupportedOpcode { .. }
        )
    }
}

#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum EvaluationError {
    #[error("animation frame must be finite")]
    NonFiniteFrame,
}

#[cfg(test)]
mod tests {
    use super::*;

    const AOBJ: usize = 4;
    const FOBJ: usize = 20;
    const PROGRAM: usize = 40;

    fn write_be_u32(bytes: &mut [u8], offset: usize, value: u32) {
        bytes[offset..offset + 4].copy_from_slice(&value.to_be_bytes());
    }

    #[test]
    fn maps_every_main_selection_material_and_texture_channel() {
        for (target, raw, expected) in [
            (ChannelTarget::Material, 4, Channel::MaterialDiffuseR),
            (ChannelTarget::Material, 5, Channel::MaterialDiffuseG),
            (ChannelTarget::Material, 6, Channel::MaterialDiffuseB),
            (ChannelTarget::Texture, 15, Channel::TextureKonstAlpha),
            (ChannelTarget::Texture, 19, Channel::TextureTev0Alpha),
        ] {
            assert_eq!(Channel::decode(target, raw), Some(expected));
        }
    }

    fn write_be_f32(bytes: &mut [u8], offset: usize, value: f32) {
        write_be_u32(bytes, offset, value.to_bits());
    }

    fn fixture(program: &[u8], channel: u8, value_encoding: u8, slope_encoding: u8) -> Vec<u8> {
        let mut bytes = vec![0_u8; PROGRAM + program.len()];
        write_be_u32(&mut bytes, AOBJ, 0x2000_0000);
        write_be_f32(&mut bytes, AOBJ + 4, 799.0);
        write_be_u32(&mut bytes, AOBJ + 8, FOBJ as u32);
        write_be_u32(&mut bytes, AOBJ + 12, 0x1234_5678);

        write_be_u32(&mut bytes, FOBJ + 4, program.len() as u32);
        write_be_f32(&mut bytes, FOBJ + 8, 0.0);
        bytes[FOBJ + 12] = channel;
        bytes[FOBJ + 13] = value_encoding;
        bytes[FOBJ + 14] = slope_encoding;
        write_be_u32(&mut bytes, FOBJ + 16, PROGRAM as u32);
        bytes[PROGRAM..].copy_from_slice(program);
        bytes
    }

    fn repeated_program_fixture(program: &[u8]) -> Vec<u8> {
        const SECOND_FOBJ: usize = 40;
        const SHARED_PROGRAM: usize = 60;
        let mut bytes = vec![0_u8; SHARED_PROGRAM + program.len()];
        write_be_u32(&mut bytes, AOBJ + 8, FOBJ as u32);
        for (offset, next) in [(FOBJ, SECOND_FOBJ as u32), (SECOND_FOBJ, 0)] {
            write_be_u32(&mut bytes, offset, next);
            write_be_u32(&mut bytes, offset + 4, program.len() as u32);
            bytes[offset + 12] = 5;
            write_be_u32(&mut bytes, offset + 16, SHARED_PROGRAM as u32);
        }
        bytes[SHARED_PROGRAM..].copy_from_slice(program);
        bytes
    }

    fn decode_track(program: &[u8]) -> FObjTrack {
        let bytes = fixture(program, 5, 0, 0);
        HsdDataSection::new(&bytes)
            .unwrap()
            .decode_aobj(DataOffset::new(AOBJ as u32), ChannelTarget::Joint)
            .unwrap()
            .tracks
            .remove(0)
    }

    fn f32_le(value: f32) -> [u8; 4] {
        value.to_le_bytes()
    }

    fn assert_close(actual: f32, expected: f32) {
        assert!((actual - expected).abs() < 1.0e-5, "{actual} != {expected}");
    }

    #[test]
    fn pack_info_uses_header_count_and_continuation_chunks() {
        let mut cursor = StreamCursor::new(&[0x01], DataOffset::new(100));
        assert_eq!(parse_pack_info(&mut cursor, 0x82).unwrap(), 9);

        let mut cursor = StreamCursor::new(&[0x81, 0x01], DataOffset::new(100));
        assert_eq!(parse_pack_info(&mut cursor, 0x81).unwrap(), 1033);
    }

    #[test]
    fn wait_uses_seven_bit_little_endian_chunks() {
        let mut cursor = StreamCursor::new(&[0xac, 0x02], DataOffset::new(100));
        assert_eq!(parse_wait(&mut cursor).unwrap(), 300);
    }

    #[test]
    fn payload_float_is_little_endian_even_though_descriptors_are_big_endian() {
        let mut program = vec![0x12];
        program.extend(f32_le(1.25));
        program.push(4);
        program.extend(f32_le(3.25));
        program.push(0);
        let track = decode_track(&program);
        assert_eq!(track.keys[0].value, 1.25);
        assert_eq!(track.keys[1].value, 3.25);
    }

    #[test]
    fn fixed_point_payload_is_little_endian_and_scaled() {
        // U16 with eight fractional bits: 0x0200 / 256 = 2.
        let track =
            HsdDataSection::new(&fixture(&[0x11, 0x00, 0x02, 4, 0x00, 0x04, 0], 5, 0x48, 0))
                .unwrap()
                .decode_aobj(DataOffset::new(AOBJ as u32), ChannelTarget::Joint)
                .unwrap()
                .tracks
                .remove(0);
        assert_eq!(track.keys[0].value, 2.0);
        assert_eq!(track.keys[1].value, 4.0);
    }

    #[test]
    fn evaluates_constant_and_linear_intervals_and_holds_the_terminal_key() {
        let mut constant = vec![0x11];
        constant.extend(f32_le(2.0));
        constant.push(10);
        constant.extend(f32_le(8.0));
        constant.push(0);
        let constant = decode_track(&constant);
        assert_eq!(constant.sample(5.0).unwrap(), Some(2.0));
        assert_eq!(constant.sample(10.0).unwrap(), Some(8.0));

        let mut linear = vec![0x12];
        linear.extend(f32_le(2.0));
        linear.push(10);
        linear.extend(f32_le(8.0));
        linear.push(0);
        let linear = decode_track(&linear);
        assert_eq!(linear.sample(5.0).unwrap(), Some(5.0));
        assert_eq!(linear.sample(15.0).unwrap(), Some(8.0));
    }

    #[test]
    fn evaluates_zero_tangent_and_encoded_tangent_splines() {
        let mut zero = vec![0x13];
        zero.extend(f32_le(0.0));
        zero.push(10);
        zero.extend(f32_le(10.0));
        zero.push(0);
        let zero = decode_track(&zero);
        assert_close(zero.sample(5.0).unwrap().unwrap(), 5.0);

        let mut spline = vec![0x14];
        spline.extend(f32_le(0.0));
        spline.extend(f32_le(2.0));
        spline.push(10);
        spline.extend(f32_le(10.0));
        spline.extend(f32_le(0.0));
        spline.push(0);
        let spline = decode_track(&spline);
        assert_close(spline.sample(5.0).unwrap().unwrap(), 7.5);
    }

    #[test]
    fn preserves_aobj_flags_end_frame_and_fobj_start_frame() {
        let mut program = vec![0x11];
        program.extend(f32_le(1.0));
        program.push(2);
        program.extend(f32_le(2.0));
        program.push(0);
        let mut bytes = fixture(&program, 5, 0, 0);
        write_be_f32(&mut bytes, FOBJ + 8, 7.5);
        let animation = HsdDataSection::new(&bytes)
            .unwrap()
            .decode_aobj(DataOffset::new(AOBJ as u32), ChannelTarget::Joint)
            .unwrap();
        assert_eq!(animation.flags, 0x2000_0000);
        assert_eq!(animation.end_frame, 799.0);
        assert_eq!(animation.object_id, 0x1234_5678);
        assert_eq!(animation.tracks[0].start_frame, 7.5);
    }

    #[test]
    fn aobj_sampling_applies_truncated_start_frame_looping_and_no_update() {
        let mut program = vec![0x12];
        program.extend(f32_le(2.0));
        program.push(10);
        program.extend(f32_le(8.0));
        program.push(0);
        let mut bytes = fixture(&program, 5, 0, 0);
        write_be_f32(&mut bytes, AOBJ + 4, 10.0);
        write_be_f32(&mut bytes, FOBJ + 8, 2.75);
        let mut animation = HsdDataSection::new(&bytes)
            .unwrap()
            .decode_aobj(DataOffset::new(AOBJ as u32), ChannelTarget::Joint)
            .unwrap();

        assert_eq!(animation.sample_requested_frame(-3.0).unwrap(), vec![]);
        assert_eq!(
            animation.sample_requested_frame(-2.0).unwrap(),
            vec![ChannelValue {
                channel: Channel::JointTranslationX,
                value: 2.0,
            }]
        );
        assert_eq!(
            animation.sample_requested_frame(13.0).unwrap(),
            vec![ChannelValue {
                channel: Channel::JointTranslationX,
                value: 5.0,
            }]
        );

        animation.flags |= AOBJ_NO_UPDATE;
        assert_eq!(animation.sample_requested_frame(3.0).unwrap(), vec![]);
    }

    #[test]
    fn start_frame_range_is_checked_after_source_integer_truncation() {
        let mut program = vec![0x11];
        program.extend(f32_le(1.0));
        program.push(1);
        program.extend(f32_le(2.0));
        program.push(0);

        for accepted in [-32768.5, 32767.5] {
            let mut bytes = fixture(&program, 5, 0, 0);
            write_be_f32(&mut bytes, FOBJ + 8, accepted);
            HsdDataSection::new(&bytes)
                .unwrap()
                .decode_aobj(DataOffset::new(AOBJ as u32), ChannelTarget::Joint)
                .unwrap();
        }
        for rejected in [-32769.0, 32768.0] {
            let mut bytes = fixture(&program, 5, 0, 0);
            write_be_f32(&mut bytes, FOBJ + 8, rejected);
            assert!(matches!(
                HsdDataSection::new(&bytes)
                    .unwrap()
                    .decode_aobj(DataOffset::new(AOBJ as u32), ChannelTarget::Joint),
                Err(DecodeError::StartFrameOutOfRange { .. })
            ));
        }
    }

    #[test]
    fn configured_limits_bound_repeated_program_allocation() {
        let mut program = vec![0x11];
        program.extend(f32_le(1.0));
        program.push(1);
        program.extend(f32_le(2.0));
        program.push(0);
        let bytes = repeated_program_fixture(&program);

        for limits in [
            DecodeLimits {
                max_tracks: 1,
                max_keys_per_track: 2,
                max_total_keys: 4,
            },
            DecodeLimits {
                max_tracks: 2,
                max_keys_per_track: 1,
                max_total_keys: 4,
            },
            DecodeLimits {
                max_tracks: 2,
                max_keys_per_track: 2,
                max_total_keys: 3,
            },
        ] {
            assert!(matches!(
                HsdDataSection::with_limits(&bytes, limits)
                    .unwrap()
                    .decode_aobj(DataOffset::new(AOBJ as u32), ChannelTarget::Joint),
                Err(DecodeError::LimitExceeded { .. })
            ));
        }
    }

    #[test]
    fn rejects_malformed_offsets_opcodes_packs_and_payloads() {
        let bytes = fixture(&[0x10, 0, 0, 0, 0, 1, 0x10, 0, 0, 0, 0], 5, 0, 0);
        let section = HsdDataSection::new(&bytes).unwrap();
        assert!(matches!(
            section.decode_aobj(DataOffset::new(AOBJ as u32), ChannelTarget::Joint),
            Err(DecodeError::UnsupportedOpcode { opcode: 0, .. })
        ));
        assert!(matches!(
            section.decode_aobj(DataOffset::new(usize::MAX as u32), ChannelTarget::Joint),
            Err(DecodeError::MisalignedDescriptor { .. } | DecodeError::OutOfBounds { .. })
        ));

        let unsupported_channel = fixture(&[0x11, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0], 14, 0, 0);
        assert!(matches!(
            HsdDataSection::new(&unsupported_channel)
                .unwrap()
                .decode_aobj(DataOffset::new(AOBJ as u32), ChannelTarget::Material),
            Err(DecodeError::UnsupportedChannel { channel: 14, .. })
        ));

        let incomplete = fixture(&[0x21, 0, 0, 0, 0, 0], 5, 0, 0);
        assert!(matches!(
            HsdDataSection::new(&incomplete)
                .unwrap()
                .decode_aobj(DataOffset::new(AOBJ as u32), ChannelTarget::Joint),
            Err(DecodeError::IncompletePack {
                missing_values: 2,
                ..
            })
        ));

        let truncated = fixture(&[0x11, 0, 0], 5, 0, 0);
        assert!(matches!(
            HsdDataSection::new(&truncated)
                .unwrap()
                .decode_aobj(DataOffset::new(AOBJ as u32), ChannelTarget::Joint),
            Err(DecodeError::TruncatedProgram { .. })
        ));

        let mut non_finite = vec![0x11];
        non_finite.extend(f32::NAN.to_le_bytes());
        non_finite.push(1);
        non_finite.extend(f32_le(0.0));
        non_finite.push(0);
        let non_finite = fixture(&non_finite, 5, 0, 0);
        assert!(matches!(
            HsdDataSection::new(&non_finite)
                .unwrap()
                .decode_aobj(DataOffset::new(AOBJ as u32), ChannelTarget::Joint),
            Err(DecodeError::NonFinitePayload { .. })
        ));

        let mut nonzero_terminal = vec![0x11];
        nonzero_terminal.extend(f32_le(0.0));
        nonzero_terminal.push(1);
        nonzero_terminal.extend(f32_le(1.0));
        nonzero_terminal.push(1);
        let nonzero_terminal = fixture(&nonzero_terminal, 5, 0, 0);
        assert!(matches!(
            HsdDataSection::new(&nonzero_terminal)
                .unwrap()
                .decode_aobj(DataOffset::new(AOBJ as u32), ChannelTarget::Joint),
            Err(DecodeError::NonZeroTerminalWait { wait: 1, .. })
        ));
    }

    #[test]
    fn strict_aobj_decode_still_returns_the_first_unsupported_track() {
        const SECOND_FOBJ: usize = 52;
        let mut bytes = fixture(&[0x11, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0], 14, 0, 0);
        bytes.resize(SECOND_FOBJ + FOBJ_DESC_SIZE, 0);
        write_be_u32(&mut bytes, FOBJ, SECOND_FOBJ as u32);
        bytes[SECOND_FOBJ + 12] = 5;

        let section = HsdDataSection::new(&bytes).unwrap();
        assert!(matches!(
            section.decode_aobj(DataOffset::new(AOBJ as u32), ChannelTarget::Material),
            Err(DecodeError::UnsupportedChannel {
                descriptor_offset,
                ..
            }) if descriptor_offset == FOBJ as u32
        ));
        assert!(matches!(
            section.decode_aobj_supported(DataOffset::new(AOBJ as u32), ChannelTarget::Material),
            Err(DecodeError::EmptyFObjProgram { descriptor_offset })
                if descriptor_offset == SECOND_FOBJ as u32
        ));
    }
}
