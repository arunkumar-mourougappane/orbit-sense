# Orbit Sense

A modern desktop application written in Rust to track and visualize man-made objects in space (like the ISS, satellites, and debris) in real-time.

## Features

- **Real-Time Tracking**: Automatically fetches active satellite Two-Line Element sets (TLEs) from CelesTrak (VISUAL group).
- **Orbit Propagation**: Uses the SGP4 algorithm to calculate accurate orbital positions and predict future trajectories up to 90 minutes ahead.
- **Interactive Map**: Displays a dynamic Slippy Map powered by OpenStreetMap and the `walkers` crate, allowing you to pan, zoom in, zoom out, and center the map.
- **Geocoding Search**: Set your observer position anywhere in the world by searching for a city name (e.g., "Houston, TX") using Nominatim.
- **Satellite Filtering**: Search, select, and filter specific satellites by name to see their planned path overlaid on the map.
- **State Persistence**: The application caches your last zoomed area and searched observer location seamlessly between sessions.

## Tech Stack

- **Core**: Rust
- **GUI**: `egui` & `eframe`
- **Mapping**: `walkers`
- **Satellite Math**: `sgp4`
- **Geocoding**: `geocoding`
- **Async Runtime**: `tokio`
- **Requests**: `reqwest`

## Getting Started

Make sure you have [Rust and Cargo](https://rustup.rs/) installed, then run the app:

```bash
cargo run
```

## Usage

1. **Search Location**: Enter a city like "Paris, France" under Observer Location and hit `Search Location`. The map will jump to your location.
2. **Fetch TLEs**: Click `Refresh TLEs` in the sidebar to download the latest orbit tracking data.
3. **Select Satellite**: Type a name (e.g., "ISS") in the filter box or click a satellite in the list to highlight it and view its predicted orbital trajectory path in red on the map.
4. **Map Controls**: Use the float-in top right buttons to Zoom In, Zoom Out, or jump back to the origin of the selected Observer.
