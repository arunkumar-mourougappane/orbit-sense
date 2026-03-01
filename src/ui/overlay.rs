use eframe::egui;
use walkers::Position;

use crate::app::OrbitSenseApp;

pub fn render_map_controls(app: &mut OrbitSenseApp, ctx: &egui::Context) {
    let frame = egui::Frame::window(&ctx.style())
        .inner_margin(6.0)
        .fill(egui::Color32::from_black_alpha(160)); // Translucent background

    egui::Window::new("Map Controls")
        .frame(frame)
        .collapsible(false)
        .resizable(false)
        .title_bar(false) // Hides the title bar to save space
        .anchor(egui::Align2::RIGHT_BOTTOM, egui::vec2(-10.0, -10.0))
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
                    app.map_memory.center_at(Position::new(0.0, 0.0));
                    let _ = app.map_memory.set_zoom(2.5);
                }
                if ui
                    .add_sized([30.0, 30.0], egui::Button::new("📍"))
                    .on_hover_text("Center on Observer")
                    .clicked()
                {
                    if let Some(obs) = &app.observer {
                        app.map_memory
                            .center_at(Position::new(obs.lon_deg, obs.lat_deg));
                    } else {
                        app.map_memory.center_at(Position::new(0.0, 20.0)); // Default rough Atlantic coords
                    }
                }
                if ui
                    .add_sized([30.0, 30.0], egui::Button::new("ℹ"))
                    .on_hover_text("Satellite Info")
                    .clicked()
                {
                    app.show_satellite_info = !app.show_satellite_info;
                }
            });
        });
}

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
            ui.label(format!("NORAD ID: {}", sat.elements.norad_id));
            ui.label(format!("Inclination: {:.4}°", sat.elements.inclination));
            
            ui.separator();
            ui.heading("Next Pass Prediction");

            if app.observer.is_some() {
                if let Some((time, dist)) = app.last_predicted_pass {
                    let local_time: chrono::DateTime<chrono::Local> = time.into();
                    ui.label(format!("Starts: {}", local_time.format("%Y-%m-%d %H:%M:%S")));
                    ui.label(format!("Distance (Ground): {:.0} km", dist));
                } else if app.is_predicting_pass {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label("Calculating...");
                    });
                } else {
                    ui.label("No passes over this location in the next 24 hours.");
                }
            } else {
                ui.label("Please search and set an Observer Location in the sidebar first to predict passes.");
            }
        });

    app.show_satellite_info = open;
}
