//! Main application state and startup logic for Orbit Sense.

use egui;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::runtime::Runtime;
use tokio::sync::mpsc::{self, Receiver, Sender};

use crate::location::Location;
use crate::satellites::{SpaceObject, fetch_active_satellites};

/// Messages passed from background asynchronous tasks back to the UI thread.
pub enum AppMessage {
    /// Received a payload containing the successfully parsed Celestrak dataset, or an error.
    SatellitesLoaded(Result<HashMap<String, SpaceObject>, String>),
    /// Received a parsed lat/lon coordinate from Nominatim, or an error.
    LocationGeocoded(Result<Location, String>),
    /// Emitted after `predict_next_pass` completes evaluating the upcoming 48 hours.
    PassPredicted(Vec<(chrono::DateTime<chrono::Utc>, f64)>),
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
    pub show_orbital_trail: bool,
    pub camera_locked: bool,
    pub pass_threshold_km: f64,
    pub rt: Arc<Runtime>,

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
    pub fn new(rt: Arc<Runtime>) -> Self {
        let (tx, rx) = mpsc::channel(100);

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
            show_orbital_trail: true,
            camera_locked: false,
            pass_threshold_km: crate::constants::DEFAULT_PASS_THRESHOLD_KM,
            rt,
            tx,
            rx,
            fetch_in_progress: false,
            location_in_progress: false,
            is_predicting_pass: false,
            error_msg: None,
        };

        // Location recovery bypassed for now since we removed eframe::Storage.
        // We'll implement Macroquad-native persistence or a simple JSON file in Phase 3.

        // If no satellites were cached yet, we show the loading spinner immediately.
        if app.satellites.is_empty() {
            app.fetch_in_progress = true;
        }

        // Silent (or otherwise) background refresh of TLEs so they are current.
        let tx = app.tx.clone();
        let category = app.satellite_category;
        tokio::spawn(async move {
            let res = fetch_active_satellites(category)
                .await
                .map_err(|e| e.to_string());
            let _ = tx.send(AppMessage::SatellitesLoaded(res)).await;
        });

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

    pub fn update(&mut self, ctx: &egui::Context) {
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
                    self.error_msg = None;
                    self.trigger_pass_prediction();
                }
                AppMessage::LocationGeocoded(Err(e)) => {
                    self.location_in_progress = false;
                    self.error_msg = Some(e);
                }
                AppMessage::PassPredicted(pass) => {
                    self.last_predicted_passes = pass;
                    self.is_predicting_pass = false;
                }
            }
        }

        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("Preferences").clicked() {
                        self.preferences_open = !self.preferences_open;
                    }
                    if ui.button("Quit").clicked() {
                        std::process::exit(0);
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
                // UI over our 3D globe later
            });

        crate::ui::render_map_controls(self, ctx);
        crate::ui::render_satellite_info(self, ctx);
        crate::ui::render_preferences_window(self, ctx);

        crate::ui::render_satellite_info(self, ctx);
    }
}
