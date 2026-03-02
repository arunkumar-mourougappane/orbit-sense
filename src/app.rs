//! Main application state and startup logic for Orbit Sense.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tokio::runtime::Handle;
use tokio::sync::mpsc::{self, Receiver, Sender};

use crate::location::Location;
use crate::satellites::{SpaceObject, fetch_active_satellites};

/// Messages passed from background asynchronous tasks back to the UI thread.
use walkers::{HttpOptions, HttpTiles, MapMemory, Position};

pub enum AppMessage {
    /// Received a payload containing the successfully parsed Celestrak dataset, or an error.
    SatellitesLoaded(Result<HashMap<String, SpaceObject>, String>),
    /// Received a parsed lat/lon coordinate from Nominatim, or an error.
    LocationGeocoded(Result<Location, String>),
    /// Emitted after `predict_next_pass` completes evaluating the upcoming 48 hours.
    PassPredicted(Vec<(chrono::DateTime<chrono::Utc>, f64)>),
}

#[derive(PartialEq, Clone, Serialize, Deserialize)]
pub enum MapStyle {
    OpenStreetMap,
    CartoDark,
}

// Removed duplicate MapStyle

/// Serializable snapshot of user preferences saved across sessions.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct AppSettings {
    pub map_style: Option<String>,
    pub show_orbital_trail: Option<bool>,
    pub swath_color: Option<[f32; 3]>,
    pub swath_opacity: Option<f32>,
    pub pass_threshold_km: Option<f64>,
    pub satellite_category: Option<crate::satellites::SatelliteCategory>,
    pub observer_name: Option<String>,
    pub observer_lat: Option<f64>,
    pub observer_lon: Option<f64>,
    pub observer_alt: Option<f64>,
    pub location_query: Option<String>,
}

impl AppSettings {
    const KEY: &'static str = "orbit_sense_settings";

    /// Load settings from eframe's cross-platform storage.
    pub fn load(storage: &dyn eframe::Storage) -> Self {
        eframe::get_value(storage, Self::KEY).unwrap_or_default()
    }

    /// Save settings into eframe's cross-platform storage.
    pub fn save(&self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, Self::KEY, self);
    }
}

/// Custom tile provider using free CartoDB Voyager endpoints.
/// See: https://github.com/CartoDB/basemap-styles
pub struct CartoDark;

impl walkers::sources::TileSource for CartoDark {
    fn tile_url(&self, tile_id: walkers::TileId) -> String {
        format!(
            "https://basemaps.cartocdn.com/rastertiles/dark_all/{}/{}/{}.png",
            tile_id.zoom, tile_id.x, tile_id.y
        )
    }

    fn attribution(&self) -> walkers::sources::Attribution {
        walkers::sources::Attribution {
            text: "© OpenStreetMap contributors © CARTO",
            url: "https://carto.com/attributions",
            logo_light: None,
            logo_dark: None,
        }
    }
}

/// The core application state running on the `eframe` GUI loop.
pub struct OrbitSenseApp {
    // Data state
    pub satellites: HashMap<String, SpaceObject>,
    pub selected_satellite: Option<String>,
    pub search_query: String,
    pub filtered_satellites: Vec<String>,
    pub satellite_category: crate::satellites::SatelliteCategory,
    pub last_updated: Option<chrono::DateTime<chrono::Local>>,

    // Observer state
    pub observer: Option<Location>,
    pub location_query: String,
    pub last_predicted_passes: Vec<(chrono::DateTime<chrono::Utc>, f64)>,

    // UI state
    pub show_satellite_info: bool,
    pub preferences_open: bool,
    pub show_about: bool,
    pub show_orbital_trail: bool,
    pub camera_locked: bool,
    pub pass_threshold_km: f64,
    pub swath_color: [f32; 3],
    pub swath_opacity: f32,
    pub map_style: MapStyle,
    pub map_memory: MapMemory,
    pub tiles_osm: HttpTiles,
    pub tiles_carto: HttpTiles,
    pub rt: Handle,

    // Async communications
    pub tx: Sender<AppMessage>,
    pub rx: Receiver<AppMessage>,

    pub fetch_in_progress: bool,
    pub location_in_progress: bool,
    pub is_predicting_pass: bool,
    /// Error from the last satellite TLE download attempt.
    pub error_msg: Option<String>,
    /// Error from the last Observer Location geocoding attempt.
    pub location_error_msg: Option<String>,
    /// If true, satellite list is sorted alphabetically (default). If false, sorted by altitude descending.
    pub sort_alpha: bool,
    /// Set to true for one frame to move keyboard focus to the filter box.
    pub focus_filter: bool,
    /// Caches the heavy SGP4 math for the selected satellite's orbital trail. (time, name, trail)
    pub cached_trail: Option<(
        chrono::DateTime<chrono::Utc>,
        String,
        Vec<walkers::Position>,
    )>,
    /// Caches the mathematical calculation for the satellite swath.
    pub cached_swath: Option<(
        chrono::DateTime<chrono::Utc>,
        String,
        Vec<walkers::Position>,
    )>,
}

impl OrbitSenseApp {
    /// Constructs the initial state of the application.
    /// Injects `reqwest` headers for Celestrak mapping and pulls initial cache.
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let (tx, rx) = mpsc::channel(100);

        // Setup walkers tiles manager
        let options = HttpOptions {
            user_agent: Some(reqwest::header::HeaderValue::from_static(
                "orbit-sense/0.1 (amouroug@gemini.local)",
            )),
            ..Default::default()
        };

        let options2 = HttpOptions {
            user_agent: Some(reqwest::header::HeaderValue::from_static(
                "orbit-sense/0.1 (amouroug@gemini.local)",
            )),
            ..Default::default()
        };

        let tiles_osm = HttpTiles::with_options(
            walkers::sources::OpenStreetMap,
            options,
            cc.egui_ctx.clone(),
        );
        let tiles_carto = HttpTiles::with_options(CartoDark, options2, cc.egui_ctx.clone());

        let mut initial_map_memory = MapMemory::default();
        initial_map_memory.center_at(Position::new(0.0, 20.0));
        let _ = initial_map_memory.set_zoom(2.5);

        let mut app = Self {
            satellites: HashMap::new(),
            selected_satellite: None,
            search_query: String::new(),
            filtered_satellites: Vec::new(),
            satellite_category: crate::satellites::SatelliteCategory::Visual,
            last_updated: None,
            observer: None,
            location_query: String::new(),
            last_predicted_passes: Vec::new(),
            show_satellite_info: false,
            preferences_open: false,
            show_about: false,
            show_orbital_trail: true,
            camera_locked: false,
            pass_threshold_km: crate::constants::DEFAULT_PASS_THRESHOLD_KM,
            swath_color: [0.78, 0.78, 0.78],
            swath_opacity: 0.16,
            map_style: MapStyle::CartoDark,
            map_memory: initial_map_memory,
            tiles_osm,
            tiles_carto,
            rt: Handle::current(),
            tx,
            rx,
            fetch_in_progress: false,
            location_in_progress: false,
            is_predicting_pass: false,
            error_msg: None,
            location_error_msg: None,
            sort_alpha: true,
            focus_filter: false,
            cached_trail: None,
            cached_swath: None,
        };

        // ------ Restore persisted settings ------
        if let Some(storage) = cc.storage {
            let s = AppSettings::load(storage);

            if let Some(style) = &s.map_style {
                app.map_style = match style.as_str() {
                    "OpenStreetMap" => MapStyle::OpenStreetMap,
                    _ => MapStyle::CartoDark,
                };
            }
            if let Some(v) = s.show_orbital_trail {
                app.show_orbital_trail = v;
            }
            if let Some(v) = s.swath_color {
                app.swath_color = v;
            }
            if let Some(v) = s.swath_opacity {
                app.swath_opacity = v;
            }
            if let Some(v) = s.pass_threshold_km {
                app.pass_threshold_km = v;
            }
            if let Some(v) = s.satellite_category {
                app.satellite_category = v;
            }
            if let Some(q) = s.location_query {
                app.location_query = q;
            }

            if let (Some(name), Some(lat), Some(lon), Some(alt)) = (
                s.observer_name,
                s.observer_lat,
                s.observer_lon,
                s.observer_alt,
            ) {
                app.observer = Some(crate::location::Location {
                    name,
                    lat_deg: lat,
                    lon_deg: lon,
                    alt_m: alt,
                });
            }
        }

        // Kick off a background refresh of TLEs
        app.fetch_in_progress = true;
        let tx = app.tx.clone();
        let category = app.satellite_category;
        app.rt.spawn(async move {
            let res = fetch_active_satellites(category)
                .await
                .map_err(|e| e.to_string());
            let _ = tx.send(AppMessage::SatellitesLoaded(res)).await;
        });

        app
    }
    /// Update the currently focused satellite and trigger an asynchronous `trigger_pass_prediction()` calculation.
    pub fn set_selected_satellite(&mut self, name: Option<String>) {
        if self.selected_satellite != name {
            self.cached_trail = None; // Invalidate cache on change
            self.cached_swath = None;
        }
        self.selected_satellite = name;
        self.trigger_pass_prediction();
    }

    /// Sorts and filters the global dataset dictionary into a vector of keys based on the `search_query` text.
    pub fn update_filtered_satellites(&mut self) {
        let query = self.search_query.to_lowercase();
        let mut keys: Vec<String> = self
            .satellites
            .keys()
            .filter(|k| k.to_lowercase().contains(&query))
            .cloned()
            .collect();

        if self.sort_alpha {
            keys.sort();
        } else {
            // Sort by altitude (descending or ascending)
            // It's better to sort by altitude ascending (lowest first)
            keys.sort_by(|a, b| {
                let alt_a = self
                    .satellites
                    .get(a)
                    .map(|s| s.cached_altitude)
                    .unwrap_or(0.0);
                let alt_b = self
                    .satellites
                    .get(b)
                    .map(|s| s.cached_altitude)
                    .unwrap_or(0.0);

                alt_a
                    .partial_cmp(&alt_b)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
        }

        self.filtered_satellites = keys;
    }

    /// Spawns a background `tokio` thread to predict the next overhead pass using `location::predict_next_pass`.
    pub fn trigger_pass_prediction(&mut self) {
        if self.observer.is_none() || self.selected_satellite.is_none() {
            self.last_predicted_passes.clear();
            return;
        }
        self.is_predicting_pass = true;
        let tx = self.tx.clone();
        let sat = self
            .satellites
            .get(self.selected_satellite.as_ref().unwrap())
            .cloned();
        let obs = self.observer.clone().unwrap();
        let threshold = self.pass_threshold_km;

        self.rt.spawn(async move {
            if let Some(s) = sat {
                let pass = crate::location::predict_next_pass(s, obs, threshold).await;
                let _ = tx.send(AppMessage::PassPredicted(pass)).await;
            }
        });
    }
}

impl eframe::App for OrbitSenseApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // --- Global Keyboard Shortcuts ---
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.selected_satellite = None;
        }
        if ctx.input(|i| i.key_pressed(egui::Key::R)) && !self.fetch_in_progress {
            self.fetch_in_progress = true;
            let tx = self.tx.clone();
            let category = self.satellite_category;

            self.rt.spawn(async move {
                let res = crate::satellites::fetch_active_satellites(category)
                    .await
                    .map_err(|e| e.to_string());
                let _ = tx.send(AppMessage::SatellitesLoaded(res)).await;
            });
        }
        if ctx.input(|i| i.modifiers.ctrl && i.key_pressed(egui::Key::F)) {
            self.focus_filter = true;
        }

        // Process messages from async tasks
        while let Ok(msg) = self.rx.try_recv() {
            match msg {
                AppMessage::SatellitesLoaded(Ok(sats)) => {
                    self.satellites = sats;
                    self.fetch_in_progress = false;
                    self.error_msg = None;
                    self.last_updated = Some(chrono::Local::now());
                    self.update_filtered_satellites();
                }
                AppMessage::SatellitesLoaded(Err(e)) => {
                    self.fetch_in_progress = false;
                    self.error_msg = Some(e);
                }
                AppMessage::LocationGeocoded(Ok(loc)) => {
                    self.observer = Some(loc);
                    self.location_in_progress = false;
                    self.location_error_msg = None;
                    self.trigger_pass_prediction();
                }
                AppMessage::LocationGeocoded(Err(e)) => {
                    self.location_in_progress = false;
                    self.location_error_msg = Some(e);
                }
                AppMessage::PassPredicted(pass) => {
                    self.last_predicted_passes = pass;
                    self.is_predicting_pass = false;
                }
            }
        }

        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            egui::MenuBar::new().ui(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("Preferences").clicked() {
                        self.preferences_open = !self.preferences_open;
                        ui.close();
                    }
                    if ui.button("Quit").clicked() {
                        std::process::exit(0);
                    }
                });
                ui.menu_button("Help", |ui| {
                    if ui.button("About").clicked() {
                        self.show_about = true;
                        ui.close();
                    }
                });
            });
        });

        egui::SidePanel::left("sidebar")
            .default_width(240.0)
            .min_width(180.0)
            .resizable(true)
            .show(ctx, |ui| {
                crate::ui::render_sidebar(self, ui);
            });

        egui::TopBottomPanel::bottom("status_bar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(
                        chrono::Utc::now()
                            .format("%Y-%m-%d %H:%M:%S UTC")
                            .to_string(),
                    )
                    .monospace()
                    .color(egui::Color32::from_rgb(180, 180, 180)),
                );
                ui.separator();

                let sat_count = self.satellites.len();
                ui.label(format!(
                    "{} Satellites loaded ({})",
                    sat_count,
                    self.satellite_category.name()
                ));

                if let Some(last) = &self.last_updated {
                    ui.separator();
                    let age_mins = (chrono::Local::now() - *last).num_minutes();
                    let color = if age_mins < 120 {
                        egui::Color32::from_rgb(100, 255, 100) // Green = fresh (< 2hr)
                    } else if age_mins < 1440 {
                        egui::Color32::from_rgb(255, 200, 50) // Orange = stale (< 24hr)
                    } else {
                        egui::Color32::from_rgb(255, 100, 100) // Red = old
                    };
                    ui.colored_label(color, format!("TLEs updated {}m ago", age_mins));
                }

                if let Some(obs) = &self.observer {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(format!("📍 {}", obs.name));
                    });
                }
            });
        });

        egui::CentralPanel::default()
            .frame(egui::Frame::central_panel(&ctx.style()).inner_margin(0.0))
            .show(ctx, |ui| {
                crate::ui::map::render_map(self, ui);
            });

        crate::ui::render_map_controls(self, ctx);
        crate::ui::render_satellite_info(self, ctx);
        crate::ui::render_preferences_window(self, ctx);
        crate::ui::about::render_about_window(self, ctx);
    }

    /// eframe calls this periodically to auto-save application state to disk.
    /// Storage location is OS-determined by eframe:
    ///   Linux   → ~/.local/share/<app_name>/
    ///   macOS   → ~/Library/Application Support/<app_name>/
    ///   Windows → %APPDATA%\<app_name>\
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        let settings = AppSettings {
            map_style: Some(match self.map_style {
                MapStyle::OpenStreetMap => "OpenStreetMap".to_string(),
                MapStyle::CartoDark => "CartoDark".to_string(),
            }),
            show_orbital_trail: Some(self.show_orbital_trail),
            swath_color: Some(self.swath_color),
            swath_opacity: Some(self.swath_opacity),
            pass_threshold_km: Some(self.pass_threshold_km),
            satellite_category: Some(self.satellite_category),
            location_query: Some(self.location_query.clone()),
            observer_name: self.observer.as_ref().map(|o| o.name.clone()),
            observer_lat: self.observer.as_ref().map(|o| o.lat_deg),
            observer_lon: self.observer.as_ref().map(|o| o.lon_deg),
            observer_alt: self.observer.as_ref().map(|o| o.alt_m),
        };
        settings.save(storage);
    }
}
