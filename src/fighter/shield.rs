//! Ordinary shield arithmetic from ftCo_Guard.c and Fighter_ProcessHit_8006D1EC.
//! Host operation order is retained; this does not certify paired-single or
//! animation-pipeline equivalence. Coefficient pairs are [light, hard].

fn blend(amount: f32, endpoints: [f32; 2]) -> f32 {
    amount * (endpoints[1] - endpoints[0]) + endpoints[0]
}

/// ftcoll.c getEnvDmg, for bounded finite stored HitCapsule damage. Nonzero
/// fractions that truncate to zero become one for shield damage and hitlag.
pub fn environment_damage(damage: f32) -> i32 {
    if damage == 0.0 {
        0
    } else if damage as i32 == 0 {
        1
    } else {
        damage as i32
    }
}

/// ftCo_800921DC/800925A4: released analog input retains the previous strength.
pub fn strength(trigger: f32, deadzone: f32, previous: f32) -> f32 {
    let value = (trigger - deadzone) / (1.0 - deadzone);
    if value < 0.0 { previous } else { value }
}

/// ftCo_Guard.c inlineB0, ordinary non-Yoshi branch.
pub fn radius(
    health: f32,
    maximum: f32,
    amount: f32,
    sizes: [f32; 2],
    minimum: f32,
    initial: f32,
) -> f32 {
    let n1 = (health / maximum) * blend(amount, sizes);
    let n2 = 1.0 - minimum;
    (n2 * n1 + minimum) * initial
}

/// ftCo_800925A4: zero is not broken until a later subtraction goes below zero.
pub fn drain(health: f32, amount: f32, scales: [f32; 2], rate: f32) -> (f32, bool) {
    let health = health - rate * blend(amount, scales);
    if health < 0.0 {
        (0.0, true)
    } else {
        (health, false)
    }
}

/// Fighter_ProcessHit_8006D1EC, including its per-process base subtraction.
pub fn damage_loss(damage: i32, amount: f32, scales: [f32; 2], multiplier: f32, base: f32) -> f32 {
    multiplier * (damage as f32 * (1.0 - blend(amount, scales))) + base
}

/// ftCo_80092ED8. Stun scales an animation's playback rate; it is not rounded
/// to an integer countdown by the original routine.
pub fn stun(damage: i32, amount: f32, scales: [f32; 2], multiplier: f32, base: f32) -> f32 {
    multiplier * (damage as f32 * (1.0 - blend(amount, scales))) + base
}

/// ftCo_80092F2C, ordinary non-parry/non-electric shield response.
pub fn response(
    stun: f32,
    animation_end: f32,
    push_scale: f32,
    normal_multiplier: f32,
    maximum: f32,
    hit_facing: f32,
) -> (f32, f32) {
    let rate = (0.1 + animation_end) / stun;
    let mut velocity = stun * push_scale;
    velocity *= normal_multiplier;
    if velocity > maximum {
        velocity = maximum;
    }
    (
        rate,
        if hit_facing < 0.0 {
            velocity
        } else {
            -velocity
        },
    )
}

/// ftCo_80093240/800932DC, horizontal displacement along the floor tangent.
pub fn displacement(
    position: &mut [f32; 2],
    floor_normal: [f32; 3],
    stick_x: f32,
    minimum: f32,
    distance: f32,
    multiplier: f32,
) -> bool {
    if stick_x.abs() >= minimum {
        let scale = multiplier * (stick_x * distance);
        position[0] += floor_normal[1] * scale;
        position[1] += -floor_normal[0] * scale;
        true
    } else {
        false
    }
}

/// ftCommon_GrabMash's ordinary non-shake branch. Neutral input retains the
/// previous directional bucket; simultaneous button and direction changes count
/// twice, while any number of pressed buttons counts once.
pub fn mash(
    timer: &mut f32,
    directions: &mut [i8; 2],
    stick: [f32; 2],
    pressed: u16,
    threshold: f32,
    amount: f32,
) -> bool {
    let mut result = false;
    if pressed & (0x100 | 0x200 | 0x400 | 0x800 | 0x20 | 0x40) != 0 {
        *timer -= amount;
        result = true;
    }
    let previous = *directions;
    for axis in 0..2 {
        if stick[axis] < -threshold {
            directions[axis] = -1;
        }
        if stick[axis] > threshold {
            directions[axis] = 1;
        }
    }
    if previous != *directions {
        *timer -= amount;
        result = true;
    }
    result
}
