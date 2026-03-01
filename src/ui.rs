use eframe::egui::{self, Color32, Stroke};
use walkers::{Map, Position};

use crate::app::{AppMessage, OrbitSenseApp};
use crate::location::{Location, calculate_observation};
use crate::satellites::{SpaceObject, fetch_active_satellites};
use chrono::Utc;

pub fn render_sidebar(app: &mut OrbitSenseApp, ui: &mut egui::Ui) {
    ui.heading("Orbit Sense");
    ui.separator();

    ui.group(|ui| {
        ui.label("Observer Location");
        ui.text_edit_singleline(&mut app.location_query)
            .on_hover_text("Enter City, State (e.g. 'Houston, TX')");

        if ui.button("Search Location").clicked() && !app.location_in_progress {
            app.location_in_progress = true;
            let query = app.location_query.clone();
            let tx = app.tx.clone();

            tokio::spawn(async move {
                let loc = Location::from_query(&query).await;
                let _ = tx.send(AppMessage::LocationGeocoded(loc)).await;
            });
        }

        if app.location_in_progress {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label("Searching...");
            });
        }

        if let Some(obs) = &app.observer {
            ui.label(format!("Lat: {:.2}, Lon: {:.2}", obs.lat_deg, obs.lon_deg));
        }
    });

    ui.separator();

    ui.group(|ui| {
        ui.label("Satellites");
        if ui.button("Refresh TLEs").clicked() && !app.fetch_in_progress {
            app.fetch_in_progress = true;
            let tx = app.tx.clone();

            tokio::spawn(async move {
                let res = fetch_active_satellites().await.map_err(|e| e.to_string());
                let _ = tx.send(AppMessage::SatellitesLoaded(res)).await;
            });
        }

        if app.fetch_in_progress {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label("Downloading...");
            });
        }

        if let Some(err) = &app.error_msg {
            ui.colored_label(egui::Color32::RED, err);
        }

        ui.text_edit_singleline(&mut app.search_query)
            .on_hover_text("Filter by name (e.g. 'ISS')");

        ui.separator();

        // Very basic list
        egui::ScrollArea::vertical().show(ui, |ui| {
            let mut keys: Vec<_> = app.satellites.keys().collect();
            keys.sort();

            for name in keys {
                if name
                    .to_lowercase()
                    .contains(&app.search_query.to_lowercase())
                {
                    let selected = app.selected_satellite.as_ref() == Some(name);
                    if ui.selectable_label(selected, name).clicked() {
                        app.selected_satellite = Some(name.clone());
                    }
                }
            }
        });
    });
}

struct SatellitesPlugin<'a> {
    satellites: &'a std::collections::HashMap<String, SpaceObject>,
    search_query: &'a str,
    selected_satellite: &'a Option<String>,
}

impl walkers::Plugin for SatellitesPlugin<'_> {
    fn run(
        self: Box<Self>,
        ui: &mut egui::Ui,
        _response: &egui::Response,
        projector: &walkers::Projector,
        _map_memory: &walkers::MapMemory,
    ) {
        let painter = ui.painter();
        let now = Utc::now();

        for (name, sat) in self.satellites {
            if self.search_query.is_empty()
                || name
                    .to_lowercase()
                    .contains(&self.search_query.to_lowercase())
            {
                if let Some(obs) = calculate_observation(
                    &sat.elements,
                    &sat.constants,
                    &Location {
                        name: "Dummy".to_string(),
                        lat_deg: 0.0,
                        lon_deg: 0.0,
                        alt_m: 0.0,
                    },
                    now,
                ) {
                    let pos = Position::new(obs.azimuth_deg, obs.elevation_deg);

                    let screen_pos = projector.project(pos);
                    let color = if self.selected_satellite.as_ref() == Some(name) {
                        Color32::RED
                    } else {
                        Color32::YELLOW
                    };

                    painter.circle_filled(screen_pos.to_pos2(), 4.0, color);
                    painter.circle_stroke(
                        screen_pos.to_pos2(),
                        5.0,
                        Stroke::new(1.0, Color32::BLACK),
                    );

                    if self.selected_satellite.as_ref() == Some(name) {
                        painter.text(
                            screen_pos.to_pos2() + egui::vec2(8.0, 0.0),
                            egui::Align2::LEFT_CENTER,
                            name,
                            egui::FontId::proportional(14.0),
                            Color32::WHITE,
                        );

                        // Draw the orbit path for the next 90 minutes
                        let mut prev_pos: Option<Position> = None;

                        for minute_offset in 1..=90 {
                            let future_time = now + chrono::Duration::minutes(minute_offset);
                            if let Some(future_obs) = calculate_observation(
                                &sat.elements,
                                &sat.constants,
                                &Location {
                                    name: "Dummy".to_string(),
                                    lat_deg: 0.0,
                                    lon_deg: 0.0,
                                    alt_m: 0.0,
                                },
                                future_time,
                            ) {
                                let curr_pos =
                                    Position::new(future_obs.azimuth_deg, future_obs.elevation_deg);

                                if let Some(prev) = prev_pos {
                                    // Make sure we don't draw a line wrapping across the map
                                    if (curr_pos.x() - prev.x()).abs() < 180.0 {
                                        let p1 = projector.project(prev).to_pos2();
                                        let p2 = projector.project(curr_pos).to_pos2();
                                        painter.line_segment(
                                            [p1, p2],
                                            Stroke::new(
                                                1.5,
                                                Color32::from_rgba_premultiplied(255, 0, 0, 150),
                                            ),
                                        );
                                    }
                                }
                                prev_pos = Some(curr_pos);
                            }
                        }
                    }
                }
            }
        }
    }
}

pub fn render_map(app: &mut OrbitSenseApp, ui: &mut egui::Ui) {
    let mut map = Map::new(
        Some(&mut app.tiles_manager),
        &mut app.map_memory,
        Position::new(0.0, 0.0),
    );

    map = map.with_plugin(SatellitesPlugin {
        satellites: &app.satellites,
        search_query: &app.search_query,
        selected_satellite: &app.selected_satellite,
    });

    // We can add floating buttons by rendering the map, then allocating UI over it
    let _response = ui.add(map);

    // Custom translucent frame
    let frame = egui::Frame::window(&ui.style())
        .inner_margin(6.0)
        .fill(egui::Color32::from_black_alpha(160)); // Translucent background

    // Overlay controls on the map
    egui::Window::new("Map Controls")
        .frame(frame)
        .collapsible(false)
        .resizable(false)
        .title_bar(false) // Hides the title bar to save space
        .anchor(egui::Align2::RIGHT_TOP, egui::vec2(-10.0, 10.0))
        .show(ui.ctx(), |ui| {
            ui.horizontal(|ui| {
                if ui.button("➕ Zoom In").clicked() {
                    let _ = app.map_memory.zoom_in();
                }
                if ui.button("➖ Zoom Out").clicked() {
                    let _ = app.map_memory.zoom_out();
                }
            });

            ui.separator();

            if ui
                .add_sized(
                    [ui.available_width(), 0.0],
                    egui::Button::new("📍 Center on Observer"),
                )
                .clicked()
            {
                if let Some(obs) = &app.observer {
                    app.map_memory
                        .center_at(Position::new(obs.lon_deg, obs.lat_deg));
                } else {
                    app.map_memory.center_at(Position::new(0.0, 20.0)); // Default rough Atlantic coords
                }
            }
        });
}
