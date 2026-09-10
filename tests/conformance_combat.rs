//! Executable missing-behavior specifications against the native match.
//! Source paths below refer to doldecomp/melee 0bac93a5ee2f985dac6220bd36ed7078ae6ac0c9.
//! Synthetic values test qualitative integration behavior, not authentic resources.
//! Run with `cargo test --test conformance_combat -- --ignored`; these gaps fail
//! until implemented. Unsupported inputs/schema are failures, never passing skips.

#[path = "support/grab.rs"]
mod grab_resources;
#[path = "support/conformance.rs"]
mod support;

use skirmish::game::{Action, BUTTON_A, Controller, Match, State, data::MatchData};
use support::{action, idle, step};

const L: u16 = 0x0040;
const Z: u16 = 0x0010;

fn close_data() -> MatchData {
    let mut data = support::data();
    data.stage.spawns = [[-2.0, 0.0], [2.0, 0.0]];
    data.rules.knockback_speed = 0.0;
    data.rules.damage.displacement = Some(skirmish::game::damage::HitlagDisplacementRules {
        axis_thresholds: [0.3; 2],
        minimum_stick_magnitude: 0.5,
        sdi_window: 3,
        sdi_distance: 1.0,
        asdi_distance: 0.5,
    });
    data
}

fn input(player: usize, buttons: u16, stick: [f32; 2]) -> [Controller; 2] {
    let mut input = idle();
    input[player] = Controller {
        cstick: [0.0; 2],
        trigger: 0.0,
        buttons,
        stick,
    };
    input
}

fn until(game: &mut Match, condition: impl Fn(&State) -> bool, message: &str) -> State {
    for _ in 0..240 {
        if condition(game.state()) {
            return game.state().clone();
        }
        step(game, idle());
    }
    assert!(condition(game.state()), "{message}: {:?}", game.state());
    game.state().clone()
}

fn jab(game: &mut Match) -> State {
    let before = game.state().fighters[1].percent;
    step(game, input(0, BUTTON_A, [0.0; 2]));
    until(
        game,
        |s| s.fighters[1].percent > before,
        "jab setup must connect",
    )
}

#[test]
fn repeated_jab_is_weaker_than_the_fresh_hit() {
    // pl/plstale.c::plStale_UpdateStaleMovesFromFighter and ft/ft_0881.c
    // apply attack-instance history to later damage from the same move.
    let mut data = close_data();
    data.rules.staling = Some(skirmish::fighter::stale::Rules {
        penalties: [0.1, 0.09, 0.08, 0.07, 0.06, 0.05, 0.04, 0.03, 0.02],
        debug_bypass: false,
    });
    for fighter in &mut data.fighters {
        fighter.jab.move_id = Some(10);
    }
    let mut game = Match::new(data, 0).unwrap();
    let first = jab(&mut game).fighters[1].percent;
    until(
        &mut game,
        |s| {
            s.fighters
                .iter()
                .all(|f| f.action == Action::Wait && f.hitlag == 0.0)
        },
        "fighters must recover for second jab",
    );
    let before = game.state().fighters[1].percent;
    let repeated = jab(&mut game).fighters[1].percent - before;
    assert!(
        repeated > 0.0 && repeated < first,
        "fresh={first}, repeated={repeated}"
    );
}

#[test]
fn equal_grounded_jabs_clank_instead_of_damaging_both_players() {
    // ft/ftcoll.c::ftColl_8007699C and its hitbox-pair collision caller test
    // equal-damage grounded hitboxes before the hurtbox damage path.
    let mut data = close_data();
    // Explicit invented common coefficients and sampled recovery resources.
    data.rules.clank = Some(skirmish::game::clank::Rules {
        profile: skirmish::game::clank::Profile::OrdinaryGroundedNonSlash,
        response: skirmish::fighter::clank::Rules {
            damage_gap: 9,
            duration_scale: 0.5,
            duration_base: 2.0,
        },
        push_scale: 0.2,
        push_base: 0.6,
        hitlag_maximum: 20.0,
        surface_friction_multiplier: 0.5,
    });
    for fighter in &mut data.fighters {
        fighter.rebound = Some(skirmish::game::clank::Animation {
            animation_length: 13.9,
            poses: vec![fighter.bones.clone(); 15],
        });
        for hit in fighter
            .jab
            .frames
            .iter_mut()
            .flat_map(|frame| &mut frame.hitboxes)
        {
            hit.clank = true;
            hit.rebound = true;
        }
    }
    let mut game = Match::new(data, 0).unwrap();
    step(
        &mut game,
        [Controller {
            cstick: [0.0; 2],
            trigger: 0.0,
            buttons: BUTTON_A,
            stick: [0.0; 2],
        }; 2],
    );
    let collision = step(&mut game, idle());
    assert!(
        collision.fighters.iter().all(|f| f.percent == 0.0),
        "equal clank must suppress both hurtbox hits"
    );
    assert!(
        collision.fighters.iter().any(|f| f.hitlag > 0.0),
        "clank must produce contact response"
    );
    assert!(
        collision
            .fighters
            .iter()
            .all(|f| f.action == Action::ReboundStop)
    );
}

#[test]
fn armor_reduces_launch_without_erasing_damage() {
    // ft/kinds/ftCommon/ftCo_Damage.c::ftCo_Damage_CalcKnockback subtracts
    // max(armor0, armor1) before the minimum clamp; percent damage remains.
    let mut data = close_data();
    data.rules.knockback_speed = 0.15;
    let mut plain = Match::new(data.clone(), 0).unwrap();
    let baseline = jab(&mut plain);
    // Explicit synthetic armor channels and common-data minimum.
    let mut resource = serde_json::to_value(data).unwrap();
    resource["fighters"][1]["armor"] =
        serde_json::json!({"armor0": 30.0, "armor1": 0.0, "minimum_knockback": 0.0});
    let armored: MatchData =
        serde_json::from_value(resource).expect("native armor0/armor1 resources must be supported");
    let armored = jab(&mut Match::new(armored, 0).unwrap());
    let squared = |state: &State| {
        state.fighters[1]
            .knockback
            .iter()
            .map(|v| v * v)
            .sum::<f32>()
    };
    assert_eq!(armored.fighters[1].percent, baseline.fighters[1].percent);
    assert!(
        squared(&armored) < squared(&baseline),
        "armor must reduce applied launch velocity"
    );
}

#[test]
fn holding_shield_takes_contact_stun_without_percent_damage() {
    // ft/kinds/ftCommon/ftCo_Guard.c and ft/ftcoll.c::ftColl_80076CBC
    // route shield contact to guard response rather than ordinary percent damage.
    let mut game = Match::new(close_data(), 0).unwrap();
    for _ in 0..5 {
        step(&mut game, input(1, L, [0.0; 2]));
    }
    let mut attack = input(1, L, [0.0; 2]);
    attack[0].buttons = BUTTON_A;
    step(&mut game, attack);
    let collision = step(&mut game, input(1, L, [0.0; 2]));
    assert_eq!(collision.fighters[1].percent, 0.0);
    assert!(
        collision.fighters[1].hitlag > 0.0,
        "shield contact must cause a response"
    );
}

#[test]
fn grab_then_forward_throw_damages_and_launches_the_captured_victim() {
    // ft/kinds/ftCommon/ftCo_Catch.c, ftCo_CatchWait.c and
    // ftCo_Throw.c::ftCo_800DD1E4 turn directional input into a captured-victim throw.
    let mut data = grab_resources::profile(close_data());
    data.stage.spawns = [[-0.5, 0.0], [0.5, 0.0]];
    data.rules.knockback_speed = 0.15;
    let mut game = Match::new(data, 0).unwrap();
    step(&mut game, input(0, Z, [0.0; 2]));
    for _ in 0..15 {
        step(&mut game, idle());
    }
    assert_eq!(
        game.state().fighters[1].percent,
        0.0,
        "capture alone must not deal throw damage"
    );
    step(&mut game, input(0, 0, [1.0, 0.0]));
    let thrown = until(
        &mut game,
        |s| s.fighters[1].percent > 0.0,
        "forward throw must damage victim",
    );
    assert!(
        thrown.fighters[1].knockback[0] > 0.0,
        "throw must launch victim away from forward-facing attacker"
    );
}

#[test]
fn a_fresh_stick_tilt_moves_the_victim_while_hitlag_remains() {
    // ft/kinds/ftCommon/ftCo_Damage.c::ftCo_Damage_OnEveryHitlag adds a
    // stick-scaled displacement for a fresh tilt and consumes its input window.
    let mut moved = Match::new(close_data(), 0).unwrap();
    assert!(jab(&mut moved).fighters[1].hitlag > 1.0);
    let mut neutral = moved.clone();
    let expected = step(&mut neutral, idle());
    let actual = step(&mut moved, input(1, 0, [1.0, 0.0]));
    assert!(actual.fighters[1].hitlag > 0.0);
    assert!(
        actual.fighters[1].position[0] > expected.fighters[1].position[0],
        "SDI must move during hitlag, not wait for launch physics"
    );
}

#[test]
fn held_stick_adds_exit_displacement_without_a_new_tilt() {
    // ft/kinds/ftCommon/ftCo_Damage.c::ftCo_Damage_OnExitHitlag applies
    // its position displacement before DI, even without a fresh-stick window.
    let mut game = Match::new(close_data(), 0).unwrap();
    assert!(jab(&mut game).fighters[1].hitlag > 1.0);
    while game.state().fighters[1].hitlag > 1.0 {
        step(&mut game, input(1, 0, [1.0, 0.0]));
    }
    let before = game.state().fighters[1].position[0];
    let after = step(&mut game, input(1, 0, [1.0, 0.0]));
    assert_eq!(after.fighters[1].hitlag, 0.0);
    assert!(
        after.fighters[1].position[0] > before,
        "ASDI must shift position on expiry with a held stick"
    );
}

fn downward_hit() -> Match {
    let mut data = close_data();
    data.rules.damage.floor_response = Some(skirmish::game::damage::FloorResponseRules {
        tumble_knockback_threshold: 20.0,
        tech_window: 20.0,
        tech_repeat_lockout: 40,
        tech_roll: None,
        knockdown_options: None,
        passive_frames: 4,
        down_bound_frames: 4,
        down_wait_frames: 4,
        down_stand_frames: 4,
    });
    data.stage.spawns[1][1] = 2.0;
    data.rules.knockback_speed = 0.15;
    for frame in &mut data.fighters[0].jab.frames {
        for hit in &mut frame.hitboxes {
            hit.damage = 40;
            hit.angle_degrees = 270.0;
        }
    }
    let mut game = Match::new(data, 0).unwrap();
    let hit = jab(&mut game);
    assert!(!hit.fighters[1].grounded && hit.fighters[1].hitstun > 0);
    game
}

#[test]
fn a_recent_shield_press_techs_the_tumbling_landing() {
    // ft/kinds/ftCommon/ftCo_Passive.c::ftCo_800987D0 enters Passive
    // from eligible damage-floor contact instead of DownBound.
    let mut game = downward_hit();
    until(
        &mut game,
        |s| s.fighters[1].hitlag <= 1.0,
        "reach final hitlag tick",
    );
    step(&mut game, input(1, L, [0.0; 2]));
    let landing = until(
        &mut game,
        |s| s.fighters[1].grounded,
        "victim must reach floor",
    );
    let name = action(&landing, 1);
    assert!(
        name.starts_with("passive") || name.starts_with("tech"),
        "recent shield press must produce a tech, got {name}"
    );
}

#[test]
fn no_tech_input_enters_down_bound_or_down_wait_on_impact() {
    // ft/kinds/ftCommon/ftCo_DownBound.c::ftCo_800978D4 and
    // ftCo_Down.c keep a tumbling landing in knockdown before a get-up transition.
    let mut game = downward_hit();
    let landing = until(
        &mut game,
        |s| s.fighters[1].grounded,
        "victim must reach floor",
    );
    let name = action(&landing, 1);
    assert!(
        name.starts_with("down") || name == "knockdown",
        "unteched tumble must enter knockdown, got {name}"
    );
}
