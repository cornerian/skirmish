//! Safe calls into the currently linked subset of the original Melee menu source.

pub const REVISION: &str = "0bac93a5ee2f985dac6220bd36ed7078ae6ac0c9";
pub const MNMAIN_SHA256: &str = "cbce53b1c0d06c1eb851cc203b17237c0fef827cb024ba6f6a0544761850b7c2";

#[cfg(skirmish_melee_source)]
unsafe extern "C" {
    fn skirmish_mn_light_color_index(menu_kind: u8, selection: u16) -> i32;
    fn skirmish_mn_digit_at(number: i32, digit: i32) -> i32;
}

pub const fn available() -> bool {
    cfg!(skirmish_melee_source)
}

/// Execute the original `mn_8022C010` menu-light selection helper.
pub fn light_color_index(menu_kind: u8, selection: u16) -> Option<i32> {
    // MENU_KIND_34 is a sentinel/unused entry. The original function has no
    // return after its switch, so crossing the FFI boundary with 34+ would be
    // undefined behavior rather than a meaningful source result.
    if menu_kind > 33 {
        return None;
    }
    #[cfg(skirmish_melee_source)]
    {
        // SAFETY: every value through 33 has an explicit return in the pinned
        // source, `selection` widens losslessly to C `int`, and the result is a
        // scalar value.
        Some(unsafe { skirmish_mn_light_color_index(menu_kind, selection) })
    }
    #[cfg(not(skirmish_melee_source))]
    {
        let _ = (menu_kind, selection);
        None
    }
}

/// Execute the original `mn_GetDigitAt` helper for a nonnegative decimal digit.
pub fn digit_at(number: i32, digit: i32) -> Option<i32> {
    if number < 0 || !(0..=9).contains(&digit) {
        return None;
    }
    #[cfg(skirmish_melee_source)]
    {
        // SAFETY: the guard excludes negative exponents and the values use the
        // exact fixed-width integer ABI declared by the C bridge.
        Some(unsafe { skirmish_mn_digit_at(number, digit) })
    }
    #[cfg(not(skirmish_melee_source))]
    {
        let _ = (number, digit);
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn availability_matches_callable_source() {
        assert_eq!(light_color_index(0, 0).is_some(), available());
        assert_eq!(digit_at(123, 1).is_some(), available());
    }

    #[cfg(skirmish_melee_source)]
    #[test]
    fn exact_pinned_source_executes_on_the_host() {
        assert_eq!(light_color_index(0, 4), Some(4));
        assert_eq!(light_color_index(2, 0), Some(1));
        assert_eq!(digit_at(12_345, 2), Some(3));
    }

    #[test]
    fn invalid_digit_requests_do_not_cross_the_ffi_boundary() {
        assert_eq!(digit_at(123, -1), None);
        assert_eq!(digit_at(-123, 1), None);
        assert_eq!(digit_at(123, 10), None);
    }

    #[test]
    fn invalid_menu_kinds_do_not_cross_the_ffi_boundary() {
        assert_eq!(light_color_index(34, 0), None);
        assert_eq!(light_color_index(u8::MAX, 0), None);
    }
}
