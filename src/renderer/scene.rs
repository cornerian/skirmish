//! CPU assets for the native visual export. This is a Lambert preview, not GX emulation.
use anyhow::{Context, Result, bail, ensure};
use serde::Deserialize;
use serde_json::Value;
use std::{
    collections::{HashMap, HashSet},
    fs::File,
    io::{BufReader, Read},
    path::Path,
    sync::Arc,
};

#[repr(C)]
#[derive(Clone, Copy, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub uv: [f32; 2],
    pub color: [f32; 4],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CullMode {
    None,
    Front,
    Back,
    All,
}

/// Stable namespace of one archive represented by a visual export.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct VisualResourceId(Arc<str>);

impl VisualResourceId {
    pub fn as_str(&self) -> &str {
        &self.0
    }

    #[cfg(test)]
    pub(crate) fn for_test(id: &str) -> Self {
        Self(Arc::from(id))
    }
}

/// Provenance required before visual offsets can be joined to a native manifest.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VisualResourceProvenance {
    id: VisualResourceId,
    sha256: Arc<str>,
}

impl VisualResourceProvenance {
    pub fn id(&self) -> &VisualResourceId {
        &self.id
    }

    pub fn sha256(&self) -> &str {
        &self.sha256
    }
}

/// Exact source JObj occurrence namespace for one exported draw.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct VisualJointOccurrence {
    pub resource_id: VisualResourceId,
    /// Descriptor position in the visual export's declared joint offset space.
    pub visual_offset: u32,
}

/// Exact DObj occurrence under one source JObj.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct VisualDObjOccurrence {
    pub owner_joint: VisualJointOccurrence,
    pub dobj_index: u16,
}

/// Exact MObj occurrence used by one DObj.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct VisualMaterialOccurrence {
    pub owner_dobj: VisualDObjOccurrence,
    /// Descriptor position in the visual export's declared material offset space.
    pub visual_offset: MaterialSourceId,
}

/// Exact TObj occurrence used by one MObj.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct VisualTextureOccurrence {
    pub owner_material: VisualMaterialOccurrence,
    pub tobj_index: u16,
    /// Descriptor position in the visual export's declared texture offset space.
    pub visual_offset: TextureSourceId,
}

/// Ordered source metadata for one authored texture stage.
///
/// `visual_offset` preserves legacy exporter metadata even when the complete
/// resource/MObj/TObj occurrence tuple is unavailable.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VisualTextureStageSource {
    pub visual_offset: Option<TextureSourceId>,
    pub occurrence: Option<VisualTextureOccurrence>,
}

/// Source MObj descriptor identity within one `skirmish-visual-v1` resource.
///
/// The integer is retained in the visual export's coordinate space, which a
/// presentation manifest must normalize before joining it to native data. It
/// remains distinct from joints and texture stages so an adapter cannot target
/// every material owned by one joint accidentally.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct MaterialSourceId(u32);

impl MaterialSourceId {
    pub const fn new(offset: u32) -> Self {
        Self(offset)
    }

    pub const fn get(self) -> u32 {
        self.0
    }
}

/// Source TObj visual offset for an authored texture stage.
///
/// Like [`MaterialSourceId`], this is resource-local and remains in the
/// exporter's coordinate space. Ordered stage metadata is retained on
/// [`Material::texture_sources`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct TextureSourceId(u32);

impl TextureSourceId {
    pub const fn new(offset: u32) -> Self {
        Self(offset)
    }

    pub const fn get(self) -> u32 {
        self.0
    }
}

/// Fixed HSD draw pass encoded in the high render-mode bits.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum RenderModeClass {
    Opaque,
    TextureEdge,
    Translucent,
}

/// Validated source MObj render mode.
///
/// All flags are retained, while the mutually exclusive pass bits are checked
/// once at load time. `0x2000_0000` is not a valid HSD pass class.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct RenderMode(u32);

impl RenderMode {
    const CLASS_MASK: u32 = 0x6000_0000;
    const CHANNEL_MODE_MASK: u32 = 0x3;
    const ALPHA_MODE_SHIFT: u32 = 13;

    pub const fn from_bits(bits: u32) -> Option<Self> {
        if bits & Self::CLASS_MASK == 0x2000_0000 {
            None
        } else {
            Some(Self(bits))
        }
    }

    pub const fn bits(self) -> u32 {
        self.0
    }

    pub const fn class(self) -> RenderModeClass {
        match self.0 & Self::CLASS_MASK {
            0 => RenderModeClass::Opaque,
            0x4000_0000 => RenderModeClass::TextureEdge,
            0x6000_0000 => RenderModeClass::Translucent,
            _ => unreachable!(),
        }
    }

    /// Whether the HSD diffuse channel reads its value from vertex color.
    pub const fn uses_vertex_color(self) -> bool {
        self.diffuse_mode() & 0x2 != 0
    }

    /// Whether the HSD alpha channel reads its value from vertex alpha.
    pub const fn uses_vertex_alpha(self) -> bool {
        let encoded = (self.0 >> Self::ALPHA_MODE_SHIFT) & Self::CHANNEL_MODE_MASK;
        let mode = if encoded == 0 {
            self.diffuse_mode()
        } else {
            encoded
        };
        mode & 0x2 != 0
    }

    const fn diffuse_mode(self) -> u32 {
        let encoded = self.0 & Self::CHANNEL_MODE_MASK;
        if encoded == 0 { 1 } else { encoded }
    }
}

/// GX comparison function shared by depth and alpha tests.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum PeCompare {
    Never = 0,
    Less = 1,
    Equal = 2,
    LessEqual = 3,
    Greater = 4,
    NotEqual = 5,
    GreaterEqual = 6,
    Always = 7,
}

impl PeCompare {
    pub const fn code(self) -> u8 {
        self as u8
    }

    pub const fn test(self, value: u8, reference: u8) -> bool {
        match self {
            Self::Never => false,
            Self::Less => value < reference,
            Self::Equal => value == reference,
            Self::LessEqual => value <= reference,
            Self::Greater => value > reference,
            Self::NotEqual => value != reference,
            Self::GreaterEqual => value >= reference,
            Self::Always => true,
        }
    }

    fn from_code(code: u8) -> Result<Self> {
        Ok(match code {
            0 => Self::Never,
            1 => Self::Less,
            2 => Self::Equal,
            3 => Self::LessEqual,
            4 => Self::Greater,
            5 => Self::NotEqual,
            6 => Self::GreaterEqual,
            7 => Self::Always,
            _ => bail!("GX comparison code {code} is outside 0..=7"),
        })
    }
}

/// GX boolean operation joining two alpha comparisons.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum PeAlphaOp {
    And = 0,
    Or = 1,
    Xor = 2,
    Xnor = 3,
}

impl PeAlphaOp {
    pub const fn code(self) -> u8 {
        self as u8
    }

    const fn combine(self, left: bool, right: bool) -> bool {
        match self {
            Self::And => left && right,
            Self::Or => left || right,
            Self::Xor => left != right,
            Self::Xnor => left == right,
        }
    }

    fn from_code(code: u8) -> Result<Self> {
        Ok(match code {
            0 => Self::And,
            1 => Self::Or,
            2 => Self::Xor,
            3 => Self::Xnor,
            _ => bail!("GX alpha operation code {code} is outside 0..=3"),
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum PeBlendMode {
    None = 0,
    Blend = 1,
    Logic = 2,
    Subtract = 3,
}

impl PeBlendMode {
    fn from_code(code: u8) -> Result<Self> {
        Ok(match code {
            0 => Self::None,
            1 => Self::Blend,
            2 => Self::Logic,
            3 => Self::Subtract,
            _ => bail!("GX blend mode code {code} is outside 0..=3"),
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum PeBlendFactor {
    Zero = 0,
    One = 1,
    SourceColor = 2,
    InverseSourceColor = 3,
    SourceAlpha = 4,
    InverseSourceAlpha = 5,
    DestinationAlpha = 6,
    InverseDestinationAlpha = 7,
}

impl PeBlendFactor {
    fn from_code(code: u8) -> Result<Self> {
        Ok(match code {
            0 => Self::Zero,
            1 => Self::One,
            2 => Self::SourceColor,
            3 => Self::InverseSourceColor,
            4 => Self::SourceAlpha,
            5 => Self::InverseSourceAlpha,
            6 => Self::DestinationAlpha,
            7 => Self::InverseDestinationAlpha,
            _ => bail!("GX blend factor code {code} is outside 0..=7"),
        })
    }
}

/// Validated GX logic-op byte. Logic blending is retained but not rendered yet.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct PeLogicOp(u8);

impl PeLogicOp {
    pub const fn new(code: u8) -> Option<Self> {
        if code <= 15 { Some(Self(code)) } else { None }
    }

    pub const fn code(self) -> u8 {
        self.0
    }

    fn from_code(code: u8) -> Result<Self> {
        Self::new(code).with_context(|| format!("GX logic operation code {code} is outside 0..=15"))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PeBlendState {
    pub mode: PeBlendMode,
    pub source_factor: PeBlendFactor,
    pub destination_factor: PeBlendFactor,
    pub logic_operation: PeLogicOp,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PeDepthState {
    pub test_enabled: bool,
    pub write_enabled: bool,
    pub comparison: PeCompare,
    /// True when GX compares depth before texture evaluation and alpha testing.
    pub compare_before_texture: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PeDestinationAlpha {
    pub enabled: bool,
    pub value: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PeAlphaTest {
    pub comparison0: PeCompare,
    pub reference0: u8,
    pub operation: PeAlphaOp,
    pub comparison1: PeCompare,
    pub reference1: u8,
}

impl PeAlphaTest {
    pub const fn passes(self, alpha: u8) -> bool {
        self.operation.combine(
            self.comparison0.test(alpha, self.reference0),
            self.comparison1.test(alpha, self.reference1),
        )
    }

    pub fn can_reject(self) -> bool {
        (u8::MIN..=u8::MAX).any(|alpha| !self.passes(alpha))
    }

    const fn always() -> Self {
        Self {
            comparison0: PeCompare::Always,
            reference0: 0,
            operation: PeAlphaOp::And,
            comparison1: PeCompare::Always,
            reference1: 0,
        }
    }
}

/// Effective HSD pixel-engine state, independent of the GPU backend.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PixelEngineState {
    pub explicit_descriptor: bool,
    pub color_write: bool,
    pub alpha_write: bool,
    pub destination_alpha: PeDestinationAlpha,
    pub blend: PeBlendState,
    pub depth: PeDepthState,
    pub alpha_test: PeAlphaTest,
    pub dither: bool,
}

impl PixelEngineState {
    /// Reproduces HSD_SetupPEMode defaults when no explicit PE descriptor exists.
    pub const fn from_render_mode(render_mode: RenderMode) -> Self {
        let bits = render_mode.bits();
        let blended = bits & 0x4000_0000 != 0;
        let depth_write = bits & 0x2000_0000 == 0;
        let texture_edge = blended && depth_write;
        let alpha_test = if texture_edge {
            PeAlphaTest {
                comparison0: PeCompare::Greater,
                reference0: 0,
                operation: PeAlphaOp::And,
                comparison1: PeCompare::Greater,
                reference1: 0,
            }
        } else {
            PeAlphaTest::always()
        };
        Self {
            explicit_descriptor: false,
            color_write: true,
            alpha_write: false,
            destination_alpha: PeDestinationAlpha {
                enabled: false,
                value: 0,
            },
            blend: PeBlendState {
                mode: if blended {
                    PeBlendMode::Blend
                } else {
                    PeBlendMode::None
                },
                source_factor: PeBlendFactor::SourceAlpha,
                destination_factor: PeBlendFactor::InverseSourceAlpha,
                logic_operation: PeLogicOp(15),
            },
            depth: PeDepthState {
                test_enabled: true,
                write_enabled: depth_write,
                comparison: if bits & 0x0800_0000 != 0 {
                    PeCompare::Always
                } else {
                    PeCompare::LessEqual
                },
                compare_before_texture: !texture_edge,
            },
            alpha_test,
            dither: false,
        }
    }

    /// Rejects PE behavior that the portable wgpu renderer cannot reproduce.
    pub fn validate_supported(self) -> Result<()> {
        ensure!(
            !self.destination_alpha.enabled,
            "GX destination-alpha override is not supported"
        );
        ensure!(!self.dither, "GX pixel-engine dithering is not supported");
        ensure!(
            self.blend.mode != PeBlendMode::Logic,
            "GX logic blending is not supported"
        );
        ensure!(
            !(self.depth.compare_before_texture
                && self.depth.write_enabled
                && self.alpha_test.can_reject()),
            "GX depth-before-texture with depth writes and a rejecting alpha test is not portable"
        );
        Ok(())
    }

    fn flags(self) -> u8 {
        u8::from(self.color_write)
            | (u8::from(self.alpha_write) << 1)
            | (u8::from(self.destination_alpha.enabled) << 2)
            | (u8::from(self.depth.compare_before_texture) << 3)
            | (u8::from(self.depth.test_enabled) << 4)
            | (u8::from(self.depth.write_enabled) << 5)
            | (u8::from(self.dither) << 6)
    }

    fn same_behavior(mut self, mut other: Self) -> bool {
        self.explicit_descriptor = false;
        other.explicit_descriptor = false;
        self == other
    }
}

#[derive(Clone, Debug)]
pub struct Material {
    /// Exact source MObj descriptor when exported; absent for legacy/procedural scenes.
    pub source_id: Option<MaterialSourceId>,
    /// Complete resource/JObj/DObj/MObj identity when exported by the exact schema.
    pub source_occurrence: Option<VisualMaterialOccurrence>,
    /// Source identity for every authored texture stage, in authored order.
    pub texture_sources: Vec<VisualTextureStageSource>,
    /// Fixed source pass and the remaining original MObj render flags.
    pub render_mode: Option<RenderMode>,
    /// Effective exported pixel-engine state; absent only for legacy/procedural scenes.
    pub pixel_engine: Option<PixelEngineState>,
    pub color: [f32; 4],
    pub texture: Option<usize>,
    pub cull_mode: CullMode,
}

#[derive(Clone, Debug)]
pub struct Mesh {
    pub name: String,
    /// Source JObj descriptor offset that owns this draw part.
    pub joint: Option<u32>,
    /// Stable exporter identity for repeated instances of the same source part.
    pub instance_id: Option<String>,
    /// Complete resource/JObj/DObj identity, absent for legacy visual scenes.
    pub source_occurrence: Option<VisualDObjOccurrence>,
    pub vertices: Vec<Vertex>,
    /// Counterclockwise front faces, converted from the source's winding at load time.
    pub indices: Vec<u32>,
    pub material: Material,
    pub hidden: bool,
}

/// Serialized source-joint state retained for future instance and animation evaluation.
///
/// Matrices use the exporter's column-major layout. They are immutable resource
/// metadata for now; the preview renderer does not apply them to draw calls yet.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct JointPose {
    pub flags: u32,
    pub local: [f32; 16],
    pub world: [f32; 16],
    pub inverse_bind: [f32; 16],
}

#[derive(Clone, Debug, PartialEq)]
pub struct Joint {
    pub name: String,
    pub offset: u32,
    pub parent: Option<u32>,
    /// Present when the visual export includes its complete serialized pose tuple.
    pub pose: Option<JointPose>,
}

#[derive(Clone, Debug)]
pub struct Texture {
    pub name: String,
    pub width: u32,
    pub height: u32,
    /// Straight-alpha sRGB bytes in top-to-bottom row order.
    pub rgba: Vec<u8>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Camera {
    pub eye: [f32; 3],
    pub interest: [f32; 3],
    pub up: [f32; 3],
    pub vertical_fov_radians: f32,
    pub aspect: f32,
    pub near: f32,
    pub far: f32,
}

#[derive(Clone, Debug)]
pub struct Scene {
    pub source: Option<String>,
    pub resources: Vec<VisualResourceProvenance>,
    pub joints: Vec<Joint>,
    pub meshes: Vec<Mesh>,
    pub textures: Vec<Texture>,
    pub warnings: Vec<String>,
    /// Authored scene camera when one is available; otherwise the renderer frames bounds.
    pub camera: Option<Camera>,
    /// Linear RGBA clear color used before scene draws.
    pub clear_color: [f32; 4],
}

#[derive(Deserialize)]
struct Document {
    schema: String,
    #[serde(default)]
    source: Option<String>,
    #[serde(default)]
    resources: Vec<RawVisualResource>,
    #[serde(default)]
    source_winding: Option<String>,
    meshes: Vec<RawMesh>,
    #[serde(default)]
    joints: Vec<RawJoint>,
    #[serde(default)]
    textures: Vec<RawTexture>,
    #[serde(default)]
    limitations: Vec<String>,
}

#[derive(Deserialize)]
struct RawMesh {
    name: String,
    #[serde(default)]
    joint: Option<u32>,
    #[serde(default)]
    resource_id: Option<String>,
    #[serde(default)]
    dobj_index: Option<u16>,
    #[serde(default)]
    instance_id: Option<String>,
    positions: Vec<[f32; 3]>,
    #[serde(default)]
    normals: Option<Vec<[f32; 3]>>,
    #[serde(default)]
    uv0: Option<Vec<[f32; 2]>>,
    #[serde(default)]
    colors0: Option<Vec<[f32; 4]>>,
    indices: Vec<u32>,
    #[serde(default)]
    line_indices: Vec<u32>,
    #[serde(default)]
    point_indices: Vec<u32>,
    #[serde(default)]
    hidden: bool,
    #[serde(default)]
    color: Option<[f32; 4]>,
    #[serde(default)]
    material: Value,
    #[serde(default, alias = "cull")]
    cull_mode: Value,
}

#[derive(Deserialize)]
struct RawVisualResource {
    id: String,
    sha256: String,
}

#[derive(Deserialize)]
struct RawJoint {
    name: String,
    offset: u32,
    #[serde(default)]
    parent: Option<u32>,
    #[serde(default)]
    flags: Option<u32>,
    #[serde(default)]
    local: Option<[f32; 16]>,
    #[serde(default)]
    world: Option<[f32; 16]>,
    #[serde(default)]
    inverse_bind: Option<[f32; 16]>,
}

#[derive(Deserialize)]
struct RawTexture {
    id: String,
    path: String,
    width: u32,
    height: u32,
}

#[derive(Deserialize)]
struct RawPixelEngine {
    pe_flags: u8,
    explicit_descriptor: bool,
    color_write: bool,
    alpha_write: bool,
    destination_alpha: RawDestinationAlpha,
    blend: RawBlendState,
    depth: RawDepthState,
    alpha_test: RawAlphaTest,
    dither: bool,
}

#[derive(Deserialize)]
struct RawDestinationAlpha {
    enabled: bool,
    value: u8,
}

#[derive(Deserialize)]
struct RawBlendState {
    #[serde(rename = "type")]
    mode: u8,
    src_factor: u8,
    dst_factor: u8,
    logic_op: u8,
}

#[derive(Deserialize)]
struct RawDepthState {
    test: bool,
    write: bool,
    compare: u8,
    before_texture: bool,
}

#[derive(Deserialize)]
struct RawAlphaTest {
    compare0: u8,
    reference0: u8,
    op: u8,
    compare1: u8,
    reference1: u8,
}

impl Scene {
    /// Read the native JSON export and only the PNGs used by its first texture stages.
    /// Relative PNG paths resolve against the scene directory; absolute paths are
    /// used as written, matching the asset viewer's composed scene exports.
    pub fn load(path: &Path) -> Result<Self> {
        Self::load_with_joint_roots(path, None)
    }

    /// Read only draw parts descended from the requested source JObj roots.
    ///
    /// Melee archives contain many independently instantiated public roots. The
    /// original scene code chooses roots by symbol; rendering every archive root
    /// at once is not equivalent. Offsets are the stable source identities carried
    /// by `skirmish-visual-v1` until symbol names are exported alongside them.
    pub fn load_joint_roots(path: &Path, roots: &[u32]) -> Result<Self> {
        ensure!(
            !roots.is_empty(),
            "at least one source joint root is required"
        );
        Self::load_with_joint_roots(path, Some(roots))
    }

    fn load_with_joint_roots(path: &Path, roots: Option<&[u32]>) -> Result<Self> {
        const MAX_JSON_BYTES: u64 = 128 * 1024 * 1024;
        let mut text = String::new();
        File::open(path)
            .with_context(|| format!("open scene {}", path.display()))?
            .take(MAX_JSON_BYTES + 1)
            .read_to_string(&mut text)?;
        ensure!(
            text.len() as u64 <= MAX_JSON_BYTES,
            "scene JSON exceeds 128 MiB"
        );
        let mut document: Document =
            serde_json::from_str(&text).context("decode visual scene JSON")?;
        ensure!(
            document.schema == "skirmish-visual-v1",
            "unsupported scene schema: {}",
            document.schema
        );
        let (resources, resource_ids) =
            load_visual_resources(std::mem::take(&mut document.resources))?;
        let clockwise = match document.source_winding.as_deref() {
            None | Some("cw") => true,
            Some("ccw") => false,
            Some(other) => bail!("unsupported source_winding: {other}"),
        };
        let mut joints: Vec<_> = document
            .joints
            .drain(..)
            .map(load_joint)
            .collect::<Result<_>>()?;
        let parents = validate_joint_tree(&joints)?;
        for mesh in &document.meshes {
            ensure!(
                mesh.joint.is_none_or(|joint| parents.contains_key(&joint)),
                "{} references missing source joint {:?}",
                mesh.name,
                mesh.joint
            );
        }
        validate_visual_joint_namespaces(&document.meshes, &resource_ids)?;
        if let Some(roots) = roots {
            for &root in roots {
                ensure!(
                    parents.contains_key(&root),
                    "missing source joint root {root}"
                );
            }
            let selected: HashSet<_> = parents
                .keys()
                .copied()
                .filter(|&joint| descends_from(joint, roots, &parents))
                .collect();
            joints.retain(|joint| selected.contains(&joint.offset));
            document
                .meshes
                .retain(|mesh| mesh.joint.is_some_and(|joint| selected.contains(&joint)));
        }
        let folder = path.parent().unwrap_or(Path::new("."));
        let mut table = HashMap::new();
        for texture in document.textures {
            ensure!(
                !table.contains_key(&texture.id),
                "duplicate texture id: {}",
                texture.id
            );
            table.insert(texture.id.clone(), texture);
        }
        let mut loaded = HashMap::new();
        let mut scene = Self {
            source: document.source,
            resources,
            joints,
            meshes: Vec::new(),
            textures: Vec::new(),
            warnings: document.limitations,
            camera: None,
            clear_color: [0.018, 0.025, 0.045, 1.0],
        };
        scene.warnings.push("Textured Lambert preview: GX TEV operations and fixed-point precision, source lighting, animation and skinning are not reproduced.".into());
        for mut raw in document.meshes {
            let count = raw.positions.len();
            validate_attribute(&raw.positions, count, &raw.name, "positions")?;
            ensure!(
                count <= u32::MAX as usize,
                "{}: too many vertices",
                raw.name
            );
            if let Some(values) = &raw.normals {
                validate_attribute(values, count, &raw.name, "normals")?;
            }
            if let Some(values) = &raw.uv0 {
                validate_attribute(values, count, &raw.name, "uv0")?;
            }
            if let Some(values) = &raw.colors0 {
                validate_attribute(values, count, &raw.name, "colors0")?;
            }
            ensure!(
                raw.indices.len().is_multiple_of(3),
                "{}: triangle index count is not divisible by three",
                raw.name
            );
            ensure!(
                raw.line_indices.len().is_multiple_of(2),
                "{}: line index count is not divisible by two",
                raw.name
            );
            ensure!(
                raw.indices
                    .iter()
                    .chain(&raw.line_indices)
                    .chain(&raw.point_indices)
                    .all(|&i| (i as usize) < count),
                "{}: index out of bounds",
                raw.name
            );
            if !raw.line_indices.is_empty() || !raw.point_indices.is_empty() {
                scene.warnings.push(format!(
                    "{}: line and point primitives are omitted.",
                    raw.name
                ));
            }
            if clockwise {
                for triangle in raw.indices.as_chunks_mut::<3>().0 {
                    triangle.swap(1, 2);
                }
            }
            ensure!(
                raw.material.is_null() || raw.material.is_object(),
                "{}: material must be an object",
                raw.name
            );
            let source_occurrence = visual_dobj_occurrence(&raw, &resource_ids)
                .with_context(|| format!("{}: invalid source occurrence", raw.name))?;
            let material_source_id = optional_u32_field(&raw.material, "material_offset")
                .with_context(|| format!("{}: invalid material_offset", raw.name))?
                .map(MaterialSourceId::new);
            let material_source_occurrence = source_occurrence
                .as_ref()
                .zip(material_source_id)
                .map(|(owner_dobj, descriptor_offset)| VisualMaterialOccurrence {
                    owner_dobj: owner_dobj.clone(),
                    visual_offset: descriptor_offset,
                });
            let render_mode = optional_u32_field(&raw.material, "render_mode")
                .with_context(|| format!("{}: invalid render_mode", raw.name))?
                .map(|bits| {
                    RenderMode::from_bits(bits).with_context(|| {
                        format!(
                            "{}: render_mode {bits:#010x} uses reserved pass bits 0x20000000",
                            raw.name
                        )
                    })
                })
                .transpose()?;
            let pixel_engine = pixel_engine_state(&raw.material, render_mode)
                .with_context(|| format!("{}: invalid pixel-engine state", raw.name))?;
            let mut color = raw.color.unwrap_or([1.; 4]);
            if let Some(diffuse) = raw.material.get("diffuse").filter(|v| !v.is_null()) {
                color = serde_json::from_value(diffuse.clone())
                    .with_context(|| format!("{}: invalid diffuse color", raw.name))?;
            }
            if let Some(alpha) = raw.material.get("material_alpha").filter(|v| !v.is_null()) {
                color[3] = serde_json::from_value(alpha.clone())
                    .with_context(|| format!("{}: invalid material alpha", raw.name))?;
            }
            ensure!(
                color.iter().all(|v| v.is_finite()),
                "{}: nonfinite material color",
                raw.name
            );
            let cull = if raw.cull_mode.is_null() {
                raw.material
                    .get("cull_mode")
                    .or_else(|| raw.material.get("cull"))
                    .unwrap_or(&Value::Null)
            } else {
                &raw.cull_mode
            };
            let cull_mode =
                cull_mode(cull).with_context(|| format!("{}: invalid cull mode", raw.name))?;
            let (texture, texture_sources) = if let Some(stages) =
                raw.material.get("textures").filter(|v| !v.is_null())
            {
                let stages = stages
                    .as_array()
                    .context("material textures must be an array")?;
                let mut texture_sources = Vec::with_capacity(stages.len());
                for (stage_index, stage) in stages.iter().enumerate() {
                    ensure!(
                        stage.is_object(),
                        "{}: texture stage must be an object (index {stage_index})",
                        raw.name
                    );
                    let descriptor_offset = optional_u32_field(stage, "tobj_offset")
                        .with_context(|| {
                            format!(
                                "{}: invalid texture stage {stage_index} tobj_offset",
                                raw.name
                            )
                        })?
                        .map(TextureSourceId::new);
                    let tobj_index =
                        optional_u16_field(stage, "tobj_index").with_context(|| {
                            format!(
                                "{}: invalid texture stage {stage_index} tobj_index",
                                raw.name
                            )
                        })?;
                    let occurrence = visual_texture_occurrence(
                        source_occurrence.as_ref(),
                        material_source_occurrence.as_ref(),
                        descriptor_offset,
                        tobj_index,
                        stage_index,
                    )
                    .with_context(|| {
                        format!(
                            "{}: invalid texture stage {stage_index} occurrence",
                            raw.name
                        )
                    })?;
                    texture_sources.push(VisualTextureStageSource {
                        visual_offset: descriptor_offset,
                        occurrence,
                    });
                }
                if stages.len() > 1 {
                    scene.warnings.push(format!(
                        "{}: only the first of {} texture stages is sampled.",
                        raw.name,
                        stages.len()
                    ));
                }
                if let Some(stage) = stages.first() {
                    scene.warnings.push(format!("{}: first texture uses UV0, repeat wrapping and linear filtering; GX texture transforms, coordinate generation, LOD and texture operations are approximated.", raw.name));
                    if ["wrap_s", "wrap_t"].iter().any(|key| {
                        stage
                            .get(*key)
                            .and_then(Value::as_u64)
                            .is_some_and(|v| v != 1)
                    }) {
                        scene.warnings.push(format!(
                            "{}: source texture wrapping differs from preview repeat wrapping.",
                            raw.name
                        ));
                    }
                    let id = stage
                        .get("texture_id")
                        .and_then(Value::as_str)
                        .or_else(|| stage.get("texture_ids")?.get(0)?.as_str());
                    let texture = match id.and_then(|id| table.get(id)) {
                        Some(source) => {
                            let index = if let Some(&index) = loaded.get(&source.id) {
                                index
                            } else {
                                let index = scene.textures.len();
                                scene.textures.push(load_texture(folder, source)?);
                                loaded.insert(source.id.clone(), index);
                                index
                            };
                            Some(index)
                        }
                        None => {
                            scene.warnings.push(format!(
                                "{}: unresolved first texture {}; diffuse color is used.",
                                raw.name,
                                id.unwrap_or("(no ID)")
                            ));
                            None
                        }
                    };
                    (texture, texture_sources)
                } else {
                    (None, Vec::new())
                }
            } else {
                (None, Vec::new())
            };
            for warning in raw
                .material
                .get("warnings")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
            {
                scene.warnings.push(format!("{}: {warning}", raw.name));
            }
            let normals = raw.normals.unwrap_or_else(|| {
                scene.warnings.push(format!(
                    "{}: missing normals were generated from triangles.",
                    raw.name
                ));
                generated_normals(&raw.positions, &raw.indices)
            });
            let explicit_use_color = raw
                .material
                .get("uses_vertex_color")
                .and_then(Value::as_bool);
            let explicit_use_alpha = raw
                .material
                .get("uses_vertex_alpha")
                .and_then(Value::as_bool);
            let (use_color, use_alpha) = if let Some(mode) = render_mode {
                let use_color = mode.uses_vertex_color();
                let use_alpha = mode.uses_vertex_alpha();
                ensure!(
                    explicit_use_color.is_none_or(|explicit| explicit == use_color),
                    "{}: uses_vertex_color contradicts render_mode",
                    raw.name
                );
                ensure!(
                    explicit_use_alpha.is_none_or(|explicit| explicit == use_alpha),
                    "{}: uses_vertex_alpha contradicts render_mode",
                    raw.name
                );
                (use_color, use_alpha)
            } else {
                (
                    explicit_use_color.unwrap_or(true),
                    explicit_use_alpha.unwrap_or(true),
                )
            };
            // GX vertex channels replace their material channels; the renderer multiplies
            // these normalized factors, so an active source vertex channel needs identity.
            let material_uses_vertex_color = render_mode
                .map(RenderMode::uses_vertex_color)
                .unwrap_or(explicit_use_color == Some(true));
            let material_uses_vertex_alpha = render_mode
                .map(RenderMode::uses_vertex_alpha)
                .unwrap_or(explicit_use_alpha == Some(true));
            if material_uses_vertex_color {
                color[..3].fill(1.);
            }
            if material_uses_vertex_alpha {
                color[3] = 1.;
            }
            let vertices = raw
                .positions
                .iter()
                .enumerate()
                .map(|(i, &position)| {
                    let mut color = raw.colors0.as_ref().map_or([1.; 4], |values| values[i]);
                    if !use_color {
                        color[..3].fill(1.);
                    }
                    if !use_alpha {
                        color[3] = 1.;
                    }
                    Vertex {
                        position,
                        normal: unit_normal(normals[i].map(f64::from)),
                        uv: raw.uv0.as_ref().map_or([0.; 2], |values| values[i]),
                        color,
                    }
                })
                .collect();
            scene.meshes.push(Mesh {
                name: raw.name,
                joint: raw.joint,
                instance_id: raw.instance_id,
                source_occurrence,
                vertices,
                indices: raw.indices,
                material: Material {
                    source_id: material_source_id,
                    source_occurrence: material_source_occurrence,
                    texture_sources,
                    render_mode,
                    pixel_engine,
                    color,
                    texture,
                    cull_mode,
                },
                hidden: raw.hidden,
            });
        }
        Ok(scene)
    }

    /// Visible triangle bounds in source coordinates, suitable for automatic camera framing.
    pub fn bounds(&self) -> Option<([f32; 3], [f32; 3])> {
        let mut bounds: Option<([f32; 3], [f32; 3])> = None;
        for mesh in self
            .meshes
            .iter()
            .filter(|m| !m.hidden && m.material.cull_mode != CullMode::All)
        {
            for &index in &mesh.indices {
                let p = mesh.vertices[index as usize].position;
                let (min, max) = bounds.get_or_insert((p, p));
                for axis in 0..3 {
                    min[axis] = min[axis].min(p[axis]);
                    max[axis] = max[axis].max(p[axis]);
                }
            }
        }
        bounds
    }

    /// A colored checker cube and floor authored here; no external game assets are needed.
    pub fn demo() -> Self {
        let mut cube = Mesh {
            name: "checker cube".into(),
            joint: None,
            instance_id: None,
            source_occurrence: None,
            vertices: Vec::new(),
            indices: Vec::new(),
            material: Material {
                source_id: None,
                source_occurrence: None,
                texture_sources: Vec::new(),
                render_mode: None,
                pixel_engine: None,
                color: [1.; 4],
                texture: Some(0),
                cull_mode: CullMode::Back,
            },
            hidden: false,
        };
        let faces = [
            (
                [0., 0., 1.],
                [[-1., 0., 1.], [1., 0., 1.], [1., 2., 1.], [-1., 2., 1.]],
                [0.25, 0.8, 1., 1.],
            ),
            (
                [0., 0., -1.],
                [[1., 0., -1.], [-1., 0., -1.], [-1., 2., -1.], [1., 2., -1.]],
                [0.9, 0.4, 0.65, 1.],
            ),
            (
                [1., 0., 0.],
                [[1., 0., 1.], [1., 0., -1.], [1., 2., -1.], [1., 2., 1.]],
                [1., 0.6, 0.25, 1.],
            ),
            (
                [-1., 0., 0.],
                [[-1., 0., -1.], [-1., 0., 1.], [-1., 2., 1.], [-1., 2., -1.]],
                [0.4, 0.85, 0.5, 1.],
            ),
            (
                [0., 1., 0.],
                [[-1., 2., 1.], [1., 2., 1.], [1., 2., -1.], [-1., 2., -1.]],
                [1., 0.85, 0.4, 1.],
            ),
            (
                [0., -1., 0.],
                [[-1., 0., -1.], [1., 0., -1.], [1., 0., 1.], [-1., 0., 1.]],
                [0.7, 0.5, 0.9, 1.],
            ),
        ];
        for (normal, positions, color) in faces {
            add_quad(&mut cube, positions, normal, color, 2.);
        }
        let mut floor = Mesh {
            name: "floor".into(),
            joint: None,
            instance_id: None,
            source_occurrence: None,
            vertices: Vec::new(),
            indices: Vec::new(),
            material: Material {
                source_id: None,
                source_occurrence: None,
                texture_sources: Vec::new(),
                render_mode: None,
                pixel_engine: None,
                color: [0.2, 0.24, 0.32, 1.],
                texture: Some(0),
                cull_mode: CullMode::Back,
            },
            hidden: false,
        };
        add_quad(
            &mut floor,
            [
                [-4., -0.02, 4.],
                [4., -0.02, 4.],
                [4., -0.02, -4.],
                [-4., -0.02, -4.],
            ],
            [0., 1., 0.],
            [1.; 4],
            8.,
        );
        let mut rgba = Vec::new();
        for y in 0..8 {
            for x in 0..8 {
                let value = if (x / 2 + y / 2) % 2 == 0 { 255 } else { 165 };
                rgba.extend([value, value, value, 255]);
            }
        }
        Self {
            source: None,
            resources: Vec::new(),
            joints: Vec::new(),
            meshes: vec![cube, floor],
            textures: vec![Texture {
                name: "procedural checker".into(),
                width: 8,
                height: 8,
                rgba,
            }],
            warnings: Vec::new(),
            camera: None,
            clear_color: [0.018, 0.025, 0.045, 1.0],
        }
    }
}

fn load_joint(raw: RawJoint) -> Result<Joint> {
    let pose = match (raw.flags, raw.local, raw.world, raw.inverse_bind) {
        (None, None, None, None) => None,
        (Some(flags), Some(local), Some(world), Some(inverse_bind)) => {
            for (label, matrix) in [
                ("local", &local),
                ("world", &world),
                ("inverse_bind", &inverse_bind),
            ] {
                ensure!(
                    matrix.iter().all(|value| value.is_finite()),
                    "{}: nonfinite {label} joint matrix",
                    raw.name
                );
            }
            Some(JointPose {
                flags,
                local,
                world,
                inverse_bind,
            })
        }
        _ => bail!(
            "{}: joint pose metadata must provide flags, local, world, and inverse_bind together",
            raw.name
        ),
    };
    Ok(Joint {
        name: raw.name,
        offset: raw.offset,
        parent: raw.parent,
        pose,
    })
}

fn validate_joint_tree(joints: &[Joint]) -> Result<HashMap<u32, Option<u32>>> {
    let mut parents = HashMap::with_capacity(joints.len());
    for joint in joints {
        ensure!(
            parents.insert(joint.offset, joint.parent).is_none(),
            "duplicate source joint offset {}",
            joint.offset
        );
    }
    for joint in joints {
        if let Some(parent) = joint.parent {
            ensure!(
                parents.contains_key(&parent),
                "{} references missing parent joint {parent}",
                joint.name
            );
        }
        let mut seen = HashSet::new();
        let mut current = Some(joint.offset);
        while let Some(offset) = current {
            ensure!(seen.insert(offset), "cycle at source joint {offset}");
            current = parents[&offset];
        }
    }
    Ok(parents)
}

fn descends_from(joint: u32, roots: &[u32], parents: &HashMap<u32, Option<u32>>) -> bool {
    let mut current = Some(joint);
    while let Some(offset) = current {
        if roots.contains(&offset) {
            return true;
        }
        current = parents[&offset];
    }
    false
}

fn load_visual_resources(
    resources: Vec<RawVisualResource>,
) -> Result<(
    Vec<VisualResourceProvenance>,
    HashMap<String, VisualResourceId>,
)> {
    let mut loaded = Vec::with_capacity(resources.len());
    let mut ids = HashMap::with_capacity(resources.len());
    for resource in resources {
        ensure!(!resource.id.is_empty(), "visual resource ID is empty");
        ensure!(
            resource.sha256.len() == 64
                && resource
                    .sha256
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)),
            "visual resource {:?} SHA-256 must be exactly 64 lowercase hexadecimal digits",
            resource.id
        );
        let id = VisualResourceId(Arc::from(resource.id.as_str()));
        ensure!(
            ids.insert(resource.id.clone(), id.clone()).is_none(),
            "duplicate visual resource ID {:?}",
            resource.id
        );
        loaded.push(VisualResourceProvenance {
            id,
            sha256: Arc::from(resource.sha256),
        });
    }
    Ok((loaded, ids))
}

fn visual_dobj_occurrence(
    mesh: &RawMesh,
    resource_ids: &HashMap<String, VisualResourceId>,
) -> Result<Option<VisualDObjOccurrence>> {
    let (resource_id, dobj_index) = match (&mesh.resource_id, mesh.dobj_index) {
        (None, None) => return Ok(None),
        (Some(resource_id), Some(dobj_index)) => (resource_id, dobj_index),
        _ => bail!("resource_id and dobj_index must be present together"),
    };
    let resource_id = resource_ids
        .get(resource_id)
        .with_context(|| format!("undeclared visual resource {resource_id:?}"))?
        .clone();
    let descriptor_offset = mesh
        .joint
        .context("resource_id and dobj_index require a source joint")?;
    Ok(Some(VisualDObjOccurrence {
        owner_joint: VisualJointOccurrence {
            resource_id,
            visual_offset: descriptor_offset,
        },
        dobj_index,
    }))
}

fn validate_visual_joint_namespaces(
    meshes: &[RawMesh],
    resource_ids: &HashMap<String, VisualResourceId>,
) -> Result<()> {
    let mut resources_by_joint = HashMap::<u32, VisualResourceId>::new();
    for mesh in meshes {
        let Some(occurrence) = visual_dobj_occurrence(mesh, resource_ids)
            .with_context(|| format!("{}: invalid source occurrence", mesh.name))?
        else {
            continue;
        };
        let joint = &occurrence.owner_joint;
        if let Some(previous) =
            resources_by_joint.insert(joint.visual_offset, joint.resource_id.clone())
        {
            ensure!(
                previous == joint.resource_id,
                "source joint visual offset {} is shared by visual resources {:?} and {:?}; multi-resource joint topology is not yet representable",
                joint.visual_offset,
                previous.as_str(),
                joint.resource_id.as_str()
            );
        }
    }
    Ok(())
}

fn visual_texture_occurrence(
    dobj_occurrence: Option<&VisualDObjOccurrence>,
    material_occurrence: Option<&VisualMaterialOccurrence>,
    descriptor_offset: Option<TextureSourceId>,
    tobj_index: Option<u16>,
    stage_index: usize,
) -> Result<Option<VisualTextureOccurrence>> {
    let Some(owner_material) = material_occurrence else {
        if dobj_occurrence.is_some() {
            bail!("exact mesh texture stages require material_offset");
        } else {
            ensure!(
                tobj_index.is_none(),
                "tobj_index requires an exact mesh resource_id/dobj_index occurrence"
            );
        }
        return Ok(None);
    };
    let descriptor_offset = descriptor_offset
        .context("exact material texture stages require tobj_offset and tobj_index")?;
    let tobj_index =
        tobj_index.context("exact material texture stages require tobj_offset and tobj_index")?;
    ensure!(
        usize::from(tobj_index) == stage_index,
        "tobj_index {tobj_index} does not match texture-stage ordinal {stage_index}"
    );
    Ok(Some(VisualTextureOccurrence {
        owner_material: owner_material.clone(),
        tobj_index,
        visual_offset: descriptor_offset,
    }))
}

fn validate_attribute<const N: usize>(
    values: &[[f32; N]],
    count: usize,
    name: &str,
    attribute: &str,
) -> Result<()> {
    ensure!(
        values.len() == count,
        "{name}: {attribute} count {} differs from position count {count}",
        values.len()
    );
    ensure!(
        values.iter().flatten().all(|v| v.is_finite()),
        "{name}: nonfinite {attribute}"
    );
    Ok(())
}

fn pixel_engine_state(
    material: &Value,
    render_mode: Option<RenderMode>,
) -> Result<Option<PixelEngineState>> {
    const FIELDS: [&str; 9] = [
        "pe_flags",
        "explicit_descriptor",
        "color_write",
        "alpha_write",
        "destination_alpha",
        "blend",
        "depth",
        "alpha_test",
        "dither",
    ];
    let present = FIELDS
        .iter()
        .filter(|field| material.get(**field).is_some_and(|value| !value.is_null()))
        .count();
    if present == 0 {
        return Ok(render_mode.map(PixelEngineState::from_render_mode));
    }
    ensure!(
        present == FIELDS.len(),
        "pixel-engine fields must be all present or all absent ({present}/{} present)",
        FIELDS.len()
    );
    let mode = render_mode.context("pixel-engine fields require render_mode")?;
    let raw: RawPixelEngine =
        serde_json::from_value(material.clone()).context("decode pixel-engine fields")?;
    let state = PixelEngineState {
        explicit_descriptor: raw.explicit_descriptor,
        color_write: raw.color_write,
        alpha_write: raw.alpha_write,
        destination_alpha: PeDestinationAlpha {
            enabled: raw.destination_alpha.enabled,
            value: raw.destination_alpha.value,
        },
        blend: PeBlendState {
            mode: PeBlendMode::from_code(raw.blend.mode).context("invalid blend.type")?,
            source_factor: PeBlendFactor::from_code(raw.blend.src_factor)
                .context("invalid blend.src_factor")?,
            destination_factor: PeBlendFactor::from_code(raw.blend.dst_factor)
                .context("invalid blend.dst_factor")?,
            logic_operation: PeLogicOp::from_code(raw.blend.logic_op)
                .context("invalid blend.logic_op")?,
        },
        depth: PeDepthState {
            test_enabled: raw.depth.test,
            write_enabled: raw.depth.write,
            comparison: PeCompare::from_code(raw.depth.compare).context("invalid depth.compare")?,
            compare_before_texture: raw.depth.before_texture,
        },
        alpha_test: PeAlphaTest {
            comparison0: PeCompare::from_code(raw.alpha_test.compare0)
                .context("invalid alpha_test.compare0")?,
            reference0: raw.alpha_test.reference0,
            operation: PeAlphaOp::from_code(raw.alpha_test.op).context("invalid alpha_test.op")?,
            comparison1: PeCompare::from_code(raw.alpha_test.compare1)
                .context("invalid alpha_test.compare1")?,
            reference1: raw.alpha_test.reference1,
        },
        dither: raw.dither,
    };
    ensure!(
        state.flags() == raw.pe_flags,
        "pe_flags {:#04x} contradict decoded pixel-engine fields ({:#04x})",
        raw.pe_flags,
        state.flags()
    );
    if !state.explicit_descriptor {
        ensure!(
            state.same_behavior(PixelEngineState::from_render_mode(mode)),
            "non-explicit pixel-engine fields contradict render_mode defaults"
        );
    }
    state.validate_supported()?;
    Ok(Some(state))
}

fn optional_u32_field(value: &Value, field: &str) -> Result<Option<u32>> {
    let Some(value) = value.get(field).filter(|value| !value.is_null()) else {
        return Ok(None);
    };
    let integer = value
        .as_u64()
        .with_context(|| format!("{field} must be an unsigned integer"))?;
    Ok(Some(
        u32::try_from(integer).with_context(|| format!("{field} exceeds 32 bits"))?,
    ))
}

fn optional_u16_field(value: &Value, field: &str) -> Result<Option<u16>> {
    let Some(value) = value.get(field).filter(|value| !value.is_null()) else {
        return Ok(None);
    };
    let integer = value
        .as_u64()
        .with_context(|| format!("{field} must be an unsigned integer"))?;
    Ok(Some(
        u16::try_from(integer).with_context(|| format!("{field} exceeds 16 bits"))?,
    ))
}

fn cull_mode(value: &Value) -> Result<CullMode> {
    Ok(match (value.as_str(), value.as_u64()) {
        _ if value.is_null() => CullMode::None,
        (Some("none"), _) | (_, Some(0)) => CullMode::None,
        (Some("front"), _) | (_, Some(1)) => CullMode::Front,
        (Some("back"), _) | (_, Some(2)) => CullMode::Back,
        (Some("all"), _) | (_, Some(3)) => CullMode::All,
        _ => bail!("unknown value {value}"),
    })
}

fn unit_normal(value: [f64; 3]) -> [f32; 3] {
    let length = value.iter().map(|v| v * v).sum::<f64>().sqrt();
    if length > 0. {
        value.map(|v| (v / length) as f32)
    } else {
        [0., 1., 0.]
    }
}

fn generated_normals(positions: &[[f32; 3]], indices: &[u32]) -> Vec<[f32; 3]> {
    let mut normals = vec![[0_f64; 3]; positions.len()];
    for triangle in indices.as_chunks::<3>().0 {
        let [a, b, c] =
            [triangle[0], triangle[1], triangle[2]].map(|i| positions[i as usize].map(f64::from));
        let u = std::array::from_fn::<_, 3, _>(|i| b[i] - a[i]);
        let v = std::array::from_fn::<_, 3, _>(|i| c[i] - a[i]);
        let n = [
            u[1] * v[2] - u[2] * v[1],
            u[2] * v[0] - u[0] * v[2],
            u[0] * v[1] - u[1] * v[0],
        ];
        for &index in triangle {
            for (sum, value) in normals[index as usize].iter_mut().zip(n) {
                *sum += value;
            }
        }
    }
    normals.into_iter().map(unit_normal).collect()
}

fn load_texture(folder: &Path, raw: &RawTexture) -> Result<Texture> {
    let path = folder.join(&raw.path);
    let mut decoder = png::Decoder::new(BufReader::new(
        File::open(&path).with_context(|| format!("open texture {}", path.display()))?,
    ));
    decoder.set_limits(png::Limits {
        bytes: 128 * 1024 * 1024,
    });
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder
        .read_info()
        .with_context(|| format!("decode PNG {}", path.display()))?;
    let info = reader.info();
    ensure!(
        info.width == raw.width && info.height == raw.height,
        "{}: PNG dimensions differ from scene metadata",
        raw.id
    );
    ensure!(
        raw.width > 0
            && raw.height > 0
            && u64::from(raw.width) * u64::from(raw.height) <= 16 * 1024 * 1024,
        "{}: texture exceeds 16 million pixels",
        raw.id
    );
    let mut data = vec![
        0;
        reader
            .output_buffer_size()
            .context("PNG output size overflow")?
    ];
    let output = reader.next_frame(&mut data)?;
    ensure!(
        output.width == raw.width && output.height == raw.height,
        "{}: partial PNG animation frames are unsupported",
        raw.id
    );
    let data = &data[..output.buffer_size()];
    let mut rgba = Vec::with_capacity(raw.width as usize * raw.height as usize * 4);
    match output.color_type {
        png::ColorType::Rgba => rgba.extend_from_slice(data),
        png::ColorType::Rgb => {
            for p in data.as_chunks::<3>().0 {
                rgba.extend([p[0], p[1], p[2], 255]);
            }
        }
        png::ColorType::Grayscale => {
            for &v in data {
                rgba.extend([v, v, v, 255]);
            }
        }
        png::ColorType::GrayscaleAlpha => {
            for p in data.as_chunks::<2>().0 {
                rgba.extend([p[0], p[0], p[0], p[1]]);
            }
        }
        png::ColorType::Indexed => bail!("{}: PNG palette was not expanded", raw.id),
    }
    ensure!(
        rgba.len() == raw.width as usize * raw.height as usize * 4,
        "{}: invalid decoded RGBA byte count",
        raw.id
    );
    Ok(Texture {
        name: raw.id.clone(),
        width: raw.width,
        height: raw.height,
        rgba,
    })
}

fn add_quad(
    mesh: &mut Mesh,
    positions: [[f32; 3]; 4],
    normal: [f32; 3],
    color: [f32; 4],
    repeats: f32,
) {
    let start = mesh.vertices.len() as u32;
    for (position, uv) in
        positions
            .into_iter()
            .zip([[0., repeats], [repeats, repeats], [repeats, 0.], [0., 0.]])
    {
        mesh.vertices.push(Vertex {
            position,
            normal,
            uv,
            color,
        });
    }
    mesh.indices
        .extend([start, start + 1, start + 2, start, start + 2, start + 3]);
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};
    use std::fs;

    fn document(material: Value) -> Value {
        json!({
            "schema": "skirmish-visual-v1",
            "meshes": [{
                "name": "triangle",
                "positions": [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
                "indices": [0, 1, 2],
                "material": material,
            }],
        })
    }

    fn exact_document(material: Value) -> Value {
        let mut document = document(material);
        document["resources"] = json!([{
            "id": "fixture.dat",
            "sha256": "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
        }]);
        document["joints"] = json!([{"name": "root", "offset": 256}]);
        document["meshes"][0]["joint"] = json!(256);
        document["meshes"][0]["resource_id"] = json!("fixture.dat");
        document["meshes"][0]["dobj_index"] = json!(2);
        document
    }

    fn load_document(root: &Path, document: &Value) -> Result<Scene> {
        let path = root.join("scene.json");
        fs::write(&path, serde_json::to_vec(document)?)?;
        Scene::load(&path)
    }

    fn translucent_pe_material(explicit_descriptor: bool) -> Value {
        json!({
            "render_mode": 0x6000_0001_u32,
            "pe_flags": 25,
            "explicit_descriptor": explicit_descriptor,
            "color_write": true,
            "alpha_write": false,
            "destination_alpha": { "enabled": false, "value": 0 },
            "blend": { "type": 1, "src_factor": 4, "dst_factor": 5, "logic_op": 15 },
            "depth": { "test": true, "write": false, "compare": 3, "before_texture": true },
            "alpha_test": {
                "compare0": 7,
                "reference0": 0,
                "op": 0,
                "compare1": 7,
                "reference1": 0
            },
            "dither": false
        })
    }

    #[test]
    fn visual_source_material_texture_and_render_mode_metadata_is_retained() {
        let directory = tempfile::tempdir().unwrap();
        let material_offset = 0x6_a180;
        let texture_offset = 0x6_a110;
        let render_mode = 0x6000_0019;
        let scene = load_document(
            directory.path(),
            &document(json!({
                "material_offset": material_offset,
                "render_mode": render_mode,
                "textures": [{"tobj_offset": texture_offset}, {}],
            })),
        )
        .unwrap();

        let material = &scene.meshes[0].material;
        assert_eq!(
            material.source_id,
            Some(MaterialSourceId::new(material_offset))
        );
        assert_eq!(material.texture_sources.len(), 2);
        assert_eq!(
            material.texture_sources[0].visual_offset,
            Some(TextureSourceId::new(texture_offset))
        );
        assert_eq!(material.texture_sources[0].occurrence, None);
        assert_eq!(material.texture_sources[1].visual_offset, None);
        assert_eq!(material.texture_sources[1].occurrence, None);
        let mode = material.render_mode.unwrap();
        assert_eq!(mode.bits(), render_mode);
        assert_eq!(mode.class(), RenderModeClass::Translucent);
        assert_eq!(
            material.pixel_engine,
            Some(PixelEngineState::from_render_mode(mode))
        );
        assert_eq!(material.texture, None, "identity survives a missing PNG");
        assert!(scene.resources.is_empty());
        assert_eq!(scene.meshes[0].source_occurrence, None);
        assert_eq!(material.source_occurrence, None);

        let legacy = load_document(directory.path(), &document(json!({}))).unwrap();
        assert_eq!(legacy.meshes[0].material.source_id, None);
        assert!(legacy.meshes[0].material.texture_sources.is_empty());
        assert_eq!(legacy.meshes[0].material.render_mode, None);
        assert_eq!(legacy.meshes[0].material.pixel_engine, None);
        assert!(legacy.resources.is_empty());
        assert_eq!(legacy.meshes[0].source_occurrence, None);
    }

    #[test]
    fn exact_visual_resource_and_occurrence_metadata_is_retained() {
        let directory = tempfile::tempdir().unwrap();
        let material_offset = 0x440;
        let texture_offset = 0x480;
        let scene = load_document(
            directory.path(),
            &exact_document(json!({
                "material_offset": material_offset,
                "textures": [
                    {"tobj_offset": texture_offset, "tobj_index": 0},
                    {"tobj_offset": 0x4c0, "tobj_index": 1},
                ],
            })),
        )
        .unwrap();

        assert_eq!(scene.resources.len(), 1);
        assert_eq!(scene.resources[0].id().as_str(), "fixture.dat");
        assert_eq!(
            scene.resources[0].sha256(),
            "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
        );
        let mesh = &scene.meshes[0];
        let dobj = mesh.source_occurrence.as_ref().unwrap();
        assert_eq!(dobj.owner_joint.resource_id.as_str(), "fixture.dat");
        assert_eq!(dobj.owner_joint.visual_offset, 256);
        assert_eq!(dobj.dobj_index, 2);

        let material = mesh.material.source_occurrence.as_ref().unwrap();
        assert_eq!(&material.owner_dobj, dobj);
        assert_eq!(
            material.visual_offset,
            MaterialSourceId::new(material_offset)
        );
        assert_eq!(mesh.material.texture_sources.len(), 2);
        assert_eq!(
            mesh.material.texture_sources[0].visual_offset,
            Some(TextureSourceId::new(texture_offset))
        );
        let texture = mesh.material.texture_sources[0]
            .occurrence
            .as_ref()
            .unwrap();
        assert_eq!(&texture.owner_material, material);
        assert_eq!(texture.tobj_index, 0);
        assert_eq!(texture.visual_offset, TextureSourceId::new(texture_offset));
        assert_eq!(
            mesh.material.texture_sources[1].visual_offset,
            Some(TextureSourceId::new(0x4c0))
        );
        let second = mesh.material.texture_sources[1]
            .occurrence
            .as_ref()
            .unwrap();
        assert_eq!(second.tobj_index, 1);
        assert_eq!(second.visual_offset, TextureSourceId::new(0x4c0));
    }

    #[test]
    fn malformed_visual_resource_and_occurrence_metadata_is_rejected() {
        let directory = tempfile::tempdir().unwrap();
        let mut empty_resource = exact_document(json!({}));
        empty_resource["resources"][0]["id"] = json!("");
        let mut invalid_hash = exact_document(json!({}));
        invalid_hash["resources"][0]["sha256"] =
            json!("0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdeF");
        let mut duplicate_resource = exact_document(json!({}));
        let duplicate = duplicate_resource["resources"][0].clone();
        duplicate_resource["resources"]
            .as_array_mut()
            .unwrap()
            .push(duplicate);
        let mut duplicate_joint = exact_document(json!({}));
        let duplicate = duplicate_joint["joints"][0].clone();
        duplicate_joint["joints"]
            .as_array_mut()
            .unwrap()
            .push(duplicate);
        let mut colliding_resource_joint = exact_document(json!({}));
        colliding_resource_joint["resources"]
            .as_array_mut()
            .unwrap()
            .push(json!({
                "id": "other.dat",
                "sha256": "abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789",
            }));
        let mut colliding_mesh = colliding_resource_joint["meshes"][0].clone();
        colliding_mesh["name"] = json!("other triangle");
        colliding_mesh["resource_id"] = json!("other.dat");
        colliding_mesh["dobj_index"] = json!(3);
        colliding_resource_joint["meshes"]
            .as_array_mut()
            .unwrap()
            .push(colliding_mesh);
        let mut missing_dobj = exact_document(json!({}));
        missing_dobj["meshes"][0]
            .as_object_mut()
            .unwrap()
            .remove("dobj_index");
        let mut missing_resource = exact_document(json!({}));
        missing_resource["meshes"][0]
            .as_object_mut()
            .unwrap()
            .remove("resource_id");
        let mut missing_joint = exact_document(json!({}));
        missing_joint["meshes"][0]
            .as_object_mut()
            .unwrap()
            .remove("joint");
        let mut unknown_resource = exact_document(json!({}));
        unknown_resource["meshes"][0]["resource_id"] = json!("other.dat");
        let partial_texture = exact_document(json!({
            "material_offset": 0x440,
            "textures": [{"tobj_offset": 0x480}],
        }));
        let missing_texture_identity = exact_document(json!({
            "material_offset": 0x440,
            "textures": [{}],
        }));
        let mut missing_texture_offset = exact_document(json!({
            "material_offset": 0x440,
            "textures": [{"tobj_offset": 0x480, "tobj_index": 0}],
        }));
        missing_texture_offset["meshes"][0]["material"]["textures"][0]
            .as_object_mut()
            .unwrap()
            .remove("tobj_offset");
        let wrong_texture_index = exact_document(json!({
            "material_offset": 0x440,
            "textures": [{"tobj_offset": 0x480, "tobj_index": 1}],
        }));
        let texture_without_material = exact_document(json!({
            "textures": [{"tobj_offset": 0x480, "tobj_index": 0}],
        }));
        let anonymous_texture_without_material = exact_document(json!({
            "textures": [{}],
        }));

        let cases = [
            (empty_resource, "resource ID is empty"),
            (invalid_hash, "64 lowercase hexadecimal digits"),
            (duplicate_resource, "duplicate visual resource ID"),
            (duplicate_joint, "duplicate source joint offset"),
            (colliding_resource_joint, "shared by visual resources"),
            (
                missing_dobj,
                "resource_id and dobj_index must be present together",
            ),
            (
                missing_resource,
                "resource_id and dobj_index must be present together",
            ),
            (missing_joint, "require a source joint"),
            (unknown_resource, "undeclared visual resource"),
            (partial_texture, "require tobj_offset and tobj_index"),
            (
                missing_texture_identity,
                "require tobj_offset and tobj_index",
            ),
            (missing_texture_offset, "require tobj_offset and tobj_index"),
            (wrong_texture_index, "does not match texture-stage ordinal"),
            (texture_without_material, "require material_offset"),
            (
                anonymous_texture_without_material,
                "require material_offset",
            ),
        ];
        for (document, expected) in cases {
            let error = load_document(directory.path(), &document).unwrap_err();
            assert!(
                format!("{error:#}").contains(expected),
                "unexpected error: {error:#}"
            );
        }
    }

    #[test]
    fn complete_exported_pixel_engine_state_is_retained() {
        let directory = tempfile::tempdir().unwrap();
        let mut material = translucent_pe_material(true);
        material["blend"]["dst_factor"] = json!(1);
        material["depth"]["before_texture"] = json!(false);
        material["pe_flags"] = json!(17);
        material["alpha_test"] = json!({
            "compare0": 6,
            "reference0": 102,
            "op": 0,
            "compare1": 3,
            "reference1": 255
        });

        let scene = load_document(directory.path(), &document(material)).unwrap();
        let state = scene.meshes[0].material.pixel_engine.unwrap();
        assert!(state.explicit_descriptor);
        assert_eq!(state.blend.destination_factor, PeBlendFactor::One);
        assert_eq!(state.depth.comparison, PeCompare::LessEqual);
        assert!(!state.depth.write_enabled);
        assert!(!state.depth.compare_before_texture);
        assert!(!state.alpha_test.passes(101));
        assert!(state.alpha_test.passes(102));
        assert!(state.alpha_test.passes(103));
    }

    #[test]
    fn pixel_engine_fields_are_atomic_and_consistent() {
        let directory = tempfile::tempdir().unwrap();
        let partial = json!({"render_mode": 0x6000_0001_u32, "pe_flags": 25});
        let mut missing_mode = translucent_pe_material(false);
        missing_mode.as_object_mut().unwrap().remove("render_mode");
        let mut invalid_factor = translucent_pe_material(false);
        invalid_factor["blend"]["src_factor"] = json!(8);
        let mut contradictory_flags = translucent_pe_material(false);
        contradictory_flags["pe_flags"] = json!(24);
        let mut contradictory_default = translucent_pe_material(false);
        contradictory_default["blend"]["dst_factor"] = json!(1);
        let mut wrong_type = translucent_pe_material(false);
        wrong_type["depth"]["compare"] = json!("lequal");

        let cases = [
            (partial, "all present or all absent"),
            (missing_mode, "require render_mode"),
            (invalid_factor, "blend factor code 8"),
            (contradictory_flags, "pe_flags"),
            (contradictory_default, "contradict render_mode defaults"),
            (wrong_type, "decode pixel-engine fields"),
        ];
        for (material, expected) in cases {
            let error = load_document(directory.path(), &document(material.clone())).unwrap_err();
            assert!(
                format!("{error:#}").contains(expected),
                "unexpected error for {material}: {error:#}"
            );
        }
    }

    #[test]
    fn unsupported_pixel_engine_features_are_rejected_explicitly() {
        let directory = tempfile::tempdir().unwrap();
        let mut destination_alpha = translucent_pe_material(true);
        destination_alpha["destination_alpha"]["enabled"] = json!(true);
        destination_alpha["pe_flags"] = json!(29);
        let mut dither = translucent_pe_material(true);
        dither["dither"] = json!(true);
        dither["pe_flags"] = json!(89);
        let mut logic = translucent_pe_material(true);
        logic["blend"]["type"] = json!(2);
        let mut early_reject = translucent_pe_material(true);
        early_reject["depth"]["write"] = json!(true);
        early_reject["pe_flags"] = json!(57);
        early_reject["alpha_test"]["compare0"] = json!(4);

        let cases = [
            (destination_alpha, "destination-alpha"),
            (dither, "dithering"),
            (logic, "logic blending"),
            (early_reject, "depth-before-texture"),
        ];
        for (material, expected) in cases {
            let error = load_document(directory.path(), &document(material)).unwrap_err();
            assert!(
                format!("{error:#}").contains(expected),
                "unexpected error: {error:#}"
            );
        }
    }

    #[test]
    fn gx_alpha_comparisons_and_boolean_operations_use_u8_values() {
        let comparisons = [
            (PeCompare::Never, false),
            (PeCompare::Less, false),
            (PeCompare::Equal, true),
            (PeCompare::LessEqual, true),
            (PeCompare::Greater, false),
            (PeCompare::NotEqual, false),
            (PeCompare::GreaterEqual, true),
            (PeCompare::Always, true),
        ];
        for (comparison, expected_at_equal) in comparisons {
            assert_eq!(comparison.test(102, 102), expected_at_equal);
        }

        let test = |operation| PeAlphaTest {
            comparison0: PeCompare::GreaterEqual,
            reference0: 102,
            operation,
            comparison1: PeCompare::LessEqual,
            reference1: 102,
        };
        assert!(test(PeAlphaOp::And).passes(102));
        assert!(test(PeAlphaOp::Or).passes(101));
        assert!(!test(PeAlphaOp::Xor).passes(102));
        assert!(test(PeAlphaOp::Xnor).passes(102));

        let inputs = [(false, false), (false, true), (true, false), (true, true)];
        let operations = [
            (PeAlphaOp::And, [false, false, false, true]),
            (PeAlphaOp::Or, [false, true, true, true]),
            (PeAlphaOp::Xor, [false, true, true, false]),
            (PeAlphaOp::Xnor, [true, false, false, true]),
        ];
        for (operation, expected) in operations {
            assert_eq!(
                inputs.map(|(left, right)| operation.combine(left, right)),
                expected
            );
        }
    }

    #[test]
    fn render_mode_is_canonical_for_initial_vertex_channel_ownership() {
        let directory = tempfile::tempdir().unwrap();
        let cases = [
            (
                "vertex color, material alpha",
                0x0000_2002,
                [1.0, 1.0, 1.0, 0.5],
                [0.6, 0.7, 0.8, 1.0],
            ),
            (
                "material color, vertex alpha",
                0x0000_4001,
                [0.2, 0.3, 0.4, 1.0],
                [1.0, 1.0, 1.0, 0.9],
            ),
            (
                "vertex alpha inherits vertex diffuse mode",
                0x0000_0002,
                [1.0; 4],
                [0.6, 0.7, 0.8, 0.9],
            ),
        ];

        for (label, render_mode, expected_material, expected_vertex) in cases {
            let mut source = document(json!({
                "render_mode": render_mode,
                "diffuse": [0.2, 0.3, 0.4, 1.0],
                "material_alpha": 0.5,
            }));
            source["meshes"][0]["colors0"] = json!([
                [0.6, 0.7, 0.8, 0.9],
                [0.6, 0.7, 0.8, 0.9],
                [0.6, 0.7, 0.8, 0.9],
            ]);

            let scene = load_document(directory.path(), &source).unwrap();
            assert_eq!(scene.meshes[0].material.color, expected_material, "{label}");
            assert_eq!(
                scene.meshes[0].vertices[0].color, expected_vertex,
                "{label}"
            );
        }
    }

    #[test]
    fn malformed_visual_source_identity_and_render_mode_metadata_is_rejected() {
        let directory = tempfile::tempdir().unwrap();
        let cases = [
            (
                "material type",
                json!({"material_offset": "430464"}),
                "material_offset must be an unsigned integer",
            ),
            (
                "material width",
                json!({"material_offset": u64::from(u32::MAX) + 1}),
                "material_offset exceeds 32 bits",
            ),
            (
                "reserved pass",
                json!({"render_mode": 0x2000_0000_u32}),
                "reserved pass bits",
            ),
            (
                "vertex color contradiction",
                json!({"render_mode": 0x0000_0002_u32, "uses_vertex_color": false}),
                "uses_vertex_color contradicts render_mode",
            ),
            (
                "vertex alpha contradiction",
                json!({"render_mode": 0x0000_4001_u32, "uses_vertex_alpha": false}),
                "uses_vertex_alpha contradicts render_mode",
            ),
            (
                "texture type",
                json!({"textures": [{"tobj_offset": -1}]}),
                "tobj_offset must be an unsigned integer",
            ),
            (
                "texture stage type",
                json!({"textures": [7]}),
                "texture stage must be an object",
            ),
        ];

        for (label, material, expected) in cases {
            let error = load_document(directory.path(), &document(material)).unwrap_err();
            assert!(
                format!("{error:#}").contains(expected),
                "unexpected {label} error: {error:#}"
            );
        }
    }

    #[test]
    fn render_mode_vertex_channel_sources_follow_hsd_fallbacks() {
        let expected_vertex_alpha = [
            [false, false, true, true],
            [false, false, true, true],
            [true, false, true, true],
            [true, false, true, true],
        ];

        for diffuse in 0_u32..4 {
            for alpha in 0_u32..4 {
                let mode = RenderMode::from_bits(diffuse | (alpha << 13)).unwrap();
                assert_eq!(
                    mode.uses_vertex_color(),
                    diffuse & 0x2 != 0,
                    "diffuse mode {diffuse}, alpha mode {alpha}"
                );
                assert_eq!(
                    mode.uses_vertex_alpha(),
                    expected_vertex_alpha[diffuse as usize][alpha as usize],
                    "diffuse mode {diffuse}, alpha mode {alpha}"
                );
            }
        }
    }
}
