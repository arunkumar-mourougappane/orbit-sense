# 🛰 Orbit Sense

A real-time satellite tracking and visualization application written in **Rust**, featuring a live 2D world map, SGP4 orbital propagation, swath footprint rendering, and overhead pass prediction.

---

## Screenshot

![Orbit Sense - Satellite Tracking](assets/icon.png)

---

## Features

| Feature | Details |
|---|---|
| **Live TLE Data** | Downloads satellite datasets from [CelesTrak](https://celestrak.org) at startup |
| **Interactive 2D Map** | Pan and zoom world map powered by the `walkers` crate |
| **Orbital Trail** | Projects the satellite's path over the next 90 minutes |
| **Swath Footprint** | Displays the satellite's view-cone as a filled polygon on the map |
| **Pass Prediction** | Predicts when a satellite will pass within a configurable distance of your location |
| **Observer Location** | Geocode any city or address via OpenStreetMap Nominatim |
| **Map Themes** | Switch between Light (OpenStreetMap) and Dark (CartoDB) basemaps |
| **Satellite Categories** | Visual, Starlink, Weather, GPS, Space Stations |
| **Persistent Settings** | Preferences and observer location are saved across restarts |

---

## Getting Started

### Prerequisites

- [Rust toolchain](https://rustup.rs/) (stable, 2024 edition)
- A working internet connection (for TLE data and map tiles)

### Build & Run

```bash
git clone https://github.com/arunkumar-mourougappane/orbit-sense.git
cd orbit-sense
cargo run
```

For a release build:

```bash
cargo build --release
./target/release/orbit-sense
```

---

## Usage

1. **Select a Satellite Category** from the dropdown in the left sidebar (defaults to *Visual — 100 Brightest*). TLEs are downloaded automatically on startup.
2. **Search** for a satellite by name using the filter box.
3. **Click a satellite** in the list to select it. The map centers on its current position and its swath footprint and orbital trail are drawn.
4. **Set your Observer Location** by typing a city name or address (e.g. `Houston, TX`) and clicking **Search Location**. The app will then predict the next overhead pass for the selected satellite.
5. **Customize** via *File → Preferences*:
   - Map theme (Light / Dark)
   - Show/hide orbital trail
   - Swath footprint color and opacity
   - Pass distance threshold (km)
6. **Help → About** displays application metadata, license, and repository link.

---

## Architecture

```
orbit-sense/
├── src/
│   ├── lib.rs            # Crate root; feature overview and module map
│   ├── constants.rs      # Application-wide numeric constants
│   ├── location.rs       # Geocoding, geodetic maths, SGP4 observation calc
│   ├── satellites.rs     # CelesTrak TLE fetch & parse
│   ├── app.rs            # Core app state, eframe::App impl, settings persistence
│   └── ui/
│       ├── mod.rs        # UI module registry
│       ├── about.rs      # About dialog
│       ├── map.rs        # 2D map rendering, swath footprint, orbital trail
│       ├── overlay.rs    # Floating map controls and satellite info panel
│       ├── preferences.rs# Preferences window
│       └── sidebar.rs    # Left sidebar: location, category, satellite list
├── assets/
│   └── icon.png          # Application icon (embedded into binary at compile time)
└── Cargo.toml
```

### Key Crates

| Crate | Purpose |
|---|---|
| `eframe` / `egui` | Immediate-mode GUI framework |
| `walkers` | Slippy-map tile rendering for egui |
| `sgp4` | Satellite orbital propagation (SGP4/SDP4) |
| `tokio` | Async runtime for TLE downloads and geocoding |
| `reqwest` | HTTP client for CelesTrak and Nominatim |
| `geocoding` | OpenStreetMap Nominatim geocoding |
| `serde` / `serde_json` | Serialization for settings persistence |
| `image` | PNG decoding for the embedded window icon |

---

## Settings Persistence

Preferences and observer location are automatically saved using `eframe`'s built-in cross-platform storage. No manual save step is required.

| Platform | Storage Path |
|---|---|
| Linux | `~/.local/share/orbit-sense/` |
| macOS | `~/Library/Application Support/orbit-sense/` |
| Windows | `%APPDATA%\orbit-sense\` |

---

## Satellite Propagation Notes

- Orbital positions are computed using the **SGP4** model via the `sgp4` crate.
- TLE data is sourced from [CelesTrak](https://celestrak.org) and refreshed on every launch.
- TEME-to-ECEF conversion uses a simplified GMST approximation sufficient for visualization purposes.
- The swath (view-cone) footprint is derived from the satellite's altitude using Earth's mean radius (6,371 km).

---

## License

This project is licensed under the **MIT License**. See [`LICENSE`](LICENSE) for details.

---

## Author

**Arunkumar Mourougappane** — [amouroug.dev@gmail.com](mailto:amouroug.dev@gmail.com)

Repository: [github.com/arunkumar-mourougappane/orbit-sense](https://github.com/arunkumar-mourougappane/orbit-sense)

---

*Satellite data sourced from CelesTrak · Orbital propagation via the SGP4 model*
