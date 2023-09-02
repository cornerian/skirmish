use peppi::model::{enums::action_state, item::Item, primitives::Velocity};

use crate::data::attacks::ATTACKS;
use crate::data::states::Statable;

use crate::snapshot::{ActionState, Agent};
use crate::math::Vector;

#[derive(Clone, PartialEq, Debug)]
pub struct Attack {
	pub code: action_state::State,

	pub damage: f32,

	pub angle: f32,

	pub knockback: f32, 
	pub knockback_scaling: f32, // 0 if set knockback

	pub electric: bool,

	pub blockable: bool,
	pub shield_damage: f32,

	pub induces: ActionState // not all attacks induce DAMAGE_FALL
}

pub trait Hittable {
    fn hit_by_agent(&self, other: Agent) -> Option<Attack>;
    fn hit_by_item(&self, item: Item) -> Option<Attack>;

	fn hits(&self, others: Vec<Agent>, items: Vec<Item>) -> Vec<Attack> {
		let mut attacks = Vec::new();

		for other in others {
			if let Some(attack) = self.hit_by_agent(other) {
				attacks.push(attack);
			}
		}

		for item in items {
			if let Some(attack) = self.hit_by_item(item) {
				attacks.push(attack);
			}
		}

		attacks
	}

	fn apply_hits(&self, attacks: Vec<Attack>) -> Self;
}

impl Hittable for Agent {
    fn hit_by_agent(&self, other: Agent) -> Option<Attack> {
        return other.state;
    }

    fn hit_by_item(&self, item: Item) -> Option<Attack> {
        return items.lookup(item).attack;
    }

	fn apply_hits(&self, attacks: Vec<Attack>) -> Agent {
		for attack in attacks {
			self.state.action = attack.induces;
			self.state.hitlag = Some(attack.hitlag);
			self.velocities.knockback = self.calculate_knockback(attack);

			return *self; // implement knockback stacking later
		}

		return *self;
	}
}

impl Hittable for Item {
	fn hit_by_agent(&self, other: Agent) -> Option<Attack> {
        return ATTACKS.get(other.character)?.get(&other.state.action.state).cloned();
    }

    fn hit_by_item(&self, item: Item) -> Option<Attack> {
        return items.lookup(item).attack;
    }

	fn apply_hits(&self, attacks: Vec<Attack>) -> Item {
		// disappear
		return *self;
	}
}

pub trait Knockbackable {
	fn calculate_staled_damage(&self, attack: Attack) -> f32;
	fn calculate_knockback(&self, attack: Attack) -> Velocity;

	fn calculate_hitlag(&self, attack: Attack) -> usize;
	fn calculate_hitstun(&self, attack: Attack) -> usize;
}

impl Knockbackable for Agent {
	fn calculate_staled_damage(&self, attack: Attack) -> f32 {
		// damage staling algorithm
		return attack.damage;
	}

	fn calculate_knockback(&self, attack: Attack) -> Velocity {
		// knockback calculation algorithm (from https://www.reddit.com/r/SSBM/comments/5dhi3v/comment/da4nhdv)
		let weight_factor = 2.0 - (2.0 * self.weight / (1.0 + self.weight));
		let stale_factor = 1.4 * (attack.damage + 2.0) * (self.calculate_staled_damage(attack) + self.damage.floor()) / 20.0;

		let magnitude = attack.knockback_scaling * (weight_factor * stale_factor + 18.0) + attack.knockback;

		return Velocity::construct(magnitude, attack.angle);
	}

	fn calculate_hitlag(&self, attack: Attack) -> usize {
		// hitlag calculation algorithm (https://www.ssbwiki.com/Hitlag)
		let electric = if attack.electric { 1.5 } else { 1.0 };
		let crouch = if self.crouching() { 0.67 } else { 1.0 };

		return ((((attack.damage / 3.0) + 4.0).floor() * electric).floor() * crouch).floor() as usize;
	}

	fn calculate_hitstun(&self, attack: Attack) -> usize {
		return (0.4 * self.calculate_knockback(attack).magnitude()).floor() as usize;
	}
}