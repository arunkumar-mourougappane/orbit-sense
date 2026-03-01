//! Core 3D map rendering logic using `macroquad`.
use crate::app::OrbitSenseApp;
use crate::constants::ORBITAL_TRAIL_MINUTES;
use crate::location::{Location, calculate_observation};
use crate::satellites::SpaceObject;
use chrono::Utc;
use egui::{Color32, Stroke};
use macroquad::prelude::*;
use walkers::{Map, Position};

/// A custom `walkers::Plugin` that draws the selected satellite and its orbital trail
/// onto the map canvas in real-time.
struct SatellitesPlugin<'a> {
    satellites: &'a std::collections::HashMap<String, SpaceObject>,
    selected_satellite: &'a Option<String>,
    show_orbital_trail: bool,
    swath_color: [f32; 3],
    swath_opacity: f32,
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

        if let Some(name) = self.selected_satellite {
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
                    let color = Color32::RED;

                    painter.circle_filled(screen_pos.to_pos2(), 4.0, color);
                    painter.circle_stroke(
                        screen_pos.to_pos2(),
                        5.0,
                        Stroke::new(1.0, Color32::BLACK),
                    );

                    painter.text(
                        screen_pos.to_pos2() + egui::vec2(8.0, 0.0),
                        egui::Align2::LEFT_CENTER,
                        name,
                        egui::FontId::proportional(14.0),
                        Color32::WHITE,
                    );

                    // --- Draw Visual Footprint (Swath) ---
                    let r_earth = crate::constants::EARTH_RADIUS_KM;
                    let h = obs.altitude_km.max(0.1);

                    if h > 50.0 {
                        let theta = (r_earth / (r_earth + h)).acos();
                        let num_points = 36;
                        let mut swath_points = Vec::with_capacity(num_points);

                        for i in 0..num_points {
                            let angle =
                                (i as f64) * 2.0 * std::f64::consts::PI / (num_points as f64);
                            let lat_rad = obs.elevation_deg.to_radians();
                            let lon_rad = obs.azimuth_deg.to_radians();

                            let point_lat = (lat_rad.sin() * theta.cos()
                                + lat_rad.cos() * theta.sin() * angle.cos())
                            .asin();
                            let mut point_lon = lon_rad
                                + (angle.sin() * theta.sin() * lat_rad.cos())
                                    .atan2(theta.cos() - lat_rad.sin() * point_lat.sin());

                            point_lon = (point_lon + 3.0 * std::f64::consts::PI)
                                % (2.0 * std::f64::consts::PI)
                                - std::f64::consts::PI;

                            let p = projector.project(Position::new(
                                point_lon.to_degrees(),
                                point_lat.to_degrees(),
                            ));
                            swath_points.push(p.to_pos2());
                        }

                        let mut valid_polygon = true;
                        for i in 1..swath_points.len() {
                            if (swath_points[i].x - swath_points[i - 1].x).abs()
                                > ui.clip_rect().width() / 2.0
                            {
                                valid_polygon = false;
                                break;
                            }
                        }

                        if valid_polygon {
                            let r = (self.swath_color[0] * 255.0) as u8;
                            let g = (self.swath_color[1] * 255.0) as u8;
                            let b = (self.swath_color[2] * 255.0) as u8;
                            let a = (self.swath_opacity * 255.0) as u8;
                            let fill_color = Color32::from_rgba_unmultiplied(r, g, b, a);
                            let stroke = Stroke::new(
                                1.0,
                                Color32::from_rgba_premultiplied(255, 255, 255, 100),
                            );
                            painter.add(egui::Shape::convex_polygon(
                                swath_points,
                                fill_color,
                                stroke,
                            ));
                        }
                    }

                    if self.show_orbital_trail {
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
                                let curr_pos =
                                    Position::new(future_obs.azimuth_deg, future_obs.elevation_deg);

                                if let Some(prev) = prev_pos {
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

/// Sets up the zoom boundaries and injects the `SatellitesPlugin` into the `walkers::Map`.
/// This also triggers the continuous repaint animation for smooth tracking.
pub fn render_walkers_2d(app: &mut OrbitSenseApp, ui: &mut egui::Ui) {
    if app.map_memory.zoom() < 2.5 {
        let _ = app.map_memory.set_zoom(2.5);
    }

    // If Camera Lock is enabled, smoothly follow the satellite's exact coordinate
    if app.camera_locked {
        if let Some(name) = &app.selected_satellite {
            if let Some(sat) = app.satellites.get(name) {
                if let Some(obs) = calculate_observation(
                    &sat.elements,
                    &sat.constants,
                    &crate::location::Location {
                        name: "Dummy".to_string(),
                        lat_deg: 0.0,
                        lon_deg: 0.0,
                        alt_m: 0.0,
                    },
                    chrono::Utc::now(),
                ) {
                    app.map_memory
                        .center_at(Position::new(obs.azimuth_deg, obs.elevation_deg));
                }
            }
        }
    }

    // Select the correct tile manager based on the user's Preferences
    let tiles = match app.map_style {
        crate::app::MapStyle::OpenStreetMap => &mut app.tiles_osm,
        crate::app::MapStyle::CartoDark => &mut app.tiles_carto,
    };

    let mut map = Map::new(Some(tiles), &mut app.map_memory, Position::new(0.0, 0.0));

    map = map.with_plugin(SatellitesPlugin {
        satellites: &app.satellites,
        selected_satellite: &app.selected_satellite,
        show_orbital_trail: app.show_orbital_trail,
        swath_color: app.swath_color,
        swath_opacity: app.swath_opacity,
    });

    let _response = ui.add(map);

    // Request continuous repaint for real-time satellite map updates
    ui.ctx().request_repaint();
}

/// Converts Geodetic coordinates (Latitude, Longitude, Altitude) to 3D Cartesian space.
/// Uses Earth Radius normalized to 1.0.
pub fn lat_lon_alt_to_vec3(lat_deg: f64, lon_deg: f64, alt_km: f64) -> Vec3 {
    let lat_rad = lat_deg.to_radians() as f32;

    // Offset by PI to match typical UV map assignments in `draw_sphere`.
    let lon_rad = (lon_deg.to_radians() as f32) + std::f32::consts::PI;

    let r = 1.0 + (alt_km as f32 / crate::constants::EARTH_RADIUS_KM as f32);

    // Standard spherical -> Cartesian, Y-up
    let x = r * lat_rad.cos() * lon_rad.cos();
    let y = r * lat_rad.sin();
    let z = r * lat_rad.cos() * lon_rad.sin();

    vec3(x, y, z)
}

/// Draws the Earth and the orbital trajectories/satellites using macroquad 3D primitives.
pub fn render_macroquad_3d(app: &OrbitSenseApp, earth_tex: &Option<Texture2D>) {
    // 1. Draw Earth
    if let Some(tex) = earth_tex {
        draw_sphere(vec3(0., 0., 0.), 1.0, Some(tex), WHITE);
    } else {
        draw_sphere(vec3(0., 0., 0.), 1.0, None, DARKBLUE);
    }

    // 2. Draw Satellites
    let now = Utc::now();
    let dummy_loc = crate::location::Location {
        name: "Center".into(),
        lat_deg: 0.0,
        lon_deg: 0.0,
        alt_m: 0.0,
    };

    if let Some(name) = &app.selected_satellite {
        if let Some(sat) = app.satellites.get(name) {
            // Calculate current position
            if let Some(obs) = calculate_observation(&sat.elements, &sat.constants, &dummy_loc, now)
            {
                // `calculate_observation` returns azimuth and elevation relative to the dummy observer at (0,0).
                let pos = lat_lon_alt_to_vec3(obs.elevation_deg, obs.azimuth_deg, obs.altitude_km);

                // Draw Satellite marker
                draw_sphere(pos, 0.03, None, RED);

                // Draw orbital trail
                if app.show_orbital_trail {
                    let mut prev_pos = None;
                    for m in (-45..=45).step_by(2) {
                        let t = now + chrono::Duration::minutes(m);
                        if let Some(o) =
                            calculate_observation(&sat.elements, &sat.constants, &dummy_loc, t)
                        {
                            let p =
                                lat_lon_alt_to_vec3(o.elevation_deg, o.azimuth_deg, o.altitude_km);
                            if let Some(prev) = prev_pos {
                                if p.distance(prev) < 0.5 {
                                    draw_line_3d(prev, p, RED);
                                }
                            }
                            prev_pos = Some(p);
                        }
                    }
                }
            }
        }
    }
}

pub fn render_map(_app: &mut OrbitSenseApp, ui: &mut egui::Ui) {
    ui.centered_and_justified(|ui| {
        ui.label("3D globe rendering is active behind this UI.");
    });
}
