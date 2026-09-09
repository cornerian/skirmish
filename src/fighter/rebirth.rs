//! Exact leader velocity update retained from `ftCo_Rebirth_Phys`.

/// Approach the current rebirth-platform target over the remaining callbacks.
pub fn approach_velocity(current: [f32; 2], target: [f32; 2], remaining: u32) -> [f32; 2] {
    let inverse = 1.0 / remaining as f32;
    [
        (target[0] - current[0]) * inverse,
        (target[1] - current[1]) * inverse,
    ]
}

/// Follow a rebirth platform during its wait, preserving the source operand order.
pub fn wait_approach_velocity(current: [f32; 2], target: [f32; 2], remaining: u32) -> [f32; 2] {
    let inverse = 1.0 / remaining as f32;
    [
        inverse * (target[0] - current[0]),
        inverse * (target[1] - current[1]),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remaining_frame_division_lands_exactly_on_the_target() {
        let current = [3.0, 8.0];
        let velocity = approach_velocity(current, [1.0, 4.0], 2);
        assert_eq!(velocity, [-1.0, -2.0]);
        assert_eq!(
            [current[0] + velocity[0], current[1] + velocity[1]],
            [2.0, 6.0]
        );
    }

    #[test]
    fn wait_velocity_retains_its_distinct_multiplication_order() {
        assert_eq!(
            wait_approach_velocity([3.0, 8.0], [1.0, 4.0], 2),
            [-1.0, -2.0]
        );
    }
}
