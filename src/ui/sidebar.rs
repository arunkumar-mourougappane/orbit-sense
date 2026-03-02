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

            app.rt.spawn(async move {
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
            ui.label(format!(
                "📍 {} — {:.2}°N {:.2}°E",
                obs.name, obs.lat_deg, obs.lon_deg
            ));
        }
    });

    ui.separator();

    ui.group(|ui| {
        // Satellite count badge next to heading
        let total = app.satellites.len();
        let shown = app.filtered_satellites.len();
        if total > 0 {
            ui.horizontal(|ui| {
                ui.label("Satellites");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        egui::RichText::new(format!("{shown}/{total}"))
                            .small()
                            .color(egui::Color32::GRAY),
                    );
                });
            });
        } else {
            ui.label("Satellites");
        }

        // Grouped satellite category dropdown
        let mut category_changed = false;
        egui::ComboBox::from_id_salt("satellite_category")
            .selected_text(app.satellite_category.name())
            .show_ui(ui, |ui| {
                let mut current_group = "";
                for &cat in crate::satellites::SatelliteCategory::all() {
                    let group = cat.group_label();
                    if group != current_group {
                        if !current_group.is_empty() {
                            ui.separator();
                        }
                        ui.label(
                            egui::RichText::new(group)
                                .small()
                                .color(egui::Color32::from_rgb(150, 180, 220)),
                        );
                        current_group = group;
                    }
                    if ui
                        .selectable_value(&mut app.satellite_category, cat, cat.name())
                        .clicked()
                    {
                        category_changed = true;
                    }
                }
            });

        // Always render Refresh button (avoids egui ID instability with short-circuit evaluation)
        let refresh_clicked = ui.button("Refresh TLEs").clicked();
        if (refresh_clicked || category_changed) && !app.fetch_in_progress {
            if category_changed {
                app.satellites.clear();
                app.filtered_satellites.clear();
                app.selected_satellite = None;
            }
            app.fetch_in_progress = true;
            let tx = app.tx.clone();
            let category = app.satellite_category;

            app.rt.spawn(async move {
                let res = fetch_active_satellites(category)
                    .await
                    .map_err(|e| e.to_string());
                let _ = tx.send(AppMessage::SatellitesLoaded(res)).await;
            });
        }

        if let Some(last) = &app.last_updated {
            ui.label(
                egui::RichText::new(format!("Updated {}", last.format("%H:%M:%S")))
                    .small()
                    .color(egui::Color32::GRAY),
            );
        }

        if app.fetch_in_progress {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(egui::RichText::new("Downloading…").small().italics());
            });
        }

        if let Some(err) = &app.error_msg {
            ui.colored_label(egui::Color32::from_rgb(255, 80, 80), format!("⚠ {err}"));
        }

        // Filter text box
        if ui
            .text_edit_singleline(&mut app.search_query)
            .on_hover_text("Filter by name (e.g. 'ISS')")
            .changed()
        {
            app.update_filtered_satellites();
        }

        if app.filtered_satellites.is_empty() && !app.search_query.is_empty() {
            ui.label(
                egui::RichText::new("No matches")
                    .small()
                    .color(egui::Color32::GRAY),
            );
        }

        ui.separator();

        let filtered = app.filtered_satellites.clone();
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                for name in &filtered {
                    let selected = app.selected_satellite.as_ref() == Some(name);
                    let row_width = ui.available_width();
                    if ui
                        .add_sized([row_width, 18.0], egui::Button::selectable(selected, name))
                        .clicked()
                    {
                        app.set_selected_satellite(Some(name.clone()));
                    }
                }
            });
    });
}
