# Orbit Sense

A modern desktop application written in Rust to track and visualize man-made objects in space (like the ISS, satellites, and debris) in real-time. Use Orbit Sense to find out exactly where a satellite is, and when it will fly overhead next.

## Features

- **Real-Time Position Tracking**: Automatically fetches active satellite Two-Line Element sets (TLEs) from CelesTrak (VISUAL group). The map renders smooth, real-time orbital pathing as the objects glide across the globe.
- **Spacecraft Details Overlay**: Click the `ℹ` button to bring up a Heads Up Display that computes real-time math ticks on the satellite's exact geocentric coordinates:
  - Live Altitude & Velocity (km/s)
  - Latitude & Longitude Coordinates
  - Mathematical Orbital Mechanics (Eccentricity, Mean Anomaly, Arg of Perigee, Inclination, etc.)
- **Overhead Pass Prediction**: The application uses the Haversine formula to asynchronously scan 24 hours into the future, predicting exactly when a satellite will fly over your chosen observer location.
- **Interactive Map**: Displays a dynamic Slippy Map powered by OpenStreetMap and `walkers`. Pan, zoom, center on an observer, or force the map to "Fit to Window" instantly.
- **Geocoding Search**: Set your observer position anywhere in the world by searching for a city name (e.g., "Houston, TX") using the Nominatim API.
- **Settings & Preferences**: Customize the threshold for an "overhead pass" in `File > Preferences` or toggle orbital trails on/off.
- **State Persistence**: The application caches your last zoomed area, preferences, searched observer location, and TLE datasets seamlessly between sessions.

## Tech Stack & Architecture

Structured as a standard Rust `lib` + `bin`, making it portable and easy to extend.

- **Core**: Rust (`src/lib.rs` and `src/bin/orbit-sense.rs`)
- **GUI Framework**: `egui` & `eframe`
- **Mapping**: `walkers`
- **Satellite Propagation**: `sgp4`
- **Geocoding**: `geocoding`
- **Async Runtime**: `tokio`
- **Web Requests**: `reqwest`

## Installation

Make sure you have [Rust and Cargo](https://rustup.rs/) installed. You can install the application directly from source:

```bash
cargo install --path .
```

Or just run it in development mode:

```bash
cargo run
```

## Usage

1. **Search Location**: Enter a city like "Paris, France" under Observer Location and hit `Search Location`. The map will jump to your location.
2. **Fetch TLEs**: Click `Refresh TLEs` in the sidebar to download the latest orbit tracking data.
3. **Select Satellite**: Type a name (e.g., "ISS") in the filter box or click a satellite in the list to highlight it and view its predicted orbital trajectory path in red on the map.
4. **View Telemetry Window**: Click the `ℹ` icon in the bottom right corner map controls to bring up the Spacecraft Details window. Watch the altitude and velocity numbers update in real-time.
5. **Map Controls**: Use the float-in bottom right buttons to Zoom In (`➕`), Zoom Out (`➖`), Max Zoomout (`🌐`), Center on Observer (`📍`), or Fit Map to Window (`🗺`).
