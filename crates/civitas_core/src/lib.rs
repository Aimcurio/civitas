pub mod components;
pub mod events;
pub mod household;
pub mod invariants;
pub mod persistence;
pub mod replay;
pub mod settlement;
pub mod sim;
pub mod systems;
pub mod types;
pub mod world;

pub fn core_version() -> &'static str {
    "0.1.0"
}
