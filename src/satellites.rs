//! Fetches satellite TLE data from CelesTrak and parses it into Orbit Sense data structures.

use serde::{Deserialize, Serialize};
use sgp4::{Constants, Elements};
use std::collections::HashMap;

/// Groups of tracked satellites available for download from CelesTrak.
///
/// Each variant corresponds to a named orbital dataset. Selecting a category
/// triggers a fresh TLE fetch from the matching CelesTrak URL.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum SatelliteCategory {
    /// The 100 visually brightest objects as seen from the ground.
    Visual,
    /// All operational Starlink broadband constellation satellites.
    Starlink,
    /// Satellites primarily used for meteorological observation.
    Weather,
    /// Operational GPS navigation satellites (Block II and later).
    Gps,
    /// Crewed and uncrewed orbital stations (ISS, CSS, etc.).
    SpaceStations,
}

impl SatelliteCategory {
    /// Returns the CelesTrak GP endpoint URL for this category's TLE dataset.
    pub fn to_url(&self) -> &'static str {
        match self {
            Self::Visual => "https://celestrak.org/NORAD/elements/gp.php?GROUP=visual&FORMAT=tle",
            Self::Starlink => {
                "https://celestrak.org/NORAD/elements/gp.php?GROUP=starlink&FORMAT=tle"
            }
            Self::Weather => "https://celestrak.org/NORAD/elements/gp.php?GROUP=weather&FORMAT=tle",
            Self::Gps => "https://celestrak.org/NORAD/elements/gp.php?GROUP=gps-ops&FORMAT=tle",
            Self::SpaceStations => {
                "https://celestrak.org/NORAD/elements/gp.php?GROUP=stations&FORMAT=tle"
            }
        }
    }

    /// Returns a short human-readable label used in UI drop-down menus.
    pub fn name(&self) -> &'static str {
        match self {
            Self::Visual => "Visual (100 Brightest)",
            Self::Starlink => "Starlink",
            Self::Weather => "Weather",
            Self::Gps => "GPS Operational",
            Self::SpaceStations => "Space Stations",
        }
    }
}

/// A parsed and propagation-ready tracking record for a single space object.
///
/// Constructed from a three-line TLE set; holds both the parsed `Elements`
/// (orbital parameters at epoch) and the pre-computed `Constants` needed by
/// SGP4 on every propagation call.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SpaceObject {
    /// The NORAD object name from the TLE header line (trimmed).
    pub name: String,
    /// SGP4 orbital elements at the TLE epoch.
    pub elements: Elements,
    /// Pre-computed SGP4 constants derived from the orbital elements.
    pub constants: Constants,
}

impl SpaceObject {
    /// Attempt to parse a three-line TLE sequence into a [`SpaceObject`].
    ///
    /// - `name`  — the header/object name line
    /// - `line1` — TLE line 1 (starts with `1`)
    /// - `line2` — TLE line 2 (starts with `2`)
    ///
    /// Returns `None` if parsing or SGP4 constant derivation fails.
    pub fn from_tle(name: &str, line1: &str, line2: &str) -> Option<Self> {
        let name = name.trim().to_string();
        let elements =
            Elements::from_tle(Some(name.clone()), line1.as_bytes(), line2.as_bytes()).ok()?;
        let constants = Constants::from_elements(&elements).ok()?;

        Some(Self {
            name,
            elements,
            constants,
        })
    }
}

/// Downloads and caches satellite TLE data for the given `category` from CelesTrak.
///
/// Parses the returned three-line-element text body into a
/// `HashMap<name, SpaceObject>`, skipping any malformed TLE triplets.
///
/// # Errors
/// Propagates any [`reqwest::Error`] encountered during the HTTP request or body read.
pub async fn fetch_active_satellites(
    category: SatelliteCategory,
) -> Result<HashMap<String, SpaceObject>, reqwest::Error> {
    let url = category.to_url();
    let response = reqwest::get(url).await?.text().await?;

    let mut objects = HashMap::new();
    let lines: Vec<&str> = response.lines().collect();

    for chunk in lines.chunks(3) {
        if chunk.len() == 3 {
            let name = chunk[0].trim();
            let line1 = chunk[1];
            let line2 = chunk[2];

            if let Some(obj) = SpaceObject::from_tle(name, line1, line2) {
                objects.insert(name.to_string(), obj);
            }
        }
    }

    Ok(objects)
}
