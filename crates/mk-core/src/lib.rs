//! Platform-independent core of the Magic Keyboard for Windows project.
//!
//! This crate is `no_std` and allocation-free so the exact same remapping logic runs inside the
//! kernel filter driver and in host-side unit tests. Enable the `std` feature for serde/TOML support.

#![cfg_attr(not(any(test, feature = "std")), no_std)]

pub mod config;
pub mod engine;
pub mod merge;
pub mod model;
pub mod proto;
pub mod state;
pub mod sync;
pub mod usage;

pub use config::Config;
pub use engine::Engine;
pub use state::{KeyState, Outputs};
