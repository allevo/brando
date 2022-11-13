#[cfg(not(test))]
mod manager;

#[cfg(test)]
pub mod manager;

mod plugin;
pub use plugin::{
    MoreInhabitantsNeeded, MoreWorkersNeeded, PalatabilityManagerResource, PalatabilityPlugin,
};
