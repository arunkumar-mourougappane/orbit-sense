#![warn(clippy::all, rust_2018_idioms)]
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

//! Entrypoint for the application.

use eframe::egui;
use orbit_sense::app;
use std::sync::Arc;

/// Starts the `tokio` asynchronous runtime and initializes the `eframe` window.
#[tokio::main]
async fn main() -> eframe::Result<()> {
    // Log to stdout (if you run with `RUST_LOG=debug`).
    env_logger::init();

    // Load window icon from compiled assets
    let icon_bytes = include_bytes!("../../assets/icon.png");
    let image = image::load_from_memory(icon_bytes)
        .expect("Failed to load embedded icon")
        .into_rgba8();
    let (width, height) = image.dimensions();
    let icon_data = Arc::new(egui::IconData {
        rgba: image.into_raw(),
        width,
        height,
    });

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1024.0, 768.0])
            .with_drag_and_drop(false)
            .with_icon(icon_data),
        ..Default::default()
    };

    eframe::run_native(
        env!("CARGO_PKG_NAME"),
        options,
        Box::new(|cc| {
            // This gives us image support:
            egui_extras::install_image_loaders(&cc.egui_ctx);
            Ok(Box::new(app::OrbitSenseApp::new(cc)))
        }),
    )
}
