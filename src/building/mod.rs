pub mod builder;
mod buildings;
pub mod plugin;

#[cfg(test)]
pub use buildings::*;

#[cfg(not(test))]
use buildings::*;
