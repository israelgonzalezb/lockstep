#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release

mod app;
mod db;

use app::LockstepApp;
use eframe::egui;

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([800.0, 600.0])
            .with_min_inner_size([300.0, 220.0])
            .with_title("Lockstep"),
        ..Default::default()
    };

    eframe::run_native(
        "Lockstep",
        options,
        Box::new(|cc| Ok(Box::new(LockstepApp::new(cc)))),
    )
}
