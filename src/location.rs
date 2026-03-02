//! Provides geographic utilities, geocoding for cities, and TLE orbital calculations.

use chrono::{DateTime, Utc};
use geocoding::{Forward, Openstreetmap};
use serde::{Deserialize, Serialize};
use sgp4::{Constants, Elements};

/// Represents a geographic location on Earth (latitude, longitude, altitude).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Location {
    /// Human-readable identifier for this location (e.g. "Houston, TX").
    pub name: String,
    /// Geodetic latitude in degrees (North is positive).
    pub lat_deg: f64,
    /// Geodetic longitude in degrees (East is positive).
    pub lon_deg: f64,
    /// Altitude above mean sea level in meters.
    pub alt_m: f64,
}

impl Location {
    /// Attempt to geocode a string like "City, Country" into a Location.
    /// Uses OpenStreetMap's Nominatim, so we need to provide an app user agent.
    pub async fn from_query(query: &str) -> Result<Self, String> {
        let query_clone = query.to_string();
        let mut results = match tokio::task::spawn_blocking(move || {
            let openstreetmap = Openstreetmap::new();
            openstreetmap.forward(&query_clone)
        })
        .await
        {
            Ok(Ok(locs)) => locs,
            Ok(Err(e)) => return Err(format!("Geocoding API error: {}", e)),
            Err(e) => return Err(format!("Task error: {}", e)),
        };

        if let Some(res) = results.pop() {
            Ok(Self {
                name: query.to_string(),
                lat_deg: res.y(),
                lon_deg: res.x(),
                alt_m: 0.0, // Nominatim doesn't provide altitude, default to sea level
            })
        } else {
            Err("Location not found".to_string())
        }
    }
}

/// Computes the great-circle distance between two points on a sphere (Earth) in kilometers.
pub fn haversine_distance(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
    let r = crate::constants::EARTH_RADIUS_KM;
    let dlat = (lat2 - lat1).to_radians();
    let dlon = (lon2 - lon1).to_radians();
    let a = (dlat / 2.0).sin().powi(2)
        + lat1.to_radians().cos() * lat2.to_radians().cos() * (dlon / 2.0).sin().powi(2);
    let c = 2.0 * a.sqrt().atan2((1.0 - a).sqrt());
    r * c
}

/// Asynchronously predicts all times the given satellite will pass within
/// the `threshold_km` distance of the provided `obs` Location over the next 48 hours.
pub async fn predict_next_pass(
    sat: crate::satellites::SpaceObject,
    obs: Location,
    threshold_km: f64,
) -> Vec<(DateTime<Utc>, f64)> {
    tokio::task::spawn_blocking(move || {
        let mut passes = Vec::new();
        let now = Utc::now();
        let mut in_pass = false;
        let mut pass_best_dist = f64::MAX;
        let mut pass_best_time = now;

        // Search next 48 hours (48 * 60 = 2880 minutes)
        // Use dynamic stepping based on distance to massive reduce SGP4 math while guaranteeing
        // we never skip over a pass. Max ground speed of a satellite is roughly 450 km/minute.
        let mut min = 1;
        while min <= 2880 {
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
                let dist = haversine_distance(
                    obs.lat_deg,
                    obs.lon_deg,
                    pass_obs.sub_lat_deg,
                    pass_obs.sub_lon_deg,
                );

                let buffer = dist - threshold_km;

                if in_pass {
                    if dist < threshold_km {
                        // Still in pass, find the peak
                        if dist < pass_best_dist {
                            pass_best_dist = dist;
                            pass_best_time = future_t;
                        }
                        min += 1;
                    } else {
                        // Pass ended
                        passes.push((pass_best_time, pass_best_dist));
                        in_pass = false;
                        pass_best_dist = f64::MAX;
                        min += 1; // Resume search with 1 min to allow safe_step calc next loop
                    }
                } else if dist < threshold_km {
                    // We entered the pass radius
                    in_pass = true;
                    pass_best_dist = dist;
                    pass_best_time = future_t;
                    min += 1; // Tracing pass minute-by-minute
                } else {
                    // We are outside. How many minutes can we safely skip?
                    // Assuming 450 km/min max ground speed.
                    let safe_step = (buffer / 450.0).floor() as i64;
                    let step = safe_step.clamp(1, 15); // Step by at least 1, up to 15 mins max
                    min += step;
                }
            } else {
                min += 1; // If error, just advance 1 min
            }
        }

        if in_pass {
            passes.push((pass_best_time, pass_best_dist));
        }

        passes
    })
    .await
    .unwrap_or_default()
}

/// The computed geodetic position of a satellite at a specific time.
#[derive(Debug, Clone)]
pub struct Observation {
    /// Timestamp for when this observation is valid.
    #[allow(dead_code)]
    pub time: DateTime<Utc>,
    /// Sub-satellite latitude — geodetic latitude of the point on Earth directly below the satellite.
    pub sub_lat_deg: f64,
    /// Sub-satellite longitude — geodetic longitude of the point on Earth directly below the satellite.
    pub sub_lon_deg: f64,
    /// Line-of-sight distance from the coordinate center to the satellite in kilometers.
    #[allow(dead_code)]
    pub range_km: f64,
    /// Altitude above the WGS-84 ellipsoid surface, in kilometres.
    pub altitude_km: f64,
    /// Magnitude of the velocity vector, in km/s.
    pub velocity_km_s: f64,
}

/// Calculates where a satellite is in the sky relative to an observer on Earth.
/// Uses the wgs84 constants directly.
pub fn calculate_observation(
    elements: &Elements,
    constants: &Constants,
    _observer: &Location,
    time: DateTime<Utc>,
) -> Option<Observation> {
    // Calculate minutes since the TLE epoch using the built-in function
    let naive_time = time.naive_utc();
    let minutes_since_epoch = elements.datetime_to_minutes_since_epoch(&naive_time).ok()?;

    // Propagate the satellite state vector to the requested time
    let prediction = constants.propagate(minutes_since_epoch).ok()?;

    // Convert prediction vectors (which are in TEME frame) to Geodetic
    // (This is an approximation assuming TEME ~ ECEF for a simplified visualizer)

    // WGS84 constants
    let a = 6378.137; // Semi-major axis (km)
    let f = 1.0 / 298.257223563; // Flattening
    let e2 = f * (2.0 - f); // Eccentricity squared

    let r = prediction.position;
    let x = r[0]; // km
    let y = r[1]; // km
    let z = r[2]; // km

    // Convert to lat/lon
    // Longitude
    let mut lon = y.atan2(x);

    // Approximate Greenwich Mean Sidereal Time (GMST) to rotate TEME to Earth fixed
    // A proper implementation needs the actual GMST equation, here's a highly simplifed rotate based on day fraction
    let julian_date = time.timestamp() as f64 / 86400.0 + 2440587.5;
    let t = (julian_date - 2451545.0) / 36525.0;
    let mut gmst = 280.46061837 + 360.98564736629 * (julian_date - 2451545.0) + t * t * 0.000387933
        - t * t * t / 38710000.0;
    gmst = (gmst % 360.0) * std::f64::consts::PI / 180.0;

    lon = (lon - gmst) % (2.0 * std::f64::consts::PI);
    if lon > std::f64::consts::PI {
        lon -= 2.0 * std::f64::consts::PI;
    }
    if lon < -std::f64::consts::PI {
        lon += 2.0 * std::f64::consts::PI;
    }

    // Latitude
    let p = (x * x + y * y).sqrt();
    let mut lat = (z / (p * (1.0 - e2))).atan(); // Initial guess

    // Iterate for precision
    let mut c;
    for _ in 0..5 {
        c = a / (1.0 - e2 * lat.sin().powi(2)).sqrt();
        lat = ((z + c * e2 * lat.sin()) / p).atan();
    }

    // Convert out to degrees
    let lat_deg = lat * 180.0 / std::f64::consts::PI;
    let lon_deg = lon * 180.0 / std::f64::consts::PI;

    let altitude_km = (x * x + y * y + z * z).sqrt() - a;
    let v = prediction.velocity;
    let velocity_km_s = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();

    Some(Observation {
        time,
        sub_lat_deg: lat_deg,
        sub_lon_deg: lon_deg,
        range_km: p,
        altitude_km,
        velocity_km_s,
    })
}
