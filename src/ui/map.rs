//! Core map rendering logic: draws satellite constellation dots, the selected satellite
//! with orbital trail and footprint, an observer pin, and hover tooltips.

use crate::app::OrbitSenseApp;
use crate::location::{Location, calculate_observation};
use egui::{Color32, Stroke};
use walkers::{Map, Position};

// ── Altitude colour helpers ──────────────────────────────────────────────────

/// Returns a vibrant colour representing the satellite's orbital regime based on altitude.
fn altitude_color(altitude_km: f64) -> Color32 {
    if altitude_km < 2_000.0 {
        Color32::from_rgb(100, 200, 255) // LEO — bright blue
    } else if altitude_km < 35_000.0 {
        Color32::from_rgb(255, 220, 0) // MEO — bright yellow
    } else {
        Color32::from_rgb(255, 80, 120) // GEO — hot pink
    }
}

/// Returns the size in pixels for a satellite dot based on altitude.
fn satellite_size(altitude_km: f64) -> f32 {
    if altitude_km < 2_000.0 {
        3.0 // LEO
    } else if altitude_km < 35_000.0 {
        4.0 // MEO
    } else {
        5.0 // GEO
    }
}

/// Draws a lat/lon graticule on the map.
fn draw_graticule(painter: &egui::Painter, projector: &walkers::Projector, clip_rect: egui::Rect) {
    let grid_step = 15.0;
    let grid_color = Color32::from_rgba_unmultiplied(100, 100, 100, 50);

    let mut lat = -90.0;
    while lat <= 90.0 {
        let mut prev_pos: Option<egui::Pos2> = None;
        let mut lon = -180.0;
        while lon < 180.0 {
            let pos = projector.project(Position::new(lon, lat)).to_pos2();
            if clip_rect.contains(pos) {
                if let Some(prev) = prev_pos
                    && (pos.x - prev.x).abs() < 200.0
                {
                    painter.line_segment([prev, pos], Stroke::new(0.5, grid_color));
                }
                prev_pos = Some(pos);
            }
            lon += 2.0;
        }
        lat += grid_step;
    }

    let mut lon = -180;
    while lon <= 180 {
        let mut prev_pos: Option<egui::Pos2> = None;
        let mut lat = -90.0;
        while lat <= 90.0 {
            let pos = projector.project(Position::new(lon as f64, lat)).to_pos2();
            if clip_rect.contains(pos) {
                if let Some(prev) = prev_pos {
                    painter.line_segment([prev, pos], Stroke::new(0.5, grid_color));
                }
                prev_pos = Some(pos);
            }
            lat += 2.0;
        }
        lon += grid_step as i32;
    }
}

/// Draws the day/night terminator line on the map.
fn draw_terminator(
    painter: &egui::Painter,
    projector: &walkers::Projector,
    clip_rect: egui::Rect,
    current_time: chrono::DateTime<chrono::Utc>,
) {
    let jd = current_time.timestamp() as f64 / 86400.0 + 2440587.5;
    let t = (jd - 2451545.0) / 36525.0;
    let gmst = (280.46061837 + 360.98564736629 * (jd - 2451545.0) + t * t * 0.000387933
        - t * t * t / 38710000.0)
        % 360.0;
    let subsolar_lon = (gmst - 180.0) % 360.0;

    let mut terminator_points = Vec::new();
    let mut lat = -90.0;
    while lat <= 90.0 {
        let lon = subsolar_lon + 90.0;
        let lon = if lon > 180.0 { lon - 360.0 } else { lon };
        let pos = projector.project(Position::new(lon, lat)).to_pos2();
        if clip_rect.contains(pos) {
            terminator_points.push(pos);
        }
        lat += 2.0;
    }

    if terminator_points.len() > 1 {
        for window in terminator_points.windows(2) {
            painter.line_segment(
                [window[0], window[1]],
                Stroke::new(1.5, Color32::from_rgba_unmultiplied(255, 140, 0, 120)),
            );
        }
    }
}

// ── Walkers plugin ───────────────────────────────────────────────────────────

/// A `walkers::Plugin` that draws:
/// - All loaded satellites as small colour-coded dots (by altitude regime)
/// - The selected satellite as a larger highlighted dot with orbital trail and swath
/// - The observer's location as a pin marker
/// - A hover tooltip showing the name and altitude of the satellite under the cursor
struct SatellitesPlugin<'a> {
    satellites: &'a std::collections::HashMap<String, crate::satellites::SpaceObject>,
    selected_satellites: &'a std::collections::HashSet<String>,
    observer: &'a Option<crate::location::Location>,
    show_orbital_trail: bool,
    swath_color: [f32; 3],
    swath_opacity: f32,
    cached_trails: &'a std::collections::HashMap<
        String,
        (chrono::DateTime<chrono::Utc>, Vec<walkers::Position>),
    >,
    cached_swaths: &'a std::collections::HashMap<String, (f64, f64, Vec<walkers::Position>)>,
    current_observations: &'a std::collections::HashMap<String, crate::location::Observation>,
}

impl walkers::Plugin for SatellitesPlugin<'_> {
    fn run(
        self: Box<Self>,
        ui: &mut egui::Ui,
        response: &egui::Response,
        projector: &walkers::Projector,
        _map_memory: &walkers::MapMemory,
    ) {
        let painter = ui.painter();
        let clip_rect = ui.clip_rect();

        let hover_pos = response.hover_pos();

        // Draw graticule
        draw_graticule(painter, projector, clip_rect);

        // Draw terminator line
        draw_terminator(painter, projector, clip_rect, chrono::Utc::now());

        // ── Global satellites ──────────────────────────────────────────────
        for (name, sat) in self.satellites {
            if self.selected_satellites.contains(name) {
                continue;
            }

            if let Some((lat_deg, lon_deg)) = sat.cached_position {
                let pos = Position::new(lon_deg, lat_deg);
                let screen_pos = projector.project(pos).to_pos2();

                // Frustum culling
                if clip_rect.contains(screen_pos) {
                    let dot_color = altitude_color(sat.cached_altitude);
                    let dot_size = satellite_size(sat.cached_altitude);

                    // Glow halo effect
                    painter.circle_filled(
                        screen_pos,
                        dot_size + 3.0,
                        Color32::from_rgba_unmultiplied(
                            dot_color.r(),
                            dot_color.g(),
                            dot_color.b(),
                            40,
                        ),
                    );

                    painter.circle_filled(screen_pos, dot_size, dot_color);

                    if let Some(hp) = hover_pos
                        && (screen_pos - hp).length() < (dot_size + 6.0)
                    {
                        painter.text(
                            screen_pos + egui::vec2(dot_size + 6.0, 0.0),
                            egui::Align2::LEFT_CENTER,
                            name,
                            egui::FontId::proportional(12.0),
                            Color32::WHITE,
                        );
                    }
                }
            }
        }

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

        // ── Selected satellites — detailed rendering ──────────────────────────
        for name in self.selected_satellites {
            if let Some(obs) = self.current_observations.get(name) {
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
                let mut drawn_swath_points = Vec::new();
                if let Some((_, _, points)) = self.cached_swaths.get(name) {
                    for p in points {
                        drawn_swath_points
                            .push(projector.project(Position::new(p.x(), p.y())).to_pos2());
                    }
                }

                if !drawn_swath_points.is_empty() {
                    let r = (self.swath_color[0] * 255.0) as u8;
                    let g = (self.swath_color[1] * 255.0) as u8;
                    let b = (self.swath_color[2] * 255.0) as u8;
                    let a = (self.swath_opacity * 255.0) as u8;
                    let fill_color = Color32::from_rgba_unmultiplied(r, g, b, a);
                    let border_color =
                        Color32::from_rgba_premultiplied(r, g, b, a.saturating_add(60));

                    let half_w = ui.clip_rect().width() / 2.0;
                    let has_antimeridian_crossing = drawn_swath_points
                        .windows(2)
                        .any(|w| (w[1].x - w[0].x).abs() > half_w);

                    if !has_antimeridian_crossing {
                        let stroke = Stroke::new(1.0, border_color);
                        painter.add(egui::Shape::convex_polygon(
                            drawn_swath_points,
                            fill_color,
                            stroke,
                        ));
                    } else {
                        let r_earth = crate::constants::EARTH_RADIUS_KM;
                        let h = obs.altitude_km.max(0.1);
                        let theta = (r_earth / (r_earth + h)).acos();
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
                if self.show_orbital_trail
                    && let Some((_, trail)) = self.cached_trails.get(name)
                {
                    let mut prev_pos: Option<Position> = None;

                    for &curr_pos in trail {
                        if let Some(prev) =
                            prev_pos.filter(|p| (curr_pos.x() - p.x()).abs() < 180.0)
                        {
                            let p1 = projector
                                .project(Position::new(prev.x(), prev.y()))
                                .to_pos2();
                            let p2 = projector
                                .project(Position::new(curr_pos.x(), curr_pos.y()))
                                .to_pos2();
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
        && let Some(first_name) = app.selected_satellites.iter().next()
        && let Some(obs) = app.current_observations.get(first_name)
    {
        app.map_memory
            .center_at(Position::new(obs.sub_lon_deg, obs.sub_lat_deg));
    }

    if app.show_orbital_trail {
        let now = app.current_time();
        for name in &app.selected_satellites {
            if let Some(sat) = app.satellites.get(name) {
                let needs_update = match app.cached_trails.get(name) {
                    None => true,
                    Some((time, _)) => (now - *time).num_seconds() > 60,
                };

                if needs_update {
                    let mut trail =
                        Vec::with_capacity(crate::constants::ORBITAL_TRAIL_MINUTES as usize);
                    for minute_offset in 1..=crate::constants::ORBITAL_TRAIL_MINUTES {
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
                            trail.push(Position::new(fut.sub_lon_deg, fut.sub_lat_deg));
                        }
                    }
                    app.cached_trails.insert(name.clone(), (now, trail));
                }
            }
        }
    }
    // Memoize swath footprint
    for name in &app.selected_satellites {
        if let Some(obs) = app.current_observations.get(name) {
            let mut needs_swath_update = true;
            if let Some((c_lat, c_lon, _)) = app.cached_swaths.get(name)
                && (obs.sub_lat_deg - *c_lat).abs() < 0.05
                && (obs.sub_lon_deg - *c_lon).abs() < 0.05
            {
                needs_swath_update = false;
            }

            if needs_swath_update {
                let r_earth = crate::constants::EARTH_RADIUS_KM;
                let h = obs.altitude_km.max(0.1);
                let mut swath_points = Vec::new();
                if h > 50.0 {
                    let theta = (r_earth / (r_earth + h)).acos();
                    let num_points = 72;
                    swath_points.reserve(num_points + 1);

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

                        swath_points.push(Position::new(
                            point_lon.to_degrees(),
                            point_lat.to_degrees(),
                        ));
                    }
                }
                app.cached_swaths.insert(
                    name.clone(),
                    (obs.sub_lat_deg, obs.sub_lon_deg, swath_points),
                );
            }
        }
    }

    let tiles = match app.map_style {
        crate::app::MapStyle::OpenStreetMap => &mut app.tiles_osm,
        crate::app::MapStyle::CartoDark => &mut app.tiles_carto,
        crate::app::MapStyle::CartoPositron => &mut app.tiles_positron,
        crate::app::MapStyle::StamenTerrain => &mut app.tiles_terrain,
        crate::app::MapStyle::EsriImagery => &mut app.tiles_esri,
        crate::app::MapStyle::OpenTopoMap => &mut app.tiles_topo,
    };

    let map = Map::new(Some(tiles), &mut app.map_memory, Position::new(0.0, 0.0)).with_plugin(
        SatellitesPlugin {
            satellites: &app.satellites,
            selected_satellites: &app.selected_satellites,
            observer: &app.observer,
            show_orbital_trail: app.show_orbital_trail,
            swath_color: app.swath_color,
            swath_opacity: app.swath_opacity,
            cached_trails: &app.cached_trails,
            cached_swaths: &app.cached_swaths,
            current_observations: &app.current_observations,
        },
    );

    let _response = ui.add(map);

    // Repaint to 30 FPS to reduce SGP4 and map tile overhead dynamically
    ui.ctx()
        .request_repaint_after(std::time::Duration::from_millis(33));
}
