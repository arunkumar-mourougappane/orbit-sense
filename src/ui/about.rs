//! About dialog — displays application version, author, and license information.

use crate::app::OrbitSenseApp;

const APP_VERSION: &str = env!("CARGO_PKG_VERSION");
const APP_DESCRIPTION: &str = env!("CARGO_PKG_DESCRIPTION");
const APP_AUTHORS: &str = env!("CARGO_PKG_AUTHORS");
const APP_REPOSITORY: &str = env!("CARGO_PKG_REPOSITORY");
const APP_LICENSE: &str = env!("CARGO_PKG_LICENSE");

/// Renders a modal-style About window with application metadata.
pub fn render_about_window(app: &mut OrbitSenseApp, ctx: &egui::Context) {
    if !app.show_about {
        return;
    }

    let mut open = app.show_about;

    egui::Window::new("About Orbit Sense")
        .open(&mut open)
        .resizable(false)
        .collapsible(false)
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .min_width(420.0)
        .show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                // Title
                ui.add_space(8.0);
                ui.add(
                    egui::Image::new(egui::include_image!("../../assets/icon.png"))
                        .max_height(80.0)
                        .corner_radius(8),
                );
                ui.add_space(4.0);
                ui.label(
                    egui::RichText::new("Orbit Sense")
                        .size(26.0)
                        .strong()
                        .color(egui::Color32::from_rgb(100, 200, 255)),
                );
                ui.label(
                    egui::RichText::new(format!("Version {}", APP_VERSION))
                        .size(13.0)
                        .color(egui::Color32::GRAY),
                );
                ui.add_space(6.0);
            });

            ui.separator();

            // Description
            ui.add_space(4.0);
            ui.label(APP_DESCRIPTION);
            ui.add_space(8.0);

            egui::Grid::new("about_grid")
                .num_columns(2)
                .spacing([12.0, 6.0])
                .show(ui, |ui| {
                    ui.label(egui::RichText::new("Author").strong());
                    // CARGO_PKG_AUTHORS uses ":" as separator for multiple authors
                    ui.label(APP_AUTHORS.replace(':', ", "));
                    ui.end_row();

                    ui.label(egui::RichText::new("License").strong());
                    ui.label(APP_LICENSE);
                    ui.end_row();

                    ui.label(egui::RichText::new("Repository").strong());
                    ui.hyperlink(APP_REPOSITORY);
                    ui.end_row();
                });

            ui.add_space(8.0);
            ui.separator();
            ui.add_space(4.0);

            ui.vertical_centered(|ui| {
                ui.label(
                    egui::RichText::new("Satellite data sourced from CelesTrak.")
                        .size(11.0)
                        .color(egui::Color32::GRAY),
                );
                ui.label(
                    egui::RichText::new("Orbital propagation via the SGP4 model.")
                        .size(11.0)
                        .color(egui::Color32::GRAY),
                );
                ui.add_space(6.0);
            });
        });

    app.show_about = open;
}
