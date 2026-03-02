//! Hovering map controls and the Spacecraft Details pop-out window.

use egui;

use crate::app::OrbitSenseApp;

/// Renders the translucent floating navigation toolbox in the corner of the map.
/// Includes zoom interactions, window fitting, and the Details toggle.
pub fn render_map_controls(app: &mut OrbitSenseApp, ctx: &egui::Context) {
    let frame = egui::Frame::window(&ctx.style())
        .inner_margin(6.0)
        .fill(egui::Color32::from_black_alpha(160)); // Translucent background

    egui::Window::new("Map Controls")
        .frame(frame)
        .collapsible(false)
        .resizable(false)
        .title_bar(false) // Hides the title bar to save space
        .anchor(egui::Align2::RIGHT_BOTTOM, egui::vec2(-10.0, -40.0))
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                if ui
                    .add_sized([30.0, 30.0], egui::Button::new("➕"))
                    .on_hover_text("Zoom In")
                    .clicked()
                {
                    let _ = app.map_memory.zoom_in();
                }
                if ui
                    .add_sized([30.0, 30.0], egui::Button::new("➖"))
                    .on_hover_text("Zoom Out")
                    .clicked()
                {
                    let _ = app.map_memory.zoom_out();
                }
                if ui
                    .add_sized([30.0, 30.0], egui::Button::new("🌐"))
                    .on_hover_text("Max Zoomout")
                    .clicked()
                {
                    let _ = app.map_memory.set_zoom(2.5);
                }
                if ui
                    .add_sized([30.0, 30.0], egui::Button::new("🗺"))
                    .on_hover_text("Fit to Window")
                    .clicked()
                {
                    app.map_memory.center_at(walkers::Position::new(0.0, 0.0));
                    let _ = app.map_memory.set_zoom(2.5);
                }
                if ui
                    .add_sized([30.0, 30.0], egui::Button::new("📍"))
                    .on_hover_text("Center on Observer")
                    .clicked()
                {
                    if let Some(obs) = &app.observer {
                        app.map_memory
                            .center_at(walkers::Position::new(obs.lon_deg, obs.lat_deg));
                    } else {
                        app.map_memory.center_at(walkers::Position::new(0.0, 20.0));
                    }
                }

                let lock_icon = if app.camera_locked { "🔒" } else { "🔓" };
                let lock_hover = if app.camera_locked {
                    "Unlock Camera"
                } else {
                    "Auto-Track Satellite"
                };
                if ui
                    .add_sized(
                        [30.0, 30.0],
                        egui::Button::new(lock_icon).selected(app.camera_locked),
                    )
                    .on_hover_text(lock_hover)
                    .clicked()
                {
                    app.camera_locked = !app.camera_locked;
                }

                if ui
                    .add_sized(
                        [30.0, 30.0],
                        egui::Button::new("ℹ").selected(app.show_satellite_info),
                    )
                    .on_hover_text("Satellite Info")
                    .clicked()
                {
                    app.show_satellite_info = !app.show_satellite_info;
                }
            });
        });
}

/// Renders the `egui::Window` displaying orbital mechanics math, real-time positional
/// tracking, and the output of the next predicted overhead pass. Continually requests UI repetition.
pub fn render_satellite_info(app: &mut OrbitSenseApp, ctx: &egui::Context) {
    if !app.show_satellite_info {
        return;
    }

    let sat_name = match &app.selected_satellite {
        Some(name) => name,
        None => return,
    };

    let sat = match app.satellites.get(sat_name) {
        Some(s) => s,
        None => return,
    };

    let mut open = app.show_satellite_info;

    egui::Window::new(format!("ℹ {}", sat_name))
        .open(&mut open)
        .resizable(false)
        .collapsible(false)
        .show(ctx, |ui| {
            ui.heading("Spacecraft Details");

            if let Some(obs) = &app.current_observation {
                ui.label(format!("Altitude:      {:.1} km", obs.altitude_km));
                ui.label(format!("Velocity:      {:.2} km/s", obs.velocity_km_s));
                ui.label(format!("Latitude (sub-sat):  {:.4}°", obs.sub_lat_deg));
                ui.label(format!("Longitude (sub-sat): {:.4}°", obs.sub_lon_deg));
                ui.separator();
            }

            ui.label(format!("NORAD ID: {}", sat.elements.norad_id));
            if let Some(id) = &sat.elements.international_designator {
                ui.label(format!("Int. Designator: {}", id));
            }
            ui.label(format!("Epoch Date: {}", sat.elements.datetime.format("%Y-%m-%d %H:%M:%S UTC")));
            ui.label(format!("Inclination: {:.4}°", sat.elements.inclination));
            ui.label(format!("Eccentricity: {:.6}", sat.elements.eccentricity));
            ui.label(format!("RA of Asc Node: {:.4}°", sat.elements.right_ascension));
            ui.label(format!("Arg of Perigee: {:.4}°", sat.elements.argument_of_perigee));
            ui.label(format!("Mean Anomaly: {:.4}°", sat.elements.mean_anomaly));
            ui.label(format!("Mean Motion: {:.4} rev/day", sat.elements.mean_motion));
            ui.label(format!("Revolution # (Epoch): {}", sat.elements.revolution_number));
            ui.label(format!("BSTAR Drag Term: {:.6}", sat.elements.drag_term));

            ui.separator();
            ui.heading("Upcoming Passes (48h)");

            if app.observer.is_some() {
                if !app.last_predicted_passes.is_empty() {
                    egui::ScrollArea::vertical().max_height(150.0).show(ui, |ui| {
                        for (time, dist) in &app.last_predicted_passes {
                            let local_time: chrono::DateTime<chrono::Local> = (*time).into();
                            ui.label(format!("Starts: {}", local_time.format("%Y-%m-%d %H:%M:%S")));

                            // Visual color hint for direct vs low horizon passes
                            let color = if *dist < app.pass_threshold_km / 2.0 {
                                egui::Color32::from_rgb(100, 255, 100) // Direct/Green
                            } else {
                                egui::Color32::from_rgb(200, 200, 200) // Lower/Gray
                            };
                            ui.colored_label(color, format!("Distance (Ground): {:.0} km", dist));
                            ui.separator();
                        }
                    });
                } else if app.is_predicting_pass {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label("Calculating...");
                    });
                } else {
                    ui.label("No passes over this location in the next 48 hours.");
                }
            } else {
                ui.label("Please search and set an Observer Location in the sidebar first to predict passes.");
            }
        });

    app.show_satellite_info = open;
    if open {
        ctx.request_repaint_after(std::time::Duration::from_millis(33));
    }
}
