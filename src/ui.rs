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
        .anchor(egui::Align2::RIGHT_BOTTOM, egui::vec2(-10.0, -10.0))
        .show(ui.ctx(), |ui| {
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

fn haversine_distance(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
    let r = 6371.0; // Earth radius in km
    let dlat = (lat2 - lat1).to_radians();
    let dlon = (lon2 - lon1).to_radians();
    let a = (dlat / 2.0).sin().powi(2)
        + lat1.to_radians().cos() * lat2.to_radians().cos() * (dlon / 2.0).sin().powi(2);
    let c = 2.0 * a.sqrt().atan2((1.0 - a).sqrt());
    r * c
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
            // Some basic elements since direct struct fields might be private or complex, 
            // but we know SGP4 has inclination, etc. We'll stick to what we know is public or safe.
            ui.heading("Spacecraft Details");
            ui.label(format!("NORAD ID: {}", sat.elements.norad_id));
            ui.label(format!("Inclination: {:.4}°", sat.elements.inclination));
            
            ui.separator();
            ui.heading("Next Pass Prediction");

            if let Some(obs) = &app.observer {
                let mut next_pass = None;
                let now = chrono::Utc::now();

                // Scan the next 24 hours (1440 minutes)
                for min in 1..=1440 {
                    let future_t = now + chrono::Duration::minutes(min);
                    if let Some(pass_obs) = calculate_observation(
                        &sat.elements,
                        &sat.constants,
                        &Location {
                            name: "Dummy".to_string(),
                            lat_deg: 0.0,
                            lon_deg: 0.0,
                            alt_m: 0.0,
                        },
                        future_t,
                    ) {
                        // calculate_observation returns lat/lon in elevation/azimuth properties
                        let dist = haversine_distance(
                            obs.lat_deg,
                            obs.lon_deg,
                            pass_obs.elevation_deg,
                            pass_obs.azimuth_deg,
                        );

                        // If the satellite's ground footprint is within 2000km, it's roughly "overhead"
                        if dist < 2000.0 {
                            next_pass = Some((future_t, dist));
                            break; // Stop at the very first pass!
                        }
                    }
                }

                if let Some((time, dist)) = next_pass {
                    // Convert to local time
                    let local_time: chrono::DateTime<chrono::Local> = time.into();
                    ui.label(format!("Starts: {}", local_time.format("%Y-%m-%d %H:%M:%S")));
                    ui.label(format!("Distance (Ground): {:.0} km", dist));
                } else {
                    ui.label("No passes over this location in the next 24 hours.");
                }
            } else {
                ui.label("Please search and set an Observer Location in the sidebar first to predict passes.");
            }
        });

    app.show_satellite_info = open;
}
