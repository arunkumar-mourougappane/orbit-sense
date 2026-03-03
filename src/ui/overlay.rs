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

    let sat_name = match app.selected_satellites.iter().next() {
        Some(name) => name,
        None => return,
    };

    let sat = match app.satellites.get(sat_name) {
        Some(s) => s,
        None => return,
    };

    let mut open = app.show_satellite_info;

    let window_title = if app.selected_satellites.len() > 1 {
        format!(
            "ℹ {} (+{} others)",
            sat_name,
            app.selected_satellites.len() - 1
        )
    } else {
        format!("ℹ {}", sat_name)
    };

    egui::Window::new(window_title)
        .open(&mut open)
        .resizable(false)
        .collapsible(false)
        .show(ctx, |ui| {
            ui.heading("Spacecraft Details");

            if let Some(obs) = app.current_observations.get(sat_name) {
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
                    ui.add_space(8.0);
                    if ui.button("📅 Export to Calendar (.ics)").clicked() {
                        export_passes_to_ics(sat_name, &app.last_predicted_passes);
                    }
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

/// Generates an iCalendar (.ics) string for the given passes and prompts the user to save it.
fn export_passes_to_ics(sat_name: &str, passes: &[(chrono::DateTime<chrono::Utc>, f64)]) {
    if let Some(path) = rfd::FileDialog::new()
        .set_title("Export Passes to Calendar")
        .add_filter("iCalendar", &["ics"])
        .set_file_name(&format!("{}_passes.ics", sat_name.replace(' ', "_")))
        .save_file()
    {
        let mut ics_content = String::new();
        ics_content.push_str("BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//Orbit Sense//EN\r\n");

        let now = chrono::Utc::now().format("%Y%m%dT%H%M%SZ");
        for (i, (time, dist)) in passes.iter().enumerate() {
            let start = time.format("%Y%m%dT%H%M%SZ");
            let end = (*time + chrono::Duration::minutes(10)).format("%Y%m%dT%H%M%SZ");
            let uid = format!("{}-{}@orbitsense", time.timestamp(), i);

            ics_content.push_str("BEGIN:VEVENT\r\n");
            ics_content.push_str(&format!("UID:{}\r\n", uid));
            ics_content.push_str(&format!("DTSTAMP:{}\r\n", now));
            ics_content.push_str(&format!("DTSTART:{}\r\n", start));
            ics_content.push_str(&format!("DTEND:{}\r\n", end));
            ics_content.push_str(&format!("SUMMARY:{} Pass\r\n", sat_name));
            ics_content.push_str(&format!(
                "DESCRIPTION:Predicted overhead pass for {} (Distance: {:.0} km).\r\n",
                sat_name, dist
            ));
            ics_content.push_str("END:VEVENT\r\n");
        }

        ics_content.push_str("END:VCALENDAR\r\n");
        let _ = std::fs::write(path, ics_content);
    }
}

/// Renders the floating time controls panel for Orbital Playback.
pub fn render_time_controls(app: &mut OrbitSenseApp, ctx: &egui::Context) {
    let frame = egui::Frame::window(&ctx.style())
        .inner_margin(6.0)
        .fill(egui::Color32::from_black_alpha(200));

    egui::Window::new("Time Controls")
        .frame(frame)
        .collapsible(false)
        .resizable(false)
        .title_bar(false)
        .anchor(egui::Align2::CENTER_BOTTOM, egui::vec2(0.0, -40.0))
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                if ui.button("⏪ -60x").clicked() {
                    app.playback_speed_multiplier = -60.0;
                }
                if ui.button("⏪ -10x").clicked() {
                    app.playback_speed_multiplier = -10.0;
                }

                let is_paused = app.playback_speed_multiplier == 0.0;
                let play_pause_icon = if is_paused { "▶ Play" } else { "⏸ Pause" };
                if ui.button(play_pause_icon).clicked() {
                    if is_paused {
                        app.playback_speed_multiplier = 1.0;
                    } else {
                        app.playback_speed_multiplier = 0.0;
                    }
                }

                if ui.button("⏩ 10x").clicked() {
                    app.playback_speed_multiplier = 10.0;
                }
                if ui.button("⏩ 60x").clicked() {
                    app.playback_speed_multiplier = 60.0;
                }

                ui.separator();

                if ui.button("Reset Time").clicked() {
                    app.time_offset_seconds = 0.0;
                    app.playback_speed_multiplier = 1.0;
                }
            });

            ui.horizontal(|ui| {
                ui.label("Speed:");
                ui.add(
                    egui::Slider::new(&mut app.playback_speed_multiplier, -3600.0..=3600.0)
                        .text("x"),
                );
            });
        });
}
