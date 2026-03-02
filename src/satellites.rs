//! Fetches satellite TLE data from CelesTrak and parses it into Orbit Sense data structures.

use serde::{Deserialize, Serialize};
use sgp4::{Constants, Elements};
use std::collections::HashMap;

/// Groups of tracked satellites available for download from CelesTrak.
///
/// Each variant corresponds to a named orbital dataset. Selecting a category
/// triggers a fresh TLE fetch from the matching CelesTrak URL. Categories are
/// organised into five logical groupings:
///
/// - **General** — popular hand-picked sets
/// - **Government/Science** — weather, GPS, research
/// - **Commercial Telecom** — established operators (Iridium, Orbcomm, etc.)
/// - **Broadband Constellations** — high-throughput LEO mega-constellations
/// - **Earth Observation** — imaging and remote-sensing operators
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum SatelliteCategory {
    // ── General ─────────────────────────────────────────────────────────────
    /// The 100 visually brightest objects as seen from the ground.
    Visual,
    /// Crewed and uncrewed orbital stations (ISS, CSS, Tiangong, etc.).
    SpaceStations,
    /// All objects in geostationary / geosynchronous orbits.
    Geostationary,

    // ── Government / Science ─────────────────────────────────────────────────
    /// NOAA operational meteorological satellites.
    Noaa,
    /// Satellites primarily used for meteorological observation (all agencies).
    Weather,
    /// Operational GPS navigation satellites.
    Gps,
    /// GLONASS navigation constellation (Russia).
    Glonass,
    /// Galileo navigation constellation (EU).
    Galileo,
    /// BeiDou navigation constellation (China).
    Beidou,
    /// Amateur radio satellites (AMSAT and partners).
    AmateurRadio,
    /// CubeSats and nano-satellites.
    CubeSats,
    /// Scientific research and exploration missions.
    Science,

    // ── Commercial Telecom ───────────────────────────────────────────────────
    /// Iridium voice and data constellation (original + NEXT).
    IridiumNext,
    /// Orbcomm machine-to-machine (M2M) data satellites.
    Orbcomm,
    /// Globalstar mobile phone and data constellation.
    Globalstar,
    /// Intelsat geostationary and flexible-orbit fleet.
    Intelsat,
    /// SES GEO and MEO communication satellites.
    Ses,
    /// Telesat GEO and LEO satellites.
    Telesat,
    /// SES O3b MEO broadband constellation.
    O3b,

    // ── Broadband Constellations ─────────────────────────────────────────────
    /// SpaceX Starlink broadband mega-constellation.
    Starlink,
    /// OneWeb LEO broadband constellation.
    OneWeb,
    /// Amazon Kuiper broadband constellation (where available).
    Kuiper,

    // ── Earth Observation ────────────────────────────────────────────────────
    /// Planet Labs Dove and SuperDove imaging cubesats.
    Planet,
    /// Spire Global weather, ship-tracking, and aviation analytics satellites.
    Spire,
}

impl SatelliteCategory {
    /// Returns the CelesTrak GP endpoint URL for this category's TLE dataset.
    pub fn to_url(&self) -> &'static str {
        match self {
            Self::Visual => "https://celestrak.org/NORAD/elements/gp.php?GROUP=visual&FORMAT=tle",
            Self::SpaceStations => {
                "https://celestrak.org/NORAD/elements/gp.php?GROUP=stations&FORMAT=tle"
            }
            Self::Geostationary => {
                "https://celestrak.org/NORAD/elements/gp.php?GROUP=geo&FORMAT=tle"
            }
            Self::Noaa => "https://celestrak.org/NORAD/elements/gp.php?GROUP=noaa&FORMAT=tle",
            Self::Weather => "https://celestrak.org/NORAD/elements/gp.php?GROUP=weather&FORMAT=tle",
            Self::Gps => "https://celestrak.org/NORAD/elements/gp.php?GROUP=gps-ops&FORMAT=tle",
            Self::Glonass => "https://celestrak.org/NORAD/elements/gp.php?GROUP=glo-ops&FORMAT=tle",
            Self::Galileo => "https://celestrak.org/NORAD/elements/gp.php?GROUP=galileo&FORMAT=tle",
            Self::Beidou => "https://celestrak.org/NORAD/elements/gp.php?GROUP=beidou&FORMAT=tle",
            Self::AmateurRadio => {
                "https://celestrak.org/NORAD/elements/gp.php?GROUP=amateur&FORMAT=tle"
            }
            Self::CubeSats => {
                "https://celestrak.org/NORAD/elements/gp.php?GROUP=cubesat&FORMAT=tle"
            }
            Self::Science => "https://celestrak.org/NORAD/elements/gp.php?GROUP=science&FORMAT=tle",
            Self::IridiumNext => {
                "https://celestrak.org/NORAD/elements/gp.php?GROUP=iridium-NEXT&FORMAT=tle"
            }
            Self::Orbcomm => "https://celestrak.org/NORAD/elements/gp.php?GROUP=orbcomm&FORMAT=tle",
            Self::Globalstar => {
                "https://celestrak.org/NORAD/elements/gp.php?GROUP=globalstar&FORMAT=tle"
            }
            Self::Intelsat => {
                "https://celestrak.org/NORAD/elements/gp.php?GROUP=intelsat&FORMAT=tle"
            }
            Self::Ses => "https://celestrak.org/NORAD/elements/gp.php?GROUP=ses&FORMAT=tle",
            Self::Telesat => "https://celestrak.org/NORAD/elements/gp.php?GROUP=telesat&FORMAT=tle",
            Self::O3b => "https://celestrak.org/NORAD/elements/gp.php?GROUP=o3b&FORMAT=tle",
            Self::Starlink => {
                "https://celestrak.org/NORAD/elements/gp.php?GROUP=starlink&FORMAT=tle"
            }
            Self::OneWeb => "https://celestrak.org/NORAD/elements/gp.php?GROUP=oneweb&FORMAT=tle",
            Self::Kuiper => "https://celestrak.org/NORAD/elements/gp.php?GROUP=kuiper&FORMAT=tle",
            Self::Planet => "https://celestrak.org/NORAD/elements/gp.php?GROUP=planet&FORMAT=tle",
            Self::Spire => "https://celestrak.org/NORAD/elements/gp.php?GROUP=spire&FORMAT=tle",
        }
    }

    /// Returns a short human-readable label used in UI drop-down menus.
    pub fn name(&self) -> &'static str {
        match self {
            Self::Visual => "Visual (100 Brightest)",
            Self::SpaceStations => "Space Stations",
            Self::Geostationary => "Geostationary",
            Self::Noaa => "NOAA",
            Self::Weather => "Weather",
            Self::Gps => "GPS",
            Self::Glonass => "GLONASS",
            Self::Galileo => "Galileo",
            Self::Beidou => "BeiDou",
            Self::AmateurRadio => "Amateur Radio",
            Self::CubeSats => "CubeSats",
            Self::Science => "Science",
            Self::IridiumNext => "Iridium NEXT",
            Self::Orbcomm => "Orbcomm",
            Self::Globalstar => "Globalstar",
            Self::Intelsat => "Intelsat",
            Self::Ses => "SES",
            Self::Telesat => "Telesat",
            Self::O3b => "O3b (SES MEO)",
            Self::Starlink => "Starlink",
            Self::OneWeb => "OneWeb",
            Self::Kuiper => "Amazon Kuiper",
            Self::Planet => "Planet Labs",
            Self::Spire => "Spire Global",
        }
    }

    /// Returns the display-group heading this category belongs to.
    /// Used to render section separators in the UI dropdown.
    pub fn group_label(&self) -> &'static str {
        match self {
            Self::Visual | Self::SpaceStations | Self::Geostationary => "General",
            Self::Noaa
            | Self::Weather
            | Self::Gps
            | Self::Glonass
            | Self::Galileo
            | Self::Beidou
            | Self::AmateurRadio
            | Self::CubeSats
            | Self::Science => "Government / Science",
            Self::IridiumNext
            | Self::Orbcomm
            | Self::Globalstar
            | Self::Intelsat
            | Self::Ses
            | Self::Telesat
            | Self::O3b => "Commercial Telecom",
            Self::Starlink | Self::OneWeb | Self::Kuiper => "Broadband Constellations",
            Self::Planet | Self::Spire => "Earth Observation",
        }
    }

    /// All categories in display order, grouped sequentially.
    pub fn all() -> &'static [SatelliteCategory] {
        &[
            // General
            Self::Visual,
            Self::SpaceStations,
            Self::Geostationary,
            // Government / Science
            Self::Noaa,
            Self::Weather,
            Self::Gps,
            Self::Glonass,
            Self::Galileo,
            Self::Beidou,
            Self::AmateurRadio,
            Self::CubeSats,
            Self::Science,
            // Commercial Telecom
            Self::IridiumNext,
            Self::Orbcomm,
            Self::Globalstar,
            Self::Intelsat,
            Self::Ses,
            Self::Telesat,
            Self::O3b,
            // Broadband Constellations
            Self::Starlink,
            Self::OneWeb,
            Self::Kuiper,
            // Earth Observation
            Self::Planet,
            Self::Spire,
        ]
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
