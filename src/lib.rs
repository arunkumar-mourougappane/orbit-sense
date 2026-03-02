#![warn(clippy::all, rust_2018_idioms)]

//! # Orbit Sense
//!
//! A real-time satellite tracking and visualization GUI application written in Rust.
//!
//! ## Features
//! - Live TLE data from [CelesTrak](https://celestrak.org) across five satellite categories
//! - SGP4 orbital propagation for accurate position over time
//! - Interactive 2D world map with pan/zoom powered by the `walkers` crate
//! - Orbital trail and swath footprint rendering per-satellite
//! - Overhead pass prediction relative to a user-defined observer location
//! - Configurable preferences (map theme, swath color/opacity, pass threshold)
//! - Cross-platform settings persistence via `eframe` storage
//!
//! ## Crate Layout
//! | Module | Purpose |
//! |---|---|
//! | [`app`] | Core application state and `eframe::App` implementation |
//! | [`constants`] | Application-wide numeric constants |
//! | [`location`] | Geocoding, geodetic math, and SGP4 observation calculation |
//! | [`satellites`] | CelesTrak TLE fetching and parsing |
//! | [`ui`] | All egui rendering sub-modules |

pub mod app;
pub mod constants;
pub mod location;
pub mod satellites;
pub mod ui;
