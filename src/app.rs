//! Main application state and startup logic for Orbit Sense.

use eframe::egui;
use std::collections::HashMap;
use tokio::sync::mpsc::{self, Receiver, Sender};
use walkers::{HttpOptions, HttpTiles, MapMemory, Position};

use crate::location::Location;
use crate::satellites::{SpaceObject, fetch_active_satellites};

/// Messages passed from background asynchronous tasks back to the UI thread.
pub enum AppMessage {
    /// Received a payload containing the successfully parsed Celestrak dataset, or an error.
    SatellitesLoaded(Result<HashMap<String, SpaceObject>, String>),
    /// Received a parsed lat/lon coordinate from Nominatim, or an error.
    LocationGeocoded(Result<Location, String>),
    /// Emitted after `predict_next_pass` completes evaluating the upcoming 24 hours.
    PassPredicted(Option<(chrono::DateTime<chrono::Utc>, f64)>),
}

#[derive(PartialEq, Clone)]
pub enum MapStyle {
    OpenStreetMap,
    CartoDark,
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
    pub last_updated: Option<chrono::DateTime<chrono::Local>>,

    // Observer state
    pub observer: Option<Location>,
    pub location_query: String,
    pub last_predicted_pass: Option<(chrono::DateTime<chrono::Utc>, f64)>,

    // UI state
    pub show_satellite_info: bool,
    pub preferences_open: bool,
    pub show_orbital_trail: bool,
    pub camera_locked: bool,
    pub pass_threshold_km: f64,
    pub map_style: MapStyle,
    pub map_memory: MapMemory,
    pub tiles_osm: HttpTiles,
    pub tiles_carto: HttpTiles,

    // Async communications
    pub tx: Sender<AppMessage>,
    pub rx: Receiver<AppMessage>,

    pub fetch_in_progress: bool,
    pub location_in_progress: bool,
    pub is_predicting_pass: bool,
    pub error_msg: Option<String>,
}

impl OrbitSenseApp {
    /// Constructs the initial state of the application.
    /// Injects `reqwest` headers for Celestrak mapping and pulls initial cache.
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // Customize egui here with cc.egui_ctx.set_visuals and set_fonts
        let mut style = (*cc.egui_ctx.style()).clone();
        style.visuals = egui::Visuals::dark();
        cc.egui_ctx.set_style(style);

        let (tx, rx) = mpsc::channel(100);
        let ctx = cc.egui_ctx.clone();

        // Setup walkers tiles manager
        let options = HttpOptions {
            // Note: reqwest::header::HeaderValue should work if walkers re-exports it, or if reqwest is available directly.
            // Since reqwest is in Cargo.toml, we can just use reqwest::header::HeaderValue::from_static
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

        let tiles_osm =
            HttpTiles::with_options(walkers::sources::OpenStreetMap, options, ctx.clone());
        let tiles_carto = HttpTiles::with_options(CartoDark, options2, ctx.clone());

        let mut app = Self {
            satellites: HashMap::new(),
            selected_satellite: None,
            search_query: String::new(),
            filtered_satellites: Vec::new(),
            last_updated: None,
            observer: None,
            location_query: String::new(),
            last_predicted_pass: None,
            show_satellite_info: false,
            preferences_open: false,
            show_orbital_trail: true,
            camera_locked: false,
            pass_threshold_km: crate::constants::DEFAULT_PASS_THRESHOLD_KM,
            map_style: MapStyle::CartoDark, // Default to sleek Dark Mode
            map_memory: MapMemory::default(),
            tiles_osm,
            tiles_carto,
            tx,
            rx,
            fetch_in_progress: false,
            location_in_progress: false,
            is_predicting_pass: false,
            error_msg: None,
        };

        // Restore location from storage
        let mut centered = false;
        if let Some(storage) = cc.storage {
            if let Some(loc_json) = storage.get_string("last_observer") {
                if let Ok(loc) = serde_json::from_str::<Location>(&loc_json) {
                    app.map_memory
                        .center_at(Position::new(loc.lon_deg, loc.lat_deg));
                    app.observer = Some(loc);
                    centered = true;
                }
            }
            if let Some(query) = storage.get_string("last_location_query") {
                app.location_query = query;
            }
            if let Some(sats_json) = storage.get_string("satellites_cache") {
                if let Ok(sats) = serde_json::from_str::<HashMap<String, SpaceObject>>(&sats_json) {
                    app.satellites = sats;
                }
            }
        }

        // If no satellites were cached yet, we show the loading spinner immediately.
        if app.satellites.is_empty() {
            app.fetch_in_progress = true;
        }

        // Silent (or otherwise) background refresh of TLEs so they are current.
        let tx = app.tx.clone();
        tokio::spawn(async move {
            let res = fetch_active_satellites().await.map_err(|e| e.to_string());
            let _ = tx.send(AppMessage::SatellitesLoaded(res)).await;
        });

        if !centered {
            // Initial map view: Center roughly over the Atlantic
            app.map_memory.center_at(Position::new(0.0, 20.0));
        }

        app
    }
    /// Update the currently focused satellite and trigger an asynchronous `trigger_pass_prediction()` calculation.
    pub fn set_selected_satellite(&mut self, name: Option<String>) {
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
        keys.sort();
        self.filtered_satellites = keys;
    }

    /// Spawns a background `tokio` thread to predict the next overhead pass using `location::predict_next_pass`.
    pub fn trigger_pass_prediction(&mut self) {
        if self.observer.is_none() || self.selected_satellite.is_none() {
            self.last_predicted_pass = None;
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

        tokio::spawn(async move {
            if let Some(s) = sat {
                let pass = crate::location::predict_next_pass(s, obs, threshold).await;
                let _ = tx.send(AppMessage::PassPredicted(pass)).await;
            }
        });
    }
}

impl eframe::App for OrbitSenseApp {
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        if let Some(obs) = &self.observer {
            if let Ok(loc_json) = serde_json::to_string(obs) {
                storage.set_string("last_observer", loc_json);
            }
        }
        storage.set_string("last_location_query", self.location_query.clone());

        if !self.satellites.is_empty() {
            if let Ok(sats_json) = serde_json::to_string(&self.satellites) {
                storage.set_string("satellites_cache", sats_json);
            }
        }
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
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
                    self.map_memory
                        .center_at(Position::new(loc.lon_deg, loc.lat_deg));
                    self.observer = Some(loc);
                    self.location_in_progress = false;
                    self.error_msg = None;
                    self.trigger_pass_prediction();
                }
                AppMessage::LocationGeocoded(Err(e)) => {
                    self.location_in_progress = false;
                    self.error_msg = Some(e);
                }
                AppMessage::PassPredicted(pass) => {
                    self.last_predicted_pass = pass;
                    self.is_predicting_pass = false;
                }
            }
        }

        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            egui::MenuBar::new().ui(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("Preferences").clicked() {
                        self.preferences_open = !self.preferences_open;
                    }
                    if ui.button("Quit").clicked() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                });
            });
        });

        egui::SidePanel::left("sidebar").show(ctx, |ui| {
            crate::ui::render_sidebar(self, ui);
        });

        egui::CentralPanel::default()
            .frame(egui::Frame::central_panel(&ctx.style()).inner_margin(0.0))
            .show(ctx, |ui| {
                crate::ui::render_map(self, ui);
            });

        crate::ui::render_map_controls(self, ctx);
        crate::ui::render_satellite_info(self, ctx);
        crate::ui::render_preferences_window(self, ctx);

        crate::ui::render_satellite_info(self, ctx);
    }
}
