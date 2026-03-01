#![warn(clippy::all, rust_2018_idioms)]

//! # Orbit Sense
//!
//! A real-time satellite tracking and visualization GUI application written in Rust.
//! It uses `egui` and `walkers` for the user interface and map rendering, and `sgp4`
//! to propagate accurate satellite orbital data from Celestrak TLEs.

pub mod app;
pub mod constants;
pub mod location;
pub mod satellites;
pub mod ui;
