//! Pinned original C function bodies, arbitrary tables and explicit coefficients.
#![cfg(feature = "c-oracle")]
#![allow(unsafe_code)]
use proptest::prelude::*;
use skirmish::fighter::stale::{Entry, InstanceCounter, Queue, Rules};

#[link(name = "skirmish_oracle", kind = "static")]
unsafe extern "C" {
    fn oracle_stale_record(
        next: u8,
        entries: *mut u16,
        move_id: u16,
        instance: u16,
        self_hit: i32,
        reset: i32,
    ) -> u8;
    fn oracle_stale_damage(
        next: u8,
        entries: *const u16,
        move_id: i32,
        instance: u16,
        base: f32,
        penalties: *const f32,
        bypass: i32,
    ) -> f32;
    fn oracle_stale_identity(identity: *mut u16, next: *mut u16, move_id: u16, operation: i32);
}

fn packed(queue: &Queue) -> [u16; 20] {
    core::array::from_fn(|index| {
        let entry = queue.entries()[index / 2];
        if index % 2 == 0 {
            entry.move_id
        } else {
            entry.attack_instance
        }
    })
}
fn exact(a: f32, e: f32) {
    if e.is_nan() {
        assert!(a.is_nan());
    } else {
        assert_eq!(a.to_bits(), e.to_bits(), "{a:?} != {e:?}");
    }
}
fn damage_case(queue: &Queue, move_id: i32, instance: u16, base: f32, rules: &Rules) {
    let entries = packed(queue);
    // SAFETY: twenty readable entry halves and nine readable coefficients;
    // every adapter call initializes all thread-local state it reads.
    let expected = unsafe {
        oracle_stale_damage(
            queue.next(),
            entries.as_ptr(),
            move_id,
            instance,
            base,
            rules.penalties.as_ptr(),
            i32::from(rules.debug_bypass),
        )
    };
    exact(queue.damage(move_id, base, rules), expected);
}
fn record_case(queue: &mut Queue, entry: Entry, self_hit: bool, reset: bool) {
    let mut entries = packed(queue);
    // SAFETY: twenty initialized writable entry halves, scalar routing flags.
    let next = unsafe {
        oracle_stale_record(
            queue.next(),
            entries.as_mut_ptr(),
            entry.move_id,
            entry.attack_instance,
            i32::from(self_hit),
            i32::from(reset),
        )
    };
    if reset {
        queue.reset();
    } else {
        queue.record(entry, self_hit);
    }
    assert_eq!(queue.next(), next);
    assert_eq!(packed(queue), entries);
}

#[test]
fn newest_nine_weights_tenth_slot_dedup_and_empty_slot_termination() {
    let rules = Rules {
        penalties: [0.1, 0.09, 0.08, 0.07, 0.06, 0.05, 0.04, 0.03, 0.02],
        debug_bypass: false,
    };
    let mut queue = Queue::default();
    for id in 2..=11 {
        record_case(
            &mut queue,
            Entry {
                move_id: id,
                attack_instance: id,
            },
            false,
            false,
        );
    }
    assert_eq!(queue.multiplier(2, &rules.penalties), 1.0); // oldest physical slot excluded
    assert!(!queue.record(
        Entry {
            move_id: 2,
            attack_instance: 2
        },
        false
    )); // but still deduplicated
    for id in 0..=12 {
        damage_case(&queue, id, 99, 10.0, &rules);
    }
    assert_eq!(queue.damage(11, 10.0, &rules), 9.0);
    record_case(
        &mut queue,
        Entry {
            move_id: 12,
            attack_instance: 12,
        },
        false,
        false,
    );
    assert!(queue.record(
        Entry {
            move_id: 2,
            attack_instance: 2
        },
        false
    )); // old physical slot overwritten
    record_case(
        &mut queue,
        Entry {
            move_id: 2,
            attack_instance: 2,
        },
        false,
        true,
    );
    assert_eq!(queue, Queue::default());
    let mut entries = [Entry {
        move_id: 7,
        attack_instance: 1,
    }; 10];
    entries[8] = Entry::default();
    let hole = Queue::from_parts(0, entries).unwrap();
    damage_case(&hole, 7, 1, 10.0, &rules);
    assert_eq!(hole.damage(7, 10.0, &rules), 9.0);
}

#[test]
fn instance_wrap_same_move_retention_and_explicit_restart_match() {
    let mut counter = InstanceCounter::from_next(u16::MAX).unwrap();
    let mut identity = Entry::INACTIVE;
    for (move_id, operation) in [(7, 1), (7, 1), (7, 2), (1, 1), (1, 1), (99, 1), (0, 0)] {
        let mut expected = [identity.move_id, identity.attack_instance];
        let mut next = counter.next_value();
        // SAFETY: two identity halves and one mutable counter.
        unsafe { oracle_stale_identity(expected.as_mut_ptr(), &mut next, move_id, operation) };
        match operation {
            0 => identity = Entry::INACTIVE,
            1 => identity.change_move(move_id, &mut counter),
            _ => identity.restart(&mut counter),
        }
        assert_eq!([identity.move_id, identity.attack_instance], expected);
        assert_eq!(counter.next_value(), next);
        assert_ne!(counter.next_value(), 0);
    }
}

#[test]
fn exempt_self_hit_fresh_noop_and_debug_bypass_match_with_special_floats() {
    let mut queue = Queue::default();
    record_case(
        &mut queue,
        Entry {
            move_id: 7,
            attack_instance: 9,
        },
        true,
        false,
    );
    record_case(
        &mut queue,
        Entry {
            move_id: 1,
            attack_instance: 9,
        },
        false,
        false,
    );
    assert_eq!(queue, Queue::default());
    for base in [-0.0, 0.0, 1.0, f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        for bypass in [false, true] {
            let rules = Rules {
                penalties: [0.1; 9],
                debug_bypass: bypass,
            };
            damage_case(&queue, 7, 9, base, &rules);
            record_case(
                &mut queue,
                Entry {
                    move_id: 7,
                    attack_instance: 9,
                },
                false,
                false,
            );
            damage_case(&queue, 7, 9, base, &rules);
            damage_case(&queue, 1, 9, base, &rules);
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]
    #[test]
    fn generated_tables_and_sequences_match(
        initial in prop::array::uniform20(any::<u16>()), next in 0_u8..10,
        steps in prop::collection::vec((0_u16..25,0_u16..25,any::<bool>(),0_u8..20),1..100),
        coefficients in any::<[u32;9]>(),base in any::<u32>(),bypass in any::<bool>(),
    ) {
        let entries=core::array::from_fn(|i|Entry{move_id:initial[2*i],attack_instance:initial[2*i+1]});
        let mut queue=Queue::from_parts(next,entries).unwrap();
        let rules=Rules{penalties:coefficients.map(f32::from_bits),debug_bypass:bypass};
        for (move_id,attack_instance,self_hit,reset) in steps {
            let attack=Entry{move_id,attack_instance};
            damage_case(&queue,i32::from(move_id),attack_instance,f32::from_bits(base),&rules);
            record_case(&mut queue,attack,self_hit,reset==0);
        }
    }
}
