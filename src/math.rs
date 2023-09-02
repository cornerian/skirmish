use peppi::model::primitives::{Direction, Port, Position, Velocity};

pub trait Vector {
	fn magnitude(&self) -> f32;

	fn angle(&self) -> f32; 
	
	fn construct(magnitude: f32, angle: f32) -> Self;

	fn rotate(&self, delta: f32) -> Self;
}

impl Vector for Velocity {
	fn magnitude(&self) -> f32 {
		(self.x.powi(2) + self.y.powi(2)).sqrt()
	}

	fn angle(&self) -> f32 {
		self.y.atan2(self.x)
	}
	
	fn construct(magnitude: f32, angle: f32) -> Self {
		Self {
			x: angle.cos() * magnitude,
			y: angle.sin() * magnitude
		}
	}

	fn rotate(&self, delta: f32) -> Self {
		Self::construct(self.magnitude(), self.angle() + delta)
	}
}

pub struct Capsule {
	pub radius: f32,
	pub position: (Position, Position), // start and end of capsule. equivalent if sphere
	pub priority: i32,
}

pub struct Sphere {
	pub radius: f32,
	pub position: Position,
	pub priority: i32,
}

pub trait Collidable {
	fn collides(&self, other: &Self) -> bool;
}

impl Collidable for Sphere {
	fn collides(&self, other: &Self) -> bool {
		let distance = (self.position.x - other.position.x).powi(2) + (self.position.y - other.position.y).powi(2);
		let radius = self.radius + other.radius;

		distance <= radius.powi(2)
	}
}

impl Collidable for Capsule {
	fn collides(&self, other: &Self) -> bool {
		let distance = (self.position.0.x - other.position.0.x).powi(2) + (self.position.0.y - other.position.0.y).powi(2);
		let radius = self.radius + other.radius;

		distance <= radius.powi(2)
	}
}