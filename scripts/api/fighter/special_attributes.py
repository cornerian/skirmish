"""Typed ids for native fighter-specific special attributes."""

from enum import IntEnum


class _Attribute(IntEnum):
    def __new__(cls, field_id: int, layout: int):
        if isinstance(field_id, bool) or not isinstance(field_id, int):
            raise TypeError("special attribute field id must be an integer")
        if not 0 <= field_id <= 0xFFFF:
            raise ValueError("special attribute field id must fit u16")
        if isinstance(layout, bool) or not isinstance(layout, int):
            raise TypeError("special attribute layout must be an integer")
        if not 0 <= layout <= 0xFF:
            raise ValueError("special attribute layout must fit u8")
        value = (layout << 16) | field_id
        member = int.__new__(cls, value)
        member._value_ = value
        member.field_id = field_id
        member.layout = layout
        return member


class DonkeyKongAttribute(_Attribute):
    SPECIAL_HI_AERIAL_VERTICAL_VELOCITY = (0, 3)
    SPECIAL_HI_AERIAL_GRAVITY = (1, 3)
    SPECIAL_HI_GROUNDED_HORIZONTAL_VELOCITY = (2, 3)
    SPECIAL_HI_AERIAL_HORIZONTAL_VELOCITY = (3, 3)
    SPECIAL_HI_GROUNDED_MOBILITY = (4, 3)
    SPECIAL_HI_AERIAL_MOBILITY = (5, 3)
    SPECIAL_HI_LANDING_LAG = (6, 3)


class SamusAttribute(_Attribute):
    # The native ftSamus table is consumed by all four special roots.  The
    # ids below are the compact script-resource ids; source byte offsets are
    # retained in comments so adding a declaration cannot silently change the
    # wire contract of the existing Screw Attack fields.
    SPECIAL_N_CHARGE_RATE = (7, 4)  # ftSamusAttributes::x18
    SPECIAL_N_RELEASE_VELOCITY = (8, 4)  # x1C
    SPECIAL_N_CHARGE_FRAME_LIMIT = (9, 4)  # x20
    SPECIAL_N_AERIAL_LANDING_LAG = (10, 4)  # x24
    SPECIAL_N_ANIMATION_THRESHOLD = (11, 4)  # x28
    SPECIAL_N_GROUND_VELOCITY_DIVISOR = (12, 4)  # x2C
    SPECIAL_N_AERIAL_FRICTION = (13, 4)  # x30
    SPECIAL_N_ARTICLE_OFFSET = (14, 4)  # x34

    SPECIAL_S_ANIMATION_THRESHOLD = (15, 4)  # x28
    SPECIAL_S_VELOCITY_DIVISOR = (16, 4)  # x2C
    SPECIAL_S_AERIAL_FRICTION = (17, 4)  # x30

    SPECIAL_LW_AERIAL_LAUNCH_VELOCITY = (18, 4)  # x54
    SPECIAL_LW_GROUND_TO_AIR_VERTICAL_VELOCITY = (19, 4)  # x58
    SPECIAL_LW_GROUND_SPEED_CLAMP = (20, 4)  # x5C
    SPECIAL_LW_AERIAL_SPEED_CLAMP = (21, 4)  # x60
    SPECIAL_LW_GROUND_STEERING = (22, 4)  # x64
    SPECIAL_LW_AERIAL_STEERING = (23, 4)  # x68
    SPECIAL_LW_GRAVITY_MULTIPLIER = (24, 4)  # x6C
    SPECIAL_LW_ARTICLE_OFFSET = (25, 4)  # x74
    SPECIAL_LW_STICK_THRESHOLD = (26, 4)  # x80

    # Descriptive aliases used by resource authors who prefer the source
    # action names over the compact SPECIAL_* spelling.
    CHARGE_RATE = SPECIAL_N_CHARGE_RATE
    CHARGE_FRAME_LIMIT = SPECIAL_N_CHARGE_FRAME_LIMIT
    SIDE_VELOCITY_DIVISOR = SPECIAL_S_VELOCITY_DIVISOR
    BOMB_AERIAL_LAUNCH_VELOCITY = SPECIAL_LW_AERIAL_LAUNCH_VELOCITY
    BOMB_VERTICAL_VELOCITY = SPECIAL_LW_GROUND_TO_AIR_VERTICAL_VELOCITY
    BOMB_HORIZONTAL_MULTIPLIER = (25, 4)  # x70

    SCREW_ATTACK_LAUNCH_HORIZONTAL_VELOCITY = (0, 4)
    # Source names these adjacent fields x3C (steering acceleration) and x40
    # (steering target/clamp); keep the old spelling as an alias below for
    # callers that used the initial exported table.
    SCREW_ATTACK_STEERING_ACCELERATION = (1, 4)
    SCREW_ATTACK_HORIZONTAL_CLAMP = (2, 4)
    SCREW_ATTACK_AERIAL_LAUNCH_VELOCITY = (3, 4)
    SCREW_ATTACK_LANDING_TRANSITION_MULTIPLIER = (4, 4)
    SCREW_ATTACK_TURNAROUND_STICK_THRESHOLD = (5, 4)
    SCREW_ATTACK_LANDING_LAG = (6, 4)
    SCREW_ATTACK_AERIAL_FRICTION = SCREW_ATTACK_STEERING_ACCELERATION


class JigglypuffPoundAttribute(_Attribute):
    STICK_ANGLE_MIN = (0, 5)
    STICK_ANGLE_MAX = (1, 5)
    MAX_LAUNCH_ANGLE = (2, 5)
    LAUNCH_SPEED = (3, 5)
    VELOCITY_MULTIPLIER = (4, 5)


class JigglypuffRolloutAttribute(_Attribute):
    """Typed view of the native ``ftPurinAttributes`` Rollout fields.

    The field ids are the compact script-resource ids.  The comments retain
    the source member names so resources can be populated from the pinned
    decomp without copying tuning values into the script.
    """

    CHARGE_INITIAL = (0, 6)  # ftPurinAttributes::xA0
    CHARGE_MAX = (1, 6)  # xA4
    CHARGE_RATE = (2, 6)  # xA8
    CHARGE_ANGLE_DEGREES = (3, 6)  # xAC
    TURN_DECELERATION = (4, 6)  # x6C
    TURN_EFFECT_INTERVAL = (5, 6)  # x70
    GROUND_RELEASE_OFFSET = (6, 6)  # xB8
    CHARGE_DECAY = (7, 6)  # xB4
    RELEASE_VELOCITY = (8, 6)  # xC0
    GROUND_SPEED_LIMIT = (9, 6)  # x4C
    GROUND_SPEED_CAP = (10, 6)  # x50
    GROUND_SLOPE_INFLUENCE = (11, 6)  # xC8
    AIR_DECELERATION = (12, 6)  # x58
    AIR_MIN_SPEED = (13, 6)  # x5C
    HIT_SPEED_THRESHOLD = (14, 6)  # xCC
    DAMAGE_BASE = (15, 6)  # x80
    DAMAGE_SPEED_SCALE = (16, 6)  # x84
    HIT_TOGGLE_PERIOD = (17, 6)  # x9C
    WALL_SPEED_SCALE = (18, 6)  # xD4
    TURN_STICK_THRESHOLD = (19, 6)  # x68

class KirbyAttribute(_Attribute):
    """Typed fields from ``ftKb_DatAttrs``'s Stone block."""

    STONE_MAX_TIME = (59, 6)  # +0xEC
    STONE_MIN_TIME = (60, 6)  # +0xF0
    STONE_MIN_SLANT_ANGLE = (61, 6)  # +0xF4
    STONE_MAX_SLANT_ANGLE = (62, 6)  # +0xF8
    STONE_SLIDE_ACCELERATION = (63, 6)  # +0xFC
    STONE_SLIDE_MAX_SPEED = (64, 6)  # +0x100
    STONE_GRAVITY = (65, 6)  # +0x104
    STONE_HP = (66, 6)  # +0x108
    STONE_RESISTANCE = (67, 6)  # +0x10C
    STONE_FREEFALL_TOGGLE = (69, 6)  # +0x114
