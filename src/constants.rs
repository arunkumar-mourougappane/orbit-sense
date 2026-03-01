//! Application-wide magic numbers and constants.

/// The average radius of the Earth in kilometers.
pub const EARTH_RADIUS_KM: f64 = 6371.0;

/// Number of minutes to predict forward when drawing the trailing orbital path.
pub const ORBITAL_TRAIL_MINUTES: i64 = 90;

/// Maximum number of minutes to look ahead when predicting the next overhead pass.
pub const PASS_SEARCH_MINUTES: i64 = 1440;

/// Default distance (in km) from the observer's location to consider a satellite "overhead".
pub const DEFAULT_PASS_THRESHOLD_KM: f64 = 2000.0;
