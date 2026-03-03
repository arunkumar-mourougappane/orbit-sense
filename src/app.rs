//! Main application state and startup logic for Orbit Sense.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tokio::runtime::Handle;
use tokio::sync::mpsc::{self, Receiver, Sender};

use crate::location::{Location, Observation};
use crate::satellites::{SpaceObject, fetch_active_satellites};

/// Messages passed from background asynchronous tasks back to the UI thread.
use walkers::{HttpOptions, HttpTiles, MapMemory, Position};
/// Messages passed from background asynchronous tasks back to the UI thread.
pub enum AppMessage {
    /// Received a payload containing the successfully parsed Celestrak dataset, or an error.
    SatellitesLoaded(Result<HashMap<String, SpaceObject>, String>),
    /// Received a parsed lat/lon coordinate from Nominatim, or an error.
    LocationGeocoded(Result<Location, String>),
    /// Emitted after `predict_next_pass` completes evaluating the upcoming 48 hours.
    PassPredicted(Vec<(chrono::DateTime<chrono::Utc>, f64)>),
}

/// Supported map tile providers.
#[derive(PartialEq, Clone, Serialize, Deserialize)]
pub enum MapStyle {
    /// Standard OpenStreetMap styled light theme.
    OpenStreetMap,
    /// Dark-themed CartoDB base map for high contrast satellite tracking.
    CartoDark,
}

// Removed duplicate MapStyle

/// Serializable snapshot of user preferences saved across sessions.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct AppSettings {
    /// String representation of the active `MapStyle`.
    pub map_style: Option<String>,
    /// Whether the orbital trail should be drawn for the selected satellite.
    pub show_orbital_trail: Option<bool>,
    /// RGB color payload for the footprint swath polyon [0.0..1.0].
    pub swath_color: Option<[f32; 3]>,
    /// Alpha opacity for the footprint swath polygon [0.0..1.0].
    pub swath_opacity: Option<f32>,
    /// Distance in kilometers used to definitively predict if a pass is "overhead".
    pub pass_threshold_km: Option<f64>,
    /// The currently selected subset of the Celestrak catalog.
    pub satellite_category: Option<crate::satellites::SatelliteCategory>,
    /// Raw string query used to look up the observer location.
    pub observer_name: Option<String>,
    /// Geocoded latitude of observer.
    pub observer_lat: Option<f64>,
    /// Geocoded longitude of observer.
    pub observer_lon: Option<f64>,
    /// Altitude of the observer (currently unused, defaults to 0).
    pub observer_alt: Option<f64>,
    /// Current search box query.
    pub location_query: Option<String>,
    /// Minimum altitude filter in km.
    pub filter_min_alt: Option<f64>,
    /// Maximum altitude filter in km.
    pub filter_max_alt: Option<f64>,
    /// Minimum inclination filter in degrees.
    pub filter_min_inc: Option<f64>,
    /// Maximum inclination filter in degrees.
    pub filter_max_inc: Option<f64>,
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
/// See: <https://github.com/CartoDB/basemap-styles>
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
    /// Active dictionary of all successfully parsed and active satellite objects.
    pub satellites: HashMap<String, SpaceObject>,
    /// HashSet of Keys (names) of the satellites currently highlighted globally on the map.
    pub selected_satellites: std::collections::HashSet<String>,
    /// Live search query filtering the satellite list.
    pub search_query: String,
    /// Keys representing the subset of satellites that match the `search_query` string.
    pub filtered_satellites: Vec<String>,
    /// Active fetch category targeting a Celestrak group (e.g. Visual, Starlink).
    pub satellite_category: crate::satellites::SatelliteCategory,
    /// Timestamp of the last successful TLE download.
    pub last_updated: Option<chrono::DateTime<chrono::Local>>,

    // Observer state
    /// Geocoded location payload acting as the viewpoint of the user.
    pub observer: Option<Location>,
    /// Live query used to send searches to the Nominatim geocoder.
    pub location_query: String,
    /// Array of times and distances when the `selected_satellite` will cross the `observer` location.
    pub last_predicted_passes: Vec<(chrono::DateTime<chrono::Utc>, f64)>,

    // UI state
    /// Toggles the right-side details panel containing SGP4 element readouts.
    pub show_satellite_info: bool,
    /// Toggles the floating preferences popup modal.
    pub preferences_open: bool,
    /// Toggles the "About" information window.
    pub show_about: bool,
    /// Visibility state of the 90-minute projected trail.
    pub show_orbital_trail: bool,
    /// When true, intercepts map-panning to force the `map_memory` center onto the selected satellite.
    pub camera_locked: bool,
    /// Upper limit distance in kilometers triggering a successful overhead pass prediction.
    pub pass_threshold_km: f64,
    /// GUI preference for the `[r,g,b]` swath color, mapped 0.0 to 1.0.
    pub swath_color: [f32; 3],
    /// GUI preference for the swath alpha opacity, mapped 0.0 to 1.0.
    pub swath_opacity: f32,
    /// Pointer to the active `walkers` tile provider logic.
    pub map_style: MapStyle,
    /// `walkers` internal coordinate memory storing zoom and panning layout.
    pub map_memory: MapMemory,
    /// Asynchronous `walkers` HTTP worker fetching OpenStreetMap slippy tiles.
    pub tiles_osm: HttpTiles,
    /// Asynchronous `walkers` HTTP worker fetching CartoDB slippy tiles.
    pub tiles_carto: HttpTiles,
    /// Handle to the concurrent tokio asynchronous executor.
    pub rt: Handle,

    // Async communications
    /// Channel for the UI thread to send tasks to background threads.
    pub tx: Sender<AppMessage>,
    /// Receiver checking for completed background data fetches like TLE processing.
    pub rx: Receiver<AppMessage>,

    /// Mutex bool restricting simultaneous HTTP requests for TLE lists.
    pub fetch_in_progress: bool,
    /// Mutex bool restricting simultaneous HTTP requests for geocoding.
    pub location_in_progress: bool,
    /// Mutex bool denoting an active brute-force overhead prediction crunch.
    pub is_predicting_pass: bool,
    /// Error from the last satellite TLE download attempt.
    pub error_msg: Option<String>,
    /// Error from the last Observer Location geocoding attempt.
    pub location_error_msg: Option<String>,
    /// If true, satellite list is sorted alphabetically (default). If false, sorted by altitude descending.
    pub sort_alpha: bool,
    /// Set to true for one frame to move keyboard focus to the filter box.
    pub focus_filter: bool,
    /// Minimum altitude in km for the sidebar filter.
    pub filter_min_alt: f64,
    /// Maximum altitude in km for the sidebar filter.
    pub filter_max_alt: f64,
    /// Minimum inclination in degrees for the sidebar filter.
    pub filter_min_inc: f64,
    /// Maximum inclination in degrees for the sidebar filter.
    pub filter_max_inc: f64,
    /// Simulated time offset from real UTC now, in seconds.
    pub time_offset_seconds: f64,
    /// Speed multiplier for orbital playback. 1.0 = real-time, 0.0 = paused.
    pub playback_speed_multiplier: f64,
    /// Caches the heavy SGP4 math for the selected satellites' orbital trails. (time, trail)
    pub cached_trails: HashMap<String, (chrono::DateTime<chrono::Utc>, Vec<walkers::Position>)>,
    /// Cache the swath polygon points to avoid heavy trig per-frame. (lat, lon, points)
    pub cached_swaths: HashMap<String, (f64, f64, Vec<walkers::Position>)>,
    /// Per-frame calculation of the selected satellites' exact positions.
    pub current_observations: HashMap<String, Observation>,
    /// Timestamp of the last global position recalculation.
    pub last_position_update: chrono::DateTime<chrono::Utc>,
}

impl OrbitSenseApp {
    /// Constructs the initial state of the application.
    /// Injects `reqwest` headers for Celestrak mapping and pulls initial cache.
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let (tx, rx) = mpsc::channel(100);

        // Setup walkers tiles manager
        let cache_dir = std::path::PathBuf::from(std::env::var("HOME").unwrap_or_default())
            .join(".local/share/orbit-sense/tiles");

        let options = HttpOptions {
            cache: Some(cache_dir.clone()),
            user_agent: Some(reqwest::header::HeaderValue::from_static(
                "orbit-sense/0.1 (amouroug@gemini.local)",
            )),
            ..Default::default()
        };

        let options2 = HttpOptions {
            cache: Some(cache_dir),
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
            selected_satellites: std::collections::HashSet::new(),
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
            filter_min_alt: 0.0,
            filter_max_alt: 200000.0,
            filter_min_inc: 0.0,
            filter_max_inc: 180.0,
            time_offset_seconds: 0.0,
            playback_speed_multiplier: 1.0,
            cached_trails: HashMap::new(),
            cached_swaths: HashMap::new(),
            current_observations: HashMap::new(),
            last_position_update: chrono::Utc::now(),
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
            if let Some(v) = s.filter_min_alt {
                app.filter_min_alt = v;
            }
            if let Some(v) = s.filter_max_alt {
                app.filter_max_alt = v;
            }
            if let Some(v) = s.filter_min_inc {
                app.filter_min_inc = v;
            }
            if let Some(v) = s.filter_max_inc {
                app.filter_max_inc = v;
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

    /// Returns the currently simulated time.
    pub fn current_time(&self) -> chrono::DateTime<chrono::Utc> {
        chrono::Utc::now() + chrono::Duration::seconds(self.time_offset_seconds as i64)
    }

    /// Toggle a satellite selection.
    pub fn toggle_selected_satellite(&mut self, name: String) {
        if self.selected_satellites.contains(&name) {
            self.selected_satellites.remove(&name);
            self.cached_trails.remove(&name);
            self.cached_swaths.remove(&name);
            self.current_observations.remove(&name);
        } else {
            self.selected_satellites.insert(name);
        }
        self.trigger_pass_prediction();
    }

    /// Sorts and filters the global dataset dictionary into a vector of keys based on the `search_query` text.
    pub fn update_filtered_satellites(&mut self) {
        let query = self.search_query.to_lowercase();
        let mut keys: Vec<String> = self
            .satellites
            .iter()
            .filter(|(k, sat)| {
                k.to_lowercase().contains(&query)
                    && sat.cached_altitude >= self.filter_min_alt
                    && sat.cached_altitude <= self.filter_max_alt
                    && sat.elements.inclination >= self.filter_min_inc
                    && sat.elements.inclination <= self.filter_max_inc
            })
            .map(|(k, _)| k.clone())
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
        if self.observer.is_none() || self.selected_satellites.is_empty() {
            self.last_predicted_passes.clear();
            return;
        }
        self.is_predicting_pass = true;
        let tx = self.tx.clone();

        let first_selected = self.selected_satellites.iter().next().unwrap().clone();
        let sat = self.satellites.get(&first_selected).cloned();
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
        let dt = ctx.input(|i| i.stable_dt) as f64;
        self.time_offset_seconds += (self.playback_speed_multiplier - 1.0) * dt;

        let now_utc = self.current_time();
        if (now_utc - self.last_position_update).num_seconds() >= 1 {
            use rayon::prelude::*;
            let loc = crate::location::Location {
                name: String::new(),
                lat_deg: 0.0,
                lon_deg: 0.0,
                alt_m: 0.0,
            };
            self.satellites.par_iter_mut().for_each(|(_, sat)| {
                if let Some(obs) = crate::location::calculate_observation(
                    &sat.elements,
                    &sat.constants,
                    &loc,
                    now_utc,
                ) {
                    sat.cached_position = Some((obs.sub_lat_deg, obs.sub_lon_deg));
                }
            });
            self.last_position_update = now_utc;
        }

        // --- Global Keyboard Shortcuts ---
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.selected_satellites.clear();
        }

        // Calculate single Observation payload per-frame instead of letting UI functions duplicate it
        self.current_observations.clear();
        for name in &self.selected_satellites {
            if let Some(sat) = self.satellites.get(name)
                && let Some(obs) = crate::location::calculate_observation(
                    &sat.elements,
                    &sat.constants,
                    &crate::location::Location {
                        name: String::new(),
                        lat_deg: 0.0,
                        lon_deg: 0.0,
                        alt_m: 0.0,
                    },
                    now_utc,
                )
            {
                self.current_observations.insert(name.clone(), obs);
            }
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
                        self.current_time()
                            .format("%Y-%m-%d %H:%M:%S Sim")
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
        crate::ui::overlay::render_time_controls(self, ctx);
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
            filter_min_alt: Some(self.filter_min_alt),
            filter_max_alt: Some(self.filter_max_alt),
            filter_min_inc: Some(self.filter_min_inc),
            filter_max_inc: Some(self.filter_max_inc),
        };
        settings.save(storage);
    }
}
