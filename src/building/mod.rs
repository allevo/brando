mod buildings;
pub mod manager;
pub mod plugin;

#[cfg(test)]
pub use buildings::*;

pub use buildings::snapshot::*;

#[cfg(not(test))]
use buildings::*;
