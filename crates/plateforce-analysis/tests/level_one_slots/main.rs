//! Guards the rules bound into this crate are held to, one module per group of rules.
//!
//! Cargo takes a `main.rs` under `tests/` as one target named for its directory, so every
//! module below compiles into one binary and a test is still selected by name.

mod common;

mod ws1;
mod ws2;
mod ws3;
mod ws3_conditioning;
mod ws5;
mod ws_power_work;
mod ws_rate_impulse;
