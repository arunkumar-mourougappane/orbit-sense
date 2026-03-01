//! Core 3D map rendering logic using `macroquad`.
use crate::app::OrbitSenseApp;
use crate::location::calculate_observation;
use chrono::Utc;
use macroquad::prelude::*;

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
