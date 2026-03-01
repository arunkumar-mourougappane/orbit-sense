use chrono::Utc;
use eframe::egui::{self, Color32, Stroke};
use walkers::{Map, Position};

use crate::app::OrbitSenseApp;
use crate::constants::ORBITAL_TRAIL_MINUTES;
use crate::location::{Location, calculate_observation};
use crate::satellites::SpaceObject;

struct SatellitesPlugin<'a> {
    satellites: &'a std::collections::HashMap<String, SpaceObject>,
    filtered_satellites: &'a Vec<String>,
    selected_satellite: &'a Option<String>,
    show_orbital_trail: bool,
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

        for name in self.filtered_satellites {
            if let Some(sat) = self.satellites.get(name) {
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

                        if self.show_orbital_trail {
                            // Draw the orbit path
                            let mut prev_pos: Option<Position> = None;

                            for minute_offset in 1..=ORBITAL_TRAIL_MINUTES {
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
                                    let curr_pos = Position::new(
                                        future_obs.azimuth_deg,
                                        future_obs.elevation_deg,
                                    );

                                    if let Some(prev) = prev_pos {
                                        // Make sure we don't draw a line wrapping across the map
                                        if (curr_pos.x() - prev.x()).abs() < 180.0 {
                                            let p1 = projector.project(prev).to_pos2();
                                            let p2 = projector.project(curr_pos).to_pos2();
                                            painter.line_segment(
                                                [p1, p2],
                                                Stroke::new(
                                                    1.5,
                                                    Color32::from_rgba_premultiplied(
                                                        255, 0, 0, 150,
                                                    ),
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
}

pub fn render_map(app: &mut OrbitSenseApp, ui: &mut egui::Ui) {
    let mut map = Map::new(
        Some(&mut app.tiles_manager),
        &mut app.map_memory,
        Position::new(0.0, 0.0),
    );

    map = map.with_plugin(SatellitesPlugin {
        satellites: &app.satellites,
        filtered_satellites: &app.filtered_satellites,
        selected_satellite: &app.selected_satellite,
        show_orbital_trail: app.show_orbital_trail,
    });

    let _response = ui.add(map);
}
