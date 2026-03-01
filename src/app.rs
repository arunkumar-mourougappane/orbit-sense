use eframe::egui;
use std::collections::HashMap;
use tokio::sync::mpsc::{self, Receiver, Sender};
use walkers::{HttpOptions, HttpTiles, MapMemory, Position};

use crate::location::Location;
use crate::satellites::SpaceObject;

// Messages from async tasks back to the UI thread
pub enum AppMessage {
    SatellitesLoaded(Result<HashMap<String, SpaceObject>, String>),
    LocationGeocoded(Option<Location>),
}

pub struct OrbitSenseApp {
    // Data state
    pub satellites: HashMap<String, SpaceObject>,
    pub selected_satellite: Option<String>,
    pub search_query: String,

    // Observer state
    pub observer: Option<Location>,
    pub location_query: String,

    // UI state
    pub map_memory: MapMemory,
    pub tiles_manager: HttpTiles,

    // Async communications
    pub tx: Sender<AppMessage>,
    pub rx: Receiver<AppMessage>,

    pub fetch_in_progress: bool,
    pub location_in_progress: bool,
    pub error_msg: Option<String>,
}

impl OrbitSenseApp {
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

        let tiles_manager =
            HttpTiles::with_options(walkers::sources::OpenStreetMap, options, ctx.clone());
        let mut app = Self {
            satellites: HashMap::new(),
            selected_satellite: None,
            search_query: String::new(),
            observer: None,
            location_query: String::new(),
            map_memory: MapMemory::default(),
            tiles_manager,
            tx,
            rx,
            fetch_in_progress: false,
            location_in_progress: false,
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
        }

        if !centered {
            // Initial map view: Center roughly over the Atlantic
            app.map_memory.center_at(Position::new(0.0, 20.0));
        }

        app
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
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Process messages from async tasks
        while let Ok(msg) = self.rx.try_recv() {
            match msg {
                AppMessage::SatellitesLoaded(Ok(sats)) => {
                    self.satellites = sats;
                    self.fetch_in_progress = false;
                    self.error_msg = None;
                }
                AppMessage::SatellitesLoaded(Err(e)) => {
                    self.fetch_in_progress = false;
                    self.error_msg = Some(e);
                }
                AppMessage::LocationGeocoded(Some(loc)) => {
                    self.map_memory
                        .center_at(Position::new(loc.lon_deg, loc.lat_deg));
                    self.observer = Some(loc);
                    self.location_in_progress = false;
                    self.error_msg = None;
                }
                AppMessage::LocationGeocoded(None) => {
                    self.location_in_progress = false;
                    self.error_msg = Some("Location not found".to_string());
                }
            }
        }

        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            egui::MenuBar::new().ui(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("Preferences").clicked() {
                        // For now, this is just a placeholder
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

        egui::CentralPanel::default().show(ctx, |ui| {
            crate::ui::render_map(self, ui);
        });
    }
}
