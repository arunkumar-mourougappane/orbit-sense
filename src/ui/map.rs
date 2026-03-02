//! Core map rendering logic: draws satellite constellation dots, the selected satellite
//! with orbital trail and footprint, an observer pin, and hover tooltips.

use crate::app::OrbitSenseApp;
use crate::constants::ORBITAL_TRAIL_MINUTES;
use crate::location::{Location, calculate_observation};
use crate::satellites::SpaceObject;
use chrono::Utc;
use egui::{Color32, Pos2, Stroke};
use walkers::{Map, Position};

// ── Altitude colour helpers ──────────────────────────────────────────────────

/// Returns a colour representing the satellite's orbital regime based on altitude.
fn altitude_color(altitude_km: f64) -> Color32 {
    if altitude_km < 2_000.0 {
        Color32::from_rgb(80, 160, 255) // LEO — blue
    } else if altitude_km < 35_000.0 {
        Color32::from_rgb(255, 165, 0) // MEO — orange
    } else {
        Color32::from_rgb(255, 80, 80) // GEO — red
    }
}

// ── Walkers plugin ───────────────────────────────────────────────────────────

/// A `walkers::Plugin` that draws:
/// - All loaded satellites as small colour-coded dots (by altitude regime)
/// - The selected satellite as a larger highlighted dot with orbital trail and swath
/// - The observer's location as a pin marker
/// - A hover tooltip showing the name and altitude of the satellite under the cursor
struct SatellitesPlugin<'a> {
    satellites: &'a std::collections::HashMap<String, SpaceObject>,
    selected_satellite: &'a Option<String>,
    observer: &'a Option<crate::location::Location>,
    show_orbital_trail: bool,
    swath_color: [f32; 3],
    swath_opacity: f32,
    cursor_pos: Option<Pos2>,
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

        // ── Observer pin ─────────────────────────────────────────────────────
        if let Some(obs) = self.observer {
            let pin_pos = projector
                .project(Position::new(obs.lon_deg, obs.lat_deg))
                .to_pos2();
            painter.circle_filled(pin_pos, 6.0, Color32::from_rgb(50, 220, 100));
            painter.circle_stroke(pin_pos, 7.0, Stroke::new(1.5, Color32::WHITE));
            painter.text(
                pin_pos + egui::vec2(9.0, 0.0),
                egui::Align2::LEFT_CENTER,
                &obs.name,
                egui::FontId::proportional(11.0),
                Color32::from_rgb(50, 220, 100),
            );
        }

        // ── All constellation dots + hover tooltip ───────────────────────────
        let mut tooltip: Option<(Pos2, String)> = None;

        for (name, sat) in self.satellites {
            if let Some(obs) = calculate_observation(
                &sat.elements,
                &sat.constants,
                &Location {
                    name: String::new(),
                    lat_deg: 0.0,
                    lon_deg: 0.0,
                    alt_m: 0.0,
                },
                now,
            ) {
                let screen_pos = projector
                    .project(Position::new(obs.sub_lon_deg, obs.sub_lat_deg))
                    .to_pos2();

                let is_selected = self.selected_satellite.as_deref() == Some(name.as_str());

                if is_selected {
                    // Selected satellite: larger white ring drawn below the detailed pass
                    continue;
                }

                let dot_color = altitude_color(obs.altitude_km);
                painter.circle_filled(screen_pos, 3.0, dot_color.gamma_multiply(0.75));

                // Hover detection
                if let Some(cursor) = self.cursor_pos
                    && (screen_pos - cursor).length() < 8.0
                {
                    tooltip = Some((screen_pos, format!("{}\n{:.0} km", name, obs.altitude_km)));
                }
            }
        }

        // ── Selected satellite — detailed rendering ──────────────────────────
        if let Some(name) = self.selected_satellite
            && let Some(sat) = self.satellites.get(name)
            && let Some(obs) = calculate_observation(
                &sat.elements,
                &sat.constants,
                &Location {
                    name: String::new(),
                    lat_deg: 0.0,
                    lon_deg: 0.0,
                    alt_m: 0.0,
                },
                now,
            )
        {
            let pos = Position::new(obs.sub_lon_deg, obs.sub_lat_deg);
            let screen_pos = projector.project(pos).to_pos2();
            let dot_color = altitude_color(obs.altitude_km);

            // Outer highlight ring
            painter.circle_stroke(screen_pos, 8.0, Stroke::new(2.0, Color32::WHITE));
            painter.circle_filled(screen_pos, 6.0, dot_color);

            // Name label
            painter.text(
                screen_pos + egui::vec2(10.0, 0.0),
                egui::Align2::LEFT_CENTER,
                name,
                egui::FontId::proportional(13.0),
                Color32::WHITE,
            );

            // ── Orbital footprint (swath) ─────────────────────────────
            let r_earth = crate::constants::EARTH_RADIUS_KM;
            let h = obs.altitude_km.max(0.1);

            if h > 50.0 {
                let theta = (r_earth / (r_earth + h)).acos();
                let num_points = 72;
                let mut swath_points = Vec::with_capacity(num_points + 1);

                for i in 0..=num_points {
                    let angle = (i as f64) * 2.0 * std::f64::consts::PI / (num_points as f64);
                    let lat_rad = obs.sub_lat_deg.to_radians();
                    let lon_rad = obs.sub_lon_deg.to_radians();

                    let point_lat = (lat_rad.sin() * theta.cos()
                        + lat_rad.cos() * theta.sin() * angle.cos())
                    .asin();
                    let mut point_lon = lon_rad
                        + (angle.sin() * theta.sin() * lat_rad.cos())
                            .atan2(theta.cos() - lat_rad.sin() * point_lat.sin());

                    point_lon = (point_lon + 3.0 * std::f64::consts::PI)
                        % (2.0 * std::f64::consts::PI)
                        - std::f64::consts::PI;

                    let p = projector
                        .project(Position::new(
                            point_lon.to_degrees(),
                            point_lat.to_degrees(),
                        ))
                        .to_pos2();
                    swath_points.push(p);
                }

                let r = (self.swath_color[0] * 255.0) as u8;
                let g = (self.swath_color[1] * 255.0) as u8;
                let b = (self.swath_color[2] * 255.0) as u8;
                let a = (self.swath_opacity * 255.0) as u8;
                let fill_color = Color32::from_rgba_unmultiplied(r, g, b, a);
                let border_color = Color32::from_rgba_premultiplied(r, g, b, a.saturating_add(60));

                let half_w = ui.clip_rect().width() / 2.0;
                let has_antimeridian_crossing = swath_points
                    .windows(2)
                    .any(|w| (w[1].x - w[0].x).abs() > half_w);

                if !has_antimeridian_crossing {
                    let stroke = Stroke::new(1.0, border_color);
                    painter.add(egui::Shape::convex_polygon(
                        swath_points[..num_points].to_vec(),
                        fill_color,
                        stroke,
                    ));
                } else {
                    let edge_screen = projector
                        .project(Position::new(
                            obs.sub_lon_deg + theta.to_degrees(),
                            obs.sub_lat_deg,
                        ))
                        .to_pos2();
                    let radius = (edge_screen.x - screen_pos.x).abs().max(10.0);
                    painter.circle(
                        screen_pos,
                        radius,
                        fill_color,
                        Stroke::new(1.0, border_color),
                    );
                }
            }

            // ── Orbital trail ─────────────────────────────────────────
            if self.show_orbital_trail {
                let mut prev_pos: Option<Position> = None;

                for minute_offset in 1..=ORBITAL_TRAIL_MINUTES {
                    let future_time = now + chrono::Duration::minutes(minute_offset);
                    if let Some(fut) = calculate_observation(
                        &sat.elements,
                        &sat.constants,
                        &Location {
                            name: String::new(),
                            lat_deg: 0.0,
                            lon_deg: 0.0,
                            alt_m: 0.0,
                        },
                        future_time,
                    ) {
                        let curr_pos = Position::new(fut.sub_lon_deg, fut.sub_lat_deg);

                        if let Some(prev) =
                            prev_pos.filter(|p| (curr_pos.x() - p.x()).abs() < 180.0)
                        {
                            let p1 = projector.project(prev).to_pos2();
                            let p2 = projector.project(curr_pos).to_pos2();
                            painter.line_segment(
                                [p1, p2],
                                Stroke::new(1.5, Color32::from_rgba_premultiplied(255, 0, 0, 150)),
                            );
                        }
                        prev_pos = Some(curr_pos);
                    }
                }
            }
        }

        // ── Hover tooltip (drawn last, on top) ───────────────────────────────
        if let Some((pos, label)) = tooltip {
            let rect = painter.text(
                pos + egui::vec2(10.0, -14.0),
                egui::Align2::LEFT_TOP,
                &label,
                egui::FontId::proportional(11.0),
                Color32::WHITE,
            );
            painter.rect_filled(rect.expand(3.0), 2.0, Color32::from_black_alpha(180));
            // Draw text again on top of the background rect
            painter.text(
                pos + egui::vec2(10.0, -14.0),
                egui::Align2::LEFT_TOP,
                &label,
                egui::FontId::proportional(11.0),
                Color32::WHITE,
            );
        }
    }
}

/// Sets up the zoom boundaries and injects the `SatellitesPlugin` into the `walkers::Map`.
/// This also triggers the continuous repaint animation for smooth tracking.
pub fn render_map(app: &mut OrbitSenseApp, ui: &mut egui::Ui) {
    if app.map_memory.zoom() < 2.5 {
        let _ = app.map_memory.set_zoom(2.5);
    }

    // Camera lock: follow the selected satellite
    if app.camera_locked
        && let Some(name) = &app.selected_satellite.clone()
        && let Some(sat) = app.satellites.get(name)
        && let Some(obs) = calculate_observation(
            &sat.elements,
            &sat.constants,
            &Location {
                name: String::new(),
                lat_deg: 0.0,
                lon_deg: 0.0,
                alt_m: 0.0,
            },
            chrono::Utc::now(),
        )
    {
        app.map_memory
            .center_at(Position::new(obs.sub_lon_deg, obs.sub_lat_deg));
    }

    // Capture cursor position for hover tooltip
    let cursor_pos = ui.input(|i| i.pointer.hover_pos());

    let tiles = match app.map_style {
        crate::app::MapStyle::OpenStreetMap => &mut app.tiles_osm,
        crate::app::MapStyle::CartoDark => &mut app.tiles_carto,
    };

    let mut map = Map::new(Some(tiles), &mut app.map_memory, Position::new(0.0, 0.0));
    map = map.with_plugin(SatellitesPlugin {
        satellites: &app.satellites,
        selected_satellite: &app.selected_satellite,
        observer: &app.observer,
        show_orbital_trail: app.show_orbital_trail,
        swath_color: app.swath_color,
        swath_opacity: app.swath_opacity,
        cursor_pos,
    });

    let _response = ui.add(map);

    // Continuous repaint for real-time satellite tracking
    ui.ctx().request_repaint();
}
