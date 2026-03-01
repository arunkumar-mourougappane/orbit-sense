use serde::{Deserialize, Serialize};
use sgp4::{Constants, Elements};
use std::collections::HashMap;

/// A space object tracking record.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SpaceObject {
    #[allow(dead_code)]
    pub name: String,
    pub elements: Elements,
    pub constants: Constants,
}

impl SpaceObject {
    /// Attempt to parse a 3-line TLE sequence into a SpaceObject.
    /// line1: Name
    /// line2: TLE Line 1
    /// line3: TLE Line 2
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

/// Fetch current active satellites from CelesTrak.
/// Returns a map of Object Name to SpaceObject.
pub async fn fetch_active_satellites() -> Result<HashMap<String, SpaceObject>, reqwest::Error> {
    // 100 brightest satellites is a good default dataset that isn't too huge
    let url = "https://celestrak.org/NORAD/elements/gp.php?GROUP=visual&FORMAT=tle";
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
