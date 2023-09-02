pub mod data {
    pub mod attacks;
    pub mod stages;
    pub mod states;
}

pub mod hits;
use hits::Hittable;

pub mod snapshot;
use snapshot::{Inputs, Snapshot, Agent};

pub mod math;

use peppi::model::{
    buttons,
    frame::{
        self,
        Frame, 
        PortData,
    },
    game::{
        Start,
        End
    },
    item::Item,
};

pub struct Simulator<const N: usize> {
    pub snapshot: Snapshot<N>,

    // data: SimulatorData<N>
}

impl<const N: usize> From<Snapshot<N>> for Simulator<N> {
    fn from(snapshot: Snapshot<N>) -> Self {
        Simulator::<N> {
            snapshot,
        }
    }
}

impl<const N: usize> Simulator<N> {

}