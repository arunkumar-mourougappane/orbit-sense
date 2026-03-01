//! Renders the left-hand navigation sidebar for selecting satellites and searching observer coordinates.

use crate::app::{AppMessage, OrbitSenseApp};
use crate::location::Location;
use crate::satellites::fetch_active_satellites;
use egui;

/// Renders the sidebar containing the observer location input and the list of active satellites.
pub fn render_sidebar(app: &mut OrbitSenseApp, ui: &mut egui::Ui) {
    ui.heading("Orbit Sense");
    ui.separator();

    ui.group(|ui| {
        ui.label("Observer Location");
        ui.text_edit_singleline(&mut app.location_query)
            .on_hover_text("Enter City, State (e.g. 'Houston, TX')");

        if ui.button("Search Location").clicked() && !app.location_in_progress {
            app.location_in_progress = true;
            let query = app.location_query.clone();
            let tx = app.tx.clone();

            tokio::spawn(async move {
                let loc_res = Location::from_query(&query).await;
                let _ = tx.send(AppMessage::LocationGeocoded(loc_res)).await;
            });
        }

        if app.location_in_progress {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label("Searching...");
            });
        }

        if let Some(obs) = &app.observer {
            ui.label(format!("Lat: {:.2}, Lon: {:.2}", obs.lat_deg, obs.lon_deg));
        }
    });

    ui.separator();

    ui.group(|ui| {
        ui.label("Satellites");

        let mut category_changed = false;
        egui::ComboBox::from_id_salt("satellite_category")
            .selected_text(app.satellite_category.name())
            .show_ui(ui, |ui| {
                if ui
                    .selectable_value(
                        &mut app.satellite_category,
                        crate::satellites::SatelliteCategory::Visual,
                        "Visual (100 Brightest)",
                    )
                    .clicked()
                {
                    category_changed = true;
                }
                if ui
                    .selectable_value(
                        &mut app.satellite_category,
                        crate::satellites::SatelliteCategory::Starlink,
                        "Starlink",
                    )
                    .clicked()
                {
                    category_changed = true;
                }
                if ui
                    .selectable_value(
                        &mut app.satellite_category,
                        crate::satellites::SatelliteCategory::Weather,
                        "Weather",
                    )
                    .clicked()
                {
                    category_changed = true;
                }
                if ui
                    .selectable_value(
                        &mut app.satellite_category,
                        crate::satellites::SatelliteCategory::Gps,
                        "GPS Operational",
                    )
                    .clicked()
                {
                    category_changed = true;
                }
                if ui
                    .selectable_value(
                        &mut app.satellite_category,
                        crate::satellites::SatelliteCategory::SpaceStations,
                        "Space Stations",
                    )
                    .clicked()
                {
                    category_changed = true;
                }
            });

        if (ui.button("Refresh TLEs").clicked() || category_changed) && !app.fetch_in_progress {
            if category_changed {
                app.satellites.clear();
                app.filtered_satellites.clear();
                app.selected_satellite = None;
            }
            app.fetch_in_progress = true;
            let tx = app.tx.clone();
            let category = app.satellite_category;

            tokio::spawn(async move {
                let res = fetch_active_satellites(category)
                    .await
                    .map_err(|e| e.to_string());
                let _ = tx.send(AppMessage::SatellitesLoaded(res)).await;
            });
        }

        if let Some(last) = &app.last_updated {
            ui.label(format!("Last Updated: {}", last.format("%H:%M:%S")));
        }

        if app.fetch_in_progress {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label("Downloading...");
            });
        }

        if let Some(err) = &app.error_msg {
            ui.colored_label(egui::Color32::RED, err);
        }

        if ui
            .text_edit_singleline(&mut app.search_query)
            .on_hover_text("Filter by name (e.g. 'ISS')")
            .changed()
        {
            app.update_filtered_satellites();
        }

        ui.separator();

        let filtered = app.filtered_satellites.clone();
        egui::ScrollArea::vertical().show(ui, |ui| {
            for name in &filtered {
                let selected = app.selected_satellite.as_ref() == Some(name);
                if ui.selectable_label(selected, name).clicked() {
                    app.set_selected_satellite(Some(name.clone()));
                }
            }
        });
    });
}
