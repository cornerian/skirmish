//! Dolphin `PadClamp.c`: exact integer input conditioning without device access.
/// Scalar layout of Dolphin PADStatus. Error zero denotes a successful sample.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[repr(C)]
pub struct PadStatus {
    pub buttons: u16,
    pub stick: [i8; 2],
    pub substick: [i8; 2],
    pub triggers: [u8; 2],
    pub analog: [u8; 2],
    pub error: i8,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct StickRegion {
    pub deadzone: i8,
    pub maximum: i8,
    pub corner: i8,
}

/// Owned replacement for the original mutable global ClampRegion.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct ClampRegion {
    pub trigger_minimum: u8,
    pub trigger_maximum: u8,
    pub stick: StickRegion,
    pub substick: StickRegion,
}

pub const DEFAULT_REGION: ClampRegion = ClampRegion {
    trigger_minimum: 30,
    trigger_maximum: 180,
    stick: StickRegion {
        deadzone: 15,
        maximum: 72,
        corner: 40,
    },
    substick: StickRegion {
        deadzone: 15,
        maximum: 59,
        corner: 31,
    },
};

impl Default for ClampRegion {
    fn default() -> Self {
        DEFAULT_REGION
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ZeroDivisor;

impl core::fmt::Display for ZeroDivisor {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("custom stick clamp region causes integer division by zero")
    }
}

impl core::error::Error for ZeroDivisor {}

impl StickRegion {
    /// Original `ClampStick`, including promotion before abs(-128), truncating
    /// integer division, and narrowing to signed bytes before restoring signs.
    pub fn clamp(&self, stick: [i8; 2]) -> Result<[i8; 2], ZeroDivisor> {
        let signs = stick.map(|axis| if axis < 0 { -1 } else { 1 });
        let [mut x, mut y] =
            stick.map(|axis| (i32::from(axis).abs() - i32::from(self.deadzone)).max(0));
        if x == 0 && y == 0 {
            return Ok([0, 0]);
        }
        let maximum = i32::from(self.maximum);
        let corner = i32::from(self.corner);
        let divisor = if corner * y <= corner * x {
            corner * x + (maximum - corner) * y
        } else {
            corner * y + (maximum - corner) * x
        };
        let limit = corner * maximum;
        if limit < divisor {
            if divisor == 0 {
                return Err(ZeroDivisor);
            }
            x = i32::from((limit * x / divisor) as i8);
            y = i32::from((limit * y / divisor) as i8);
        }
        Ok([(signs[0] * x) as i8, (signs[1] * y) as i8])
    }
}

impl ClampRegion {
    /// Original `ClampTrigger`. A custom reversed range retains C's byte wrap.
    pub fn clamp_trigger(&self, trigger: u8) -> u8 {
        if trigger <= self.trigger_minimum {
            0
        } else {
            trigger
                .min(self.trigger_maximum)
                .wrapping_sub(self.trigger_minimum)
        }
    }

    /// Original `PADClamp` using owned calibration. Invalid custom calibration
    /// returns an error before committing any changes to the four statuses.
    pub fn apply(&self, statuses: &mut [PadStatus; 4]) -> Result<(), ZeroDivisor> {
        let mut next = *statuses;
        for status in &mut next {
            if status.error == 0 {
                status.stick = self.stick.clamp(status.stick)?;
                status.substick = self.substick.clamp(status.substick)?;
                status.triggers = status.triggers.map(|trigger| self.clamp_trigger(trigger));
            }
        }
        *statuses = next;
        Ok(())
    }
}

/// Original `PADClamp` with its fixed default ClampRegion.
pub fn clamp(statuses: &mut [PadStatus; 4]) {
    DEFAULT_REGION
        .apply(statuses)
        .expect("default clamp region has no zero divisors");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_gate_and_deadzone_examples() {
        let region = DEFAULT_REGION;
        assert_eq!(region.stick.clamp([15, -15]), Ok([0, 0]));
        assert_eq!(region.stick.clamp([16, -16]), Ok([1, -1]));
        assert_eq!(region.stick.clamp([127, -128]), Ok([39, -40]));
        assert_eq!(region.stick.clamp([127, 0]), Ok([72, 0]));
        assert_eq!(region.substick.clamp([0, -128]), Ok([0, -59]));
        assert_eq!(region.clamp_trigger(30), 0);
        assert_eq!(region.clamp_trigger(31), 1);
        assert_eq!(region.clamp_trigger(255), 150);
    }

    #[test]
    fn controller_errors_preserve_all_sample_fields() {
        let sample = PadStatus {
            buttons: 0xFFFF,
            stick: [-128, 127],
            substick: [127, -128],
            triggers: [255; 2],
            analog: [254, 253],
            error: -1,
        };
        let mut statuses = [sample; 4];
        statuses[0].error = 0;
        clamp(&mut statuses);
        assert_eq!(statuses[1..], [sample; 3]);
        assert_eq!(statuses[0].buttons, sample.buttons);
        assert_eq!(statuses[0].analog, sample.analog);
        assert_eq!(statuses[0].triggers, [150; 2]);
    }

    #[test]
    fn invalid_custom_region_preserves_the_entire_batch() {
        let region = ClampRegion {
            stick: StickRegion {
                deadzone: 0,
                maximum: -1,
                corner: 1,
            },
            ..DEFAULT_REGION
        };
        assert_eq!(region.stick.clamp([2, 1]), Err(ZeroDivisor));
        let mut statuses = [PadStatus {
            triggers: [255; 2],
            ..PadStatus::default()
        }; 4];
        statuses[1].stick = [2, 1];
        let before = statuses;
        assert_eq!(region.apply(&mut statuses), Err(ZeroDivisor));
        assert_eq!(statuses, before);
    }

    #[test]
    fn reversed_trigger_range_uses_byte_arithmetic() {
        let region = ClampRegion {
            trigger_minimum: 200,
            trigger_maximum: 30,
            ..DEFAULT_REGION
        };
        assert_eq!(region.clamp_trigger(200), 0);
        assert_eq!(region.clamp_trigger(201), 86);
    }
}
