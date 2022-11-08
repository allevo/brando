pub mod inhabitant;
pub mod manager;
pub mod plugin;

#[cfg(test)]
pub mod entity_storage;

#[cfg(not(test))]
mod entity_storage;
