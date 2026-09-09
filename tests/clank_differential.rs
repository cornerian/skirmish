#![cfg(feature = "c-oracle")]
#![allow(unsafe_code)]
use proptest::prelude::*;
use skirmish::fighter::clank::{
    self, Fighter, Hit, ReboundRules, Response, Rules, Victim, Victims,
};

#[derive(Clone, Copy, Debug, Default)]
#[repr(C)]
struct BridgeHit {
    enabled: u32,
    group: u32,
    rebound: u32,
    next: u32,
    damage: f32,
    victims: [[u32; 2]; 12],
}
#[derive(Clone, Copy, Debug)]
#[repr(C)]
struct BridgeFighter {
    id: u32,
    grounded: u32,
    x: f32,
    damage: i32,
    duration: f32,
    towards: f32,
    hits: [BridgeHit; 4],
}
impl From<&Fighter> for BridgeFighter {
    fn from(f: &Fighter) -> Self {
        Self {
            id: f.id,
            grounded: u32::from(f.grounded),
            x: f.x,
            damage: f.response.damage,
            duration: f.response.rebound_duration,
            towards: f.response.towards,
            hits: f.hits.map(|h| BridgeHit {
                enabled: u32::from(h.enabled),
                group: h.group,
                rebound: u32::from(h.rebound),
                next: u32::from(h.victims.next()),
                damage: h.damage,
                victims: h.victims.entries().map(|v| [v.id, v.remaining]),
            }),
        }
    }
}
impl BridgeFighter {
    fn words(self) -> Vec<u32> {
        let mut words = vec![
            self.id,
            self.grounded,
            self.x.to_bits(),
            self.damage as u32,
            self.duration.to_bits(),
            self.towards.to_bits(),
        ];
        for h in self.hits {
            words.extend([h.enabled, h.group, h.rebound, h.next, h.damage.to_bits()]);
            words.extend(h.victims.into_iter().flatten());
        }
        words
    }
}
unsafe extern "C" {
    fn oracle_clank_pair(
        fighters: *mut BridgeFighter,
        slots: *const u32,
        candidates: *mut u32,
        gap: i32,
        scale: f32,
        base: f32,
    ) -> i32;
    fn oracle_clank_victim(entries: *mut u32, next: *mut u32, id: u32) -> i32;
    fn oracle_clank_rebound(values: *const f32, out: *mut f32);
}

fn compare(mut fighters: [Fighter; 2], slots: [usize; 2], mut candidates: [bool; 4], rules: Rules) {
    let mut original = fighters.each_ref().map(BridgeFighter::from);
    let mut mask = candidates.map(u32::from);
    let original_result = unsafe {
        oracle_clank_pair(
            original.as_mut_ptr(),
            slots.map(|v| v as u32).as_ptr(),
            mask.as_mut_ptr(),
            rules.damage_gap,
            rules.duration_scale,
            rules.duration_base,
        )
    };
    let result = clank::resolve_pair(&mut fighters, slots, &mut candidates, &rules).unwrap();
    assert_eq!(result, original_result != 0);
    assert_eq!(candidates.map(u32::from), mask);
    assert_eq!(
        fighters.each_ref().map(|f| BridgeFighter::from(f).words()),
        original.map(BridgeFighter::words)
    );
}

fn victim_table() -> impl Strategy<Value = Victims> {
    (prop::array::uniform12((0u32..20, any::<u32>())), 0u8..12).prop_map(|(entries, next)| {
        Victims::from_parts(
            entries.map(|(id, remaining)| Victim { id, remaining }),
            next,
        )
        .unwrap()
    })
}
fn hit() -> impl Strategy<Value = Hit> {
    (
        any::<bool>(),
        0u32..4,
        0.0f32..100.0,
        any::<bool>(),
        victim_table(),
    )
        .prop_map(|(enabled, group, damage, rebound, victims)| Hit {
            enabled,
            group,
            damage,
            rebound,
            victims,
            clank: true,
            hits_grounded: true,
        })
}
fn fighter(id: u32) -> impl Strategy<Value = Fighter> {
    (
        any::<bool>(),
        -100.0f32..100.0,
        prop::array::uniform4(hit()),
        0i32..100,
        -100.0f32..100.0,
        -1.0f32..1.0,
    )
        .prop_map(
            move |(grounded, x, hits, damage, rebound_duration, towards)| Fighter {
                id,
                grounded,
                x,
                hits,
                response: Response {
                    damage,
                    rebound_duration,
                    towards,
                },
            },
        )
}

proptest! {
    #[test]
    fn complete_clash_mutation_matches_original(
        a in fighter(1), b in fighter(2), slots in prop::array::uniform2(0usize..4), mask in any::<[bool;4]>(),
        gap in 0i32..30, scale in -1.0f32..2.0, base in -2.0f32..10.0
    ) {
        compare([a,b],slots,mask,Rules {damage_gap:gap,duration_scale:scale,duration_base:base});
    }

    #[test]
    fn victim_insertion_preserves_holes_ring_and_duplicate_timers(mut table in victim_table(), id in 1u32..30) {
        let mut entries=table.entries().map(|v|[v.id,v.remaining]);let mut next=u32::from(table.next());
        let result=unsafe {oracle_clank_victim(entries.as_mut_ptr().cast(),&mut next,id)};
        prop_assert_eq!(table.record(id).unwrap(),result!=0);
        prop_assert_eq!(table.entries().map(|v|[v.id,v.remaining]),entries);
        prop_assert_eq!(u32::from(table.next()),next);
    }

    #[test]
    fn rebound_entry_and_first_physics_match_original(duration in 0.1f32..100.0,towards in prop_oneof![Just(-1.0f32),Just(1.0f32)],
        end in 0.0f32..80.0,scale in 0.0f32..2.0,base in 0.0f32..3.0,friction in 0.0f32..2.0) {
        compare_rebound(duration,towards,ReboundRules {animation_length:end,push_scale:scale,push_base:base,surface_friction_multiplier:friction});
    }
}

fn compare_rebound(duration: f32, towards: f32, rules: ReboundRules) {
    let values = [
        duration,
        towards,
        rules.animation_length,
        rules.push_scale,
        rules.push_base,
        rules.surface_friction_multiplier,
    ];
    let mut output = [0.0; 7];
    unsafe { oracle_clank_rebound(values.as_ptr(), output.as_mut_ptr()) };
    let result = clank::rebound(duration, towards, &rules).unwrap();
    assert_eq!(
        [
            result.animation_rate,
            result.impulse,
            result.ground_acceleration
        ]
        .map(f32::to_bits),
        output[..3]
            .iter()
            .copied()
            .map(f32::to_bits)
            .collect::<Vec<_>>()
            .as_slice()
    );
    let mut marker = result.impulse;
    let first = clank::apply_rebound_friction(&mut marker);
    assert_eq!(marker.to_bits(), output[3].to_bits());
    assert_eq!(u32::from(first), output[4] as u32);
    assert_eq!(
        u32::from(first) + u32::from(clank::apply_rebound_friction(&mut marker)),
        output[5] as u32
    );
    assert_eq!(output[6], 237.0);
}

#[test]
fn exact_thresholds_fractional_minimum_old_responses_and_zero_impulse_match() {
    let rules = Rules {
        damage_gap: 9,
        duration_scale: 0.5,
        duration_base: 2.0,
    };
    for a in [
        0.0, -0.0, 0.5, 0.99, 1.0, 9.99, 10.0, 18.99, 19.0, 19.99, 20.0,
    ] {
        for b in [0.0, -0.0, 0.5, 1.0, 10.0, 19.0] {
            for old in [0, 1, 10, 50] {
                let fighters = [(1, a), (2, b)].map(|(id, damage)| Fighter {
                    id,
                    grounded: true,
                    x: -0.0,
                    hits: [Hit {
                        enabled: true,
                        group: 7,
                        damage,
                        rebound: true,
                        clank: true,
                        hits_grounded: true,
                        ..Default::default()
                    }; 4],
                    response: Response {
                        damage: old,
                        rebound_duration: -0.0,
                        towards: 0.25,
                    },
                });
                compare(fighters, [0, 0], [true; 4], rules);
            }
        }
    }
    for towards in [-1.0, 1.0] {
        compare_rebound(
            4.0,
            towards,
            ReboundRules {
                animation_length: 8.0,
                push_scale: 0.0,
                push_base: 0.0,
                surface_friction_multiplier: 1.0,
            },
        );
    }
}
