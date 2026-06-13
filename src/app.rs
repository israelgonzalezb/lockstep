use eframe::egui;
use crate::db::Db;

pub struct LockstepApp {
    db: Db,
}

impl LockstepApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // Enforce the Null Vector aesthetic
        let mut visuals = egui::Visuals::dark();
        visuals.override_text_color = Some(egui::Color32::WHITE);
        visuals.panel_fill = egui::Color32::BLACK;
        visuals.window_fill = egui::Color32::BLACK;
        cc.egui_ctx.set_visuals(visuals);

        let db = Db::new().expect("Failed to initialize Lockstep SQLite database.");

        Self {
            db,
        }
    }
}

impl eframe::App for LockstepApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        ui.heading("LOCKSTEP // THE NULL VECTOR");
        ui.separator();
        
        ui.columns(2, |columns| {
            columns[0].vertical(|ui| {
                ui.label(" [ TIMELINE ] ");
                ui.label("Day plan stacking visualization...");
            });
            columns[1].vertical(|ui| {
                ui.label(" [ TELEMETRY ] ");
                ui.label("Task details & statistics...");
            });
        });
    }
}
