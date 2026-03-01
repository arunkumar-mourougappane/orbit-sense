//! Configuration Window for tuning the application's calculations and displays.

use egui;

use crate::app::OrbitSenseApp;
use crate::constants::DEFAULT_PASS_THRESHOLD_KM;

/// Renders the `File > Preferences` popup window to configure prediction thresholds.
pub fn render_preferences_window(app: &mut OrbitSenseApp, ctx: &egui::Context) {
    if !app.preferences_open {
        return;
    }

    let mut open = app.preferences_open;
    egui::Window::new("Preferences")
        .open(&mut open)
        .resizable(false)
        .collapsible(false)
        .show(ctx, |ui| {
            ui.heading("Display Options");
            ui.checkbox(&mut app.show_orbital_trail, "Show Orbital Trail");

            ui.separator();
            ui.heading("Prediction Settings");
            ui.horizontal(|ui| {
                ui.label("Overhead Pass Distance Threshold (km):");
                ui.add(
                    egui::DragValue::new(&mut app.pass_threshold_km)
                        .range(0.0..=10000.0)
                        .speed(10.0),
                );
            });
            if ui.button("Reset to Default").clicked() {
                app.pass_threshold_km = DEFAULT_PASS_THRESHOLD_KM;
            }
        });

    app.preferences_open = open;
}
