//! Renders the left-hand navigation sidebar for selecting satellites and searching observer coordinates.

use crate::app::{AppMessage, OrbitSenseApp};
use crate::location::Location;
use crate::satellites::fetch_active_satellites;
use egui::{self, Color32, RichText};

/// Renders the sidebar containing the observer location input and the list of active satellites.
pub fn render_sidebar(app: &mut OrbitSenseApp, ui: &mut egui::Ui) {
    // ── Observer Location ───────────────────────────────────────────────────
    ui.add_space(4.0);
    ui.label(RichText::new("📍 Observer Location").strong());
    ui.add_space(2.0);

    let location_hint = if app.observer.is_some() {
        "Change location..."
    } else {
        "City, State or Country"
    };

    let search_box = egui::TextEdit::singleline(&mut app.location_query)
        .hint_text(location_hint)
        .desired_width(f32::INFINITY);
    ui.add(search_box);

    ui.add_space(2.0);

    // Show observer coordinates in a compact styled label
    if let Some(obs) = &app.observer {
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(format!("🌐 {}", obs.name))
                    .small()
                    .color(Color32::from_rgb(120, 200, 140)),
            );
        });
        ui.label(
            RichText::new(format!("  {:.3}°N  {:.3}°E", obs.lat_deg, obs.lon_deg))
                .small()
                .monospace()
                .color(Color32::GRAY),
        );
        ui.add_space(2.0);
    }

    // Full-width search button or spinner
    if app.location_in_progress {
        ui.horizontal(|ui| {
            ui.spinner();
            ui.label(RichText::new("Geocoding…").small().italics());
        });
    } else {
        let btn_label = if app.observer.is_some() {
            "🔄 Update Location"
        } else {
            "🔍 Search Location"
        };
        if ui
            .add_sized([f32::INFINITY, 22.0], egui::Button::new(btn_label))
            .clicked()
            && !app.location_query.is_empty()
        {
            app.location_in_progress = true;
            let query = app.location_query.clone();
            let tx = app.tx.clone();
            app.rt.spawn(async move {
                let loc_res = Location::from_query(&query).await;
                let _ = tx.send(AppMessage::LocationGeocoded(loc_res)).await;
            });
        }
    }

    ui.add_space(6.0);
    ui.separator();
    ui.add_space(4.0);

    // ── Satellite Category ──────────────────────────────────────────────────
    let sat_count = app.filtered_satellites.len();
    let total_count = app.satellites.len();

    ui.horizontal(|ui| {
        ui.label(RichText::new("🛰 Satellites").strong());
        if total_count > 0 {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(
                    RichText::new(format!("{}/{}", sat_count, total_count))
                        .small()
                        .color(Color32::GRAY),
                );
            });
        }
    });
    ui.add_space(2.0);

    // Category dropdown
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
                        RichText::new(group)
                            .small()
                            .color(Color32::from_rgb(150, 180, 220)),
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

    // Refresh button — always render to keep egui ID stable
    let refresh_clicked = ui
        .add_sized([f32::INFINITY, 22.0], egui::Button::new("↻  Refresh TLEs"))
        .clicked();
    if (category_changed || refresh_clicked) && !app.fetch_in_progress {
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

    // Status row
    if app.fetch_in_progress {
        ui.horizontal(|ui| {
            ui.spinner();
            ui.label(RichText::new("Downloading TLEs…").small().italics());
        });
    } else if let Some(last) = &app.last_updated {
        ui.label(
            RichText::new(format!("Updated {}", last.format("%H:%M:%S")))
                .small()
                .color(Color32::GRAY),
        );
    }

    if let Some(err) = &app.error_msg.clone() {
        ui.colored_label(Color32::from_rgb(255, 80, 80), format!("⚠ {}", err));
    }

    ui.add_space(4.0);

    // ── Search / Filter ─────────────────────────────────────────────────────
    let filter_box = egui::TextEdit::singleline(&mut app.search_query)
        .hint_text("Filter satellites…")
        .desired_width(f32::INFINITY);
    if ui.add(filter_box).changed() {
        app.update_filtered_satellites();
    }

    ui.add_space(2.0);

    // ── Satellite List ──────────────────────────────────────────────────────
    if app.satellites.is_empty() && !app.fetch_in_progress {
        ui.add_space(12.0);
        ui.vertical_centered(|ui| {
            ui.label(RichText::new("No data").color(Color32::GRAY));
            ui.label(
                RichText::new("Click Refresh TLEs")
                    .small()
                    .color(Color32::GRAY),
            );
        });
        return;
    }

    if app.filtered_satellites.is_empty() && !app.search_query.is_empty() {
        ui.label(RichText::new("No matches").small().color(Color32::GRAY));
    }

    let filtered = app.filtered_satellites.clone();
    egui::ScrollArea::vertical()
        .auto_shrink([false; 2])
        .show(ui, |ui| {
            for name in &filtered {
                let selected = app.selected_satellite.as_ref() == Some(name);
                if ui
                    .selectable_label(selected, RichText::new(name).monospace().small())
                    .clicked()
                {
                    app.set_selected_satellite(Some(name.clone()));
                }
            }
        });
}
