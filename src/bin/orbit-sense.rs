#![warn(clippy::all, rust_2018_idioms)]
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

//! Entrypoint for the application.

use macroquad::prelude::*;
use std::sync::Arc;
use tokio::runtime::Runtime;

use orbit_sense::app;

fn window_conf() -> Conf {
    Conf {
        window_title: "Orbit Sense V2".to_owned(),
        window_width: 1024,
        window_height: 768,
        ..Default::default()
    }
}

#[macroquad::main(window_conf)]
async fn main() {
    env_logger::init();

    // Spin up an explicit Tokio runtime locally to avoid conflicts with macroquad's
    // inherent async executor.
    let rt = Arc::new(Runtime::new().unwrap());

    let mut app: Option<app::OrbitSenseApp> = None;

    let mut camera = Camera3D {
        position: vec3(0., 0., 3.),
        up: vec3(0., 1., 0.),
        target: vec3(0., 0., 0.),
        ..Default::default()
    };

    let earth_tex = load_texture("/home/amouroug/.gemini/antigravity/brain/8cff8ad3-93ec-4866-82ea-10b6dc6fc5d9/earth_texture_1772398719808.png").await.ok();
    let mut wants_pointer = false;

    loop {
        clear_background(BLACK);

        if let Some(app_ref) = &app {
            if app_ref.render_mode == orbit_sense::app::RenderMode::Globe3D {
                // Update 3D Camera Controls (simple orbital drag)
                if !wants_pointer && is_mouse_button_down(MouseButton::Left) {
                    let _delta = mouse_delta_position();
                    // Basic rotation logic here later
                }

                set_camera(&camera);

                // Render globe and satellites
                orbit_sense::ui::map::render_macroquad_3d(app_ref, &earth_tex);

                // Reset camera to standard 2D
                set_default_camera();
            }
        }

        // Draw Egui UI overlays
        egui_macroquad::ui(|egui_ctx| {
            if app.is_none() {
                app = Some(app::OrbitSenseApp::new(rt.clone(), egui_ctx.clone()));
            }

            if let Some(app_ref) = &mut app {
                app_ref.update(egui_ctx);
                wants_pointer = egui_ctx.wants_pointer_input();
            }
        });

        // Physically draw Egui UI overlays on top
        egui_macroquad::draw();

        next_frame().await;
    }
}
