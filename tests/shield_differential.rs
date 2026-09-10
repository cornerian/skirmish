#![cfg(feature = "c-oracle")]
#![allow(unsafe_code)]
use proptest::prelude::*;
use skirmish::fighter::shield;

unsafe extern "C" {
    fn oracle_shield_environment_damage(damage: f32) -> i32;
    fn oracle_shield_radius(values: *const f32) -> f32;
    fn oracle_shield_drain(values: *const f32, output: *mut f32) -> i32;
    fn oracle_shield_response(damage: i32, values: *const f32, powershield: i32, output: *mut f32);
    fn oracle_shield_displacement(
        values: *const f32,
        timer: *mut u8,
        window: i32,
        exit: i32,
        output: *mut f32,
    );
    fn oracle_shield_mash(
        timer: *mut f32,
        directions: *mut i8,
        stick: *const f32,
        pressed: u16,
        threshold: f32,
        amount: f32,
    ) -> i32;
    fn oracle_powershield_tick(timers: *mut f32, flags: *mut u8);
}

fn bits(value: f32) -> u32 {
    value.to_bits()
}

proptest! {
    #[test]
    fn radius_matches_original(health in 0.0f32..100.0, maximum in 1.0f32..100.0, amount in 0.0f32..1.0,
        low in 0.0f32..3.0, high in 0.0f32..3.0, minimum in 0.01f32..1.0, initial in 0.1f32..20.0) {
        let values=[health,maximum,amount,low,high,minimum,initial];
        prop_assert_eq!(bits(shield::radius(health,maximum,amount,[low,high],minimum,initial)),bits(unsafe{oracle_shield_radius(values.as_ptr())}));
    }

    #[test]
    fn drain_and_trigger_strength_match_original(health in 0.0f32..100.0, trigger in 0.0f32..1.0,
        deadzone in 0.0f32..0.9, previous in 0.0f32..1.0, low in 0.0f32..3.0,
        high in 0.0f32..3.0, rate in 0.0f32..5.0, minimum in 0.0f32..10.0) {
        let amount=shield::strength(trigger,deadzone,previous);
        let actual=shield::drain(health,amount,[low,high],rate);
        let values=[health,trigger,deadzone,previous,low,high,rate,minimum];
        let mut output=[0.0;3];
        let broken=unsafe{oracle_shield_drain(values.as_ptr(),output.as_mut_ptr())};
        prop_assert_eq!((bits(actual.0),actual.1,bits(amount)),(bits(output[0]),broken!=0,bits(output[1])));
    }

    #[test]
    fn stun_animation_rate_and_push_match_original(damage in 0i32..1000, amount in 0.0f32..1.0,
        low in 0.0f32..1.0, high in 0.0f32..1.0, scale in 0.0f32..2.0, base in 0.1f32..10.0,
        end in 0.1f32..80.0, push in 0.0f32..2.0, multiplier in 0.0f32..2.0, maximum in 0.0f32..30.0,
        facing in prop_oneof![Just(-1.0f32),Just(1.0f32)], powershield in any::<bool>()) {
        let values=[amount,low,high,scale,base,end,push,multiplier,maximum,facing];
        let mut output=[0.0;3]; unsafe{oracle_shield_response(damage,values.as_ptr(),i32::from(powershield),output.as_mut_ptr())};
        let stun=shield::stun(damage,amount,[low,high],scale,base);
        let (rate,velocity)=shield::response(stun,end,push,if powershield {1.0} else {multiplier},maximum,facing);
        prop_assert_eq!([stun,rate,velocity].map(bits),output.map(bits));
    }

    #[test]
    fn floor_tangent_displacement_matches_original(x in -50.0f32..50.0,y in -50.0f32..50.0,
        nx in -1.0f32..1.0,ny in -1.0f32..1.0,stick in -1.0f32..1.0,minimum in 0.0f32..1.0,
        distance in 0.0f32..5.0,multiplier in 0.0f32..3.0,timer in 0u8..=254,window in 1u8..=254,exit in any::<bool>()) {
        let values=[x,y,nx,ny,stick,1.0,minimum,distance,multiplier];
        let mut oracle_timer=timer;let mut output=[0.0;2];
        unsafe{oracle_shield_displacement(values.as_ptr(),&mut oracle_timer,i32::from(window),i32::from(exit),output.as_mut_ptr())};
        let mut actual=[x,y];let mut actual_timer=timer;
        if (exit || timer<window) && shield::displacement(&mut actual,[nx,ny,0.0],stick,minimum,distance,multiplier) && !exit {actual_timer=254;}
        prop_assert_eq!(actual.map(bits),output.map(bits));prop_assert_eq!(actual_timer,oracle_timer);
    }

    #[test]
    fn dizzy_mash_retains_direction_buckets_and_button_priority(timer in -10.0f32..300.0,
        directions in prop::array::uniform2(-1i8..=1),stick in prop::array::uniform2(-1.0f32..1.0),
        pressed in any::<u16>(),threshold in 0.0f32..1.0,amount in 0.0f32..20.0) {
        let mut actual=timer;let mut actual_directions=directions;
        let changed=shield::mash(&mut actual,&mut actual_directions,stick,pressed,threshold,amount);
        let mut original=timer;let mut original_directions=directions;
        let original_changed=unsafe{oracle_shield_mash(&mut original,original_directions.as_mut_ptr(),stick.as_ptr(),pressed,threshold,amount)};
        prop_assert_eq!((bits(actual),actual_directions,changed),(bits(original),original_directions,original_changed!=0));
    }

    #[test]
    fn powershield_windows_match_original(
        timers in prop::array::uniform2(-10.0f32..300.0), flags in prop::array::uniform3(any::<bool>())
    ) {
        let [mut reflect_timer,mut powershield_timer]=timers;
        let [mut just_started,mut reflecting,mut powershield]=flags;
        shield::powershield_tick(&mut just_started,&mut reflecting,&mut powershield,&mut reflect_timer,&mut powershield_timer);
        let mut oracle_timers=timers;
        let mut oracle_flags=[u8::from(flags[0]),u8::from(flags[1]),u8::from(flags[2]),u8::from(flags[1])];
        unsafe{oracle_powershield_tick(oracle_timers.as_mut_ptr(),oracle_flags.as_mut_ptr())};
        prop_assert_eq!([reflect_timer,powershield_timer].map(bits),oracle_timers.map(bits));
        prop_assert_eq!([just_started,reflecting,powershield,reflecting].map(u8::from),oracle_flags);
    }
}

#[test]
fn exact_zero_health_signed_zero_and_exact_thresholds_match() {
    for damage in [0.0, -0.0, 0.01, 0.99, 1.0, 1.99, 9.75, 999.0] {
        assert_eq!(shield::environment_damage(damage), unsafe {
            oracle_shield_environment_damage(damage)
        });
    }
    for health in [0.0, -0.0, 1.0] {
        let values = [health, 1.0, 0.2, 0.0, 1.0, 1.0, 1.0, 0.0];
        let mut output = [0.0; 3];
        let broken = unsafe { oracle_shield_drain(values.as_ptr(), output.as_mut_ptr()) };
        let actual = shield::drain(health, 1.0, [1.0, 1.0], 1.0);
        assert_eq!((bits(actual.0), actual.1), (bits(output[0]), broken != 0));
    }
    assert_eq!(shield::drain(1.0, 1.0, [1.0, 1.0], 1.0), (0.0, false));
}
