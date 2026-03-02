# Release Notes - Orbit Sense v0.1.1

We are excited to announce the release of **Orbit Sense v0.1.1**! This update focuses on significant performance optimizations, critical rendering fixes for global tracking, and a massive overhaul of internal documentation.

## 🚀 Key Highlights

### ⚡ Substantial Performance Gains

- **Dynamic Orbital Prediction**: The overhead pass predictor now uses "Intelligent Striding". Instead of calculating every single minute for the next 48 hours, it dynamically adjusts its calculation frequency based on the satellite's distance from the observer. This results in **near-instantaneous pass predictions** with significantly lower CPU usage.
- **Optimized Footprint Rendering**: We removed expensive and redundant coordinate caching for satellite swaths. Footprints are now calculated in real-time within the render loop, ensuring the "view cone" always stays perfectly centered on the satellite regardless of its velocity.

### 🗺️ Map & Tracking Improvements

- **Improved Global Wrapping**: Fixed a critical issue where satellites and footprints would "break" or duplicate when crossing the $\pm180^\circ$ longitude anti-meridian. The rendering engine now seamlessly handles wrap-around geometry for global tracking.
- **UX Polish**: The floating map toolbox (Zoom, Auto-track, Info) has been shifted slightly upward to improve visibility and prevent overlap with the bottom status overlay.
- **Marker Accuracy**: Reverted experimental "phantom rendering" that caused ghost satellite markers to appear in empty space. Markers are now 100% accurate to their SGP4-calculated TLE positions.

### 📚 Documentation & Stability

- **Full API Documentation**: Every core struct and field in the application (including `OrbitSenseApp`, `AppSettings`, and `Observation` types) is now fully documented using Rustdoc.
- **Architectural Transparency**: The `README.md` has been expanded with a new section on **Satellite Propagation**, explaining the math and logic behind our real-time visualizations.
- **Clean Build**: The project now maintains strict compliance with `clippy` and `missing_docs` lints, ensuring a more stable and maintainable codebase.

---

## 🛠️ Detailed Change Log

### ✨ Added

- Inline documentation for all public and private data structures.
- Technical documentation in `README.md` regarding propagation and map-wrapping.
- New `CHANGELOG.md` for historical version tracking.

### 🔧 Changed

- Map toolbox anchor offset moved from `-10.0` to `-40.0` on the Y-axis.
- `predict_next_pass` algorithm now uses variable time-steps (1 to 15 mins) based on distance.

### 🐛 Fixed

- Duplicated "ghost" satellite markers near the map edges.
- Satellite footprints detaching or lagging behind the spacecraft at high altitudes.
- Broken intra-doc links in the source code.

---

## 📥 How to Update

If you are running from source:

```bash
git pull origin main
cargo run --release
```

---
*Thank you for using Orbit Sense! Clear skies and happy tracking.*
