use chrono::{DateTime, Utc};
use geocoding::{Forward, Openstreetmap};
use serde::{Deserialize, Serialize};
use sgp4::{Constants, Elements};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Location {
    pub name: String,
    pub lat_deg: f64,
    pub lon_deg: f64,
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

pub fn haversine_distance(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
    let r = crate::constants::EARTH_RADIUS_KM;
    let dlat = (lat2 - lat1).to_radians();
    let dlon = (lon2 - lon1).to_radians();
    let a = (dlat / 2.0).sin().powi(2)
        + lat1.to_radians().cos() * lat2.to_radians().cos() * (dlon / 2.0).sin().powi(2);
    let c = 2.0 * a.sqrt().atan2((1.0 - a).sqrt());
    r * c
}

pub async fn predict_next_pass(
    sat: crate::satellites::SpaceObject,
    obs: Location,
    threshold_km: f64,
) -> Option<(DateTime<Utc>, f64)> {
    tokio::task::spawn_blocking(move || {
        let mut next_pass = None;
        let now = Utc::now();
        for min in 1..=crate::constants::PASS_SEARCH_MINUTES {
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
                    pass_obs.elevation_deg,
                    pass_obs.azimuth_deg,
                );

                if dist < threshold_km {
                    next_pass = Some((future_t, dist));
                    break;
                }
            }
        }
        next_pass
    })
    .await
    .unwrap_or(None)
}

/// A simplified observation at a specific time
#[derive(Debug, Clone)]
pub struct Observation {
    #[allow(dead_code)]
    pub time: DateTime<Utc>,
    pub elevation_deg: f64,
    pub azimuth_deg: f64,
    #[allow(dead_code)]
    pub range_km: f64,
}

/// Calculates where a satellite is in the sky relative to an observer on Earth.
/// Uses the wgs84 constants directly.
pub fn calculate_observation(
    elements: &Elements,
    constants: &Constants,
    _observer: &Location,
    time: DateTime<Utc>,
) -> Option<Observation> {
    // Calculate minutes since the TLE epoch using Julian dates
    let current_jd = time.timestamp() as f64 / 86400.0 + 2440587.5;
    let epoch_jd = elements.epoch() + 2440587.5; // elements.epoch() returns days from 1949 Dec 31 00:00 UT

    // Convert to minutes
    let minutes_since_epoch = (current_jd - epoch_jd) * 1440.0;
    // Propagate the satellite state vector to the requested time
    let prediction = constants
        .propagate(sgp4::MinutesSinceEpoch(minutes_since_epoch))
        .ok()?;

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

    Some(Observation {
        time,
        elevation_deg: lat_deg, // Repurposing these struct fields for passing visual tracking coordiantes to UI
        azimuth_deg: lon_deg,
        range_km: p,
    })
}
