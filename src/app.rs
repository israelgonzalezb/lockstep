#![allow(clippy::collapsible_if)]

use eframe::egui;
use crate::db::Db;
use crate::stack::{self, ScheduleBlock, Task};

pub struct LockstepApp {
    db: Db,
    current_day: String,
    schedule_blocks: Vec<ScheduleBlock>,
    tasks: Vec<Task>,
    selected_task_index: Option<usize>,
    new_task_name: String,
    new_task_duration: String,
    error_message: Option<String>,
}

impl LockstepApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // Enforce the Null Vector aesthetic: extreme high-contrast monochrome
        let mut visuals = egui::Visuals::dark();
        visuals.override_text_color = Some(egui::Color32::WHITE);
        visuals.panel_fill = egui::Color32::BLACK;
        visuals.window_fill = egui::Color32::BLACK;
        
        // Custom styling for widgets (white borders, black fill)
        visuals.widgets.noninteractive.bg_fill = egui::Color32::BLACK;
        visuals.widgets.noninteractive.fg_stroke = egui::Stroke::new(1.0, egui::Color32::WHITE);
        visuals.widgets.inactive.bg_fill = egui::Color32::BLACK;
        visuals.widgets.inactive.fg_stroke = egui::Stroke::new(1.0, egui::Color32::WHITE);
        visuals.widgets.hovered.bg_fill = egui::Color32::from_gray(30);
        visuals.widgets.hovered.fg_stroke = egui::Stroke::new(1.5, egui::Color32::WHITE);
        visuals.widgets.active.bg_fill = egui::Color32::WHITE;
        visuals.widgets.active.fg_stroke = egui::Stroke::new(1.5, egui::Color32::BLACK);

        cc.egui_ctx.set_visuals(visuals);

        let db = Db::new().expect("Failed to initialize Lockstep SQLite database.");
        
        let current_day = chrono::Local::now().format("%Y-%m-%d").to_string();
        let schedule_blocks = db.get_or_create_schedule(&current_day).unwrap_or_default();
        let tasks = db.get_all_tasks().unwrap_or_default();

        Self {
            db,
            current_day,
            schedule_blocks,
            tasks,
            selected_task_index: None,
            new_task_name: String::new(),
            new_task_duration: "30".to_string(),
            error_message: None,
        }
    }

    fn reload_all(&mut self) {
        self.schedule_blocks = self.db.get_or_create_schedule(&self.current_day).unwrap_or_default();
        self.tasks = self.db.get_all_tasks().unwrap_or_default();
    }

    fn shift_day(&mut self, days_delta: i64) {
        if let Ok(naive_date) = chrono::NaiveDate::parse_from_str(&self.current_day, "%Y-%m-%d") {
            if let Some(new_date) = naive_date.checked_add_signed(chrono::Duration::days(days_delta)) {
                self.current_day = new_date.format("%Y-%m-%d").to_string();
                self.reload_all();
                self.error_message = None;
            }
        }
    }
}

fn format_time(minutes: i32) -> String {
    let hours = minutes / 60;
    let mins = minutes % 60;
    format!("{:02}:{:02}", hours, mins)
}

impl eframe::App for LockstepApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        ui.spacing_mut().item_spacing = egui::vec2(10.0, 10.0);
        
        // Header Bar
        ui.horizontal(|ui| {
            ui.heading("LOCKSTEP // THE NULL VECTOR");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button(" > ").clicked() {
                    self.shift_day(1);
                }
                ui.label(&self.current_day);
                if ui.button(" < ").clicked() {
                    self.shift_day(-1);
                }
                if ui.button("Today").clicked() {
                    self.current_day = chrono::Local::now().format("%Y-%m-%d").to_string();
                    self.reload_all();
                    self.error_message = None;
                }
            });
        });
        ui.separator();

        // Error Banner (cloned to avoid borrow checker errors)
        if let Some(msg) = self.error_message.clone() {
            ui.group(|ui| {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(format!("[!] WARNING: {}", msg)).color(egui::Color32::WHITE));
                    if ui.button("Dismiss").clicked() {
                        self.error_message = None;
                    }
                });
            });
        }

        // Columns Layout
        ui.columns(2, |columns| {
            // Column 0: Timeline
            columns[0].vertical(|ui| {
                ui.label(egui::RichText::new(" [ TIMELINE / SCHEDULE ] ").strong());
                ui.separator();

                egui::ScrollArea::vertical().id_salt("timeline_scroll").show(ui, |ui| {
                    let mut trigger_db_save = false;
                    let num_blocks = self.schedule_blocks.len();

                    for idx in 0..num_blocks {
                        // Extract copyable properties first to release borrow on self.schedule_blocks
                        let block = &self.schedule_blocks[idx];
                        let start_str = format_time(block.start_minute);
                        let end_str = format_time(block.start_minute + block.duration_minutes);
                        let block_name = block.task_name.clone();
                        let duration = block.duration_minutes;
                        let is_done_val = block.is_done;
                        let is_task = block.task_id.is_some();
                        
                        // Visual frame for each block
                        ui.group(|ui| {
                            ui.horizontal(|ui| {
                                // Time Indicator
                                ui.label(egui::RichText::new(format!("{} - {}", start_str, end_str)).code());
                                
                                ui.separator();

                                // Checkbox for status
                                if is_task {
                                    let checkbox_label = if is_done_val {
                                        egui::RichText::new(&block_name).strikethrough().color(egui::Color32::GRAY)
                                    } else {
                                        egui::RichText::new(&block_name).strong()
                                    };
                                    
                                    let mut temp_done = is_done_val;
                                    if ui.checkbox(&mut temp_done, checkbox_label).changed() {
                                        self.schedule_blocks[idx].is_done = temp_done;
                                        trigger_db_save = true;
                                    }
                                } else {
                                    ui.label(egui::RichText::new(&block_name).color(egui::Color32::DARK_GRAY));
                                }

                                // Controls align right
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    if is_task {
                                        // Delete Task Block
                                        if ui.button("Delete").clicked() {
                                            if stack::delete_block(&mut self.schedule_blocks, idx) {
                                                trigger_db_save = true;
                                            }
                                        }
                                    } else {
                                        // Split / Insert Selected Task
                                        if let Some(sel_idx) = self.selected_task_index {
                                            if sel_idx < self.tasks.len() {
                                                let task = &self.tasks[sel_idx];
                                                let label = format!("Insert {}", task.name);
                                                if ui.button(label).clicked() {
                                                    let duration_to_split = std::cmp::min(task.default_duration, duration);
                                                    if stack::split_block(
                                                        &mut self.schedule_blocks,
                                                        idx,
                                                        task.id,
                                                        task.name.clone(),
                                                        self.current_day.clone(),
                                                        duration_to_split,
                                                    ) {
                                                        trigger_db_save = true;
                                                    }
                                                }
                                            }
                                        }
                                    }

                                    // Duration adjustments
                                    if ui.button("+5m").clicked() {
                                        if stack::adjust_block_duration(&mut self.schedule_blocks, idx, 5) {
                                            trigger_db_save = true;
                                            self.error_message = None;
                                        } else {
                                            self.error_message = Some("Cannot expand: downstream plan buffer limit reached.".to_string());
                                        }
                                    }
                                    if ui.button("-5m").clicked() {
                                        if stack::adjust_block_duration(&mut self.schedule_blocks, idx, -5) {
                                            trigger_db_save = true;
                                            self.error_message = None;
                                        } else {
                                            self.error_message = Some("Cannot shrink: block duration must be greater than zero.".to_string());
                                        }
                                    }

                                    ui.label(format!("{}m", duration));
                                });
                            });
                        });
                    }

                    if trigger_db_save {
                        let day_str = self.current_day.clone();
                        let blocks_clone = self.schedule_blocks.clone();
                        if let Err(e) = self.db.save_schedule_blocks(&day_str, &blocks_clone) {
                            self.error_message = Some(format!("Database write error: {}", e));
                        }
                        self.reload_all();
                    }
                });
            });

            // Column 1: Task Armory & Statistics
            columns[1].vertical(|ui| {
                ui.label(egui::RichText::new(" [ TASK ARMORY ] ").strong());
                ui.separator();

                // Create New Task Form
                ui.group(|ui| {
                    ui.label("New Task Template");
                    ui.horizontal(|ui| {
                        ui.text_edit_singleline(&mut self.new_task_name);
                        ui.add(egui::TextEdit::singleline(&mut self.new_task_duration).desired_width(40.0));
                        ui.label("min");
                        if ui.button("Create").clicked() {
                            if !self.new_task_name.trim().is_empty() {
                                if let Ok(dur) = self.new_task_duration.trim().parse::<i32>() {
                                    if dur > 0 {
                                        if let Err(e) = self.db.add_task(&self.new_task_name, dur) {
                                            self.error_message = Some(format!("Failed to create task: {}", e));
                                        } else {
                                            self.new_task_name.clear();
                                            self.new_task_duration = "30".to_string();
                                            self.reload_all();
                                        }
                                    }
                                }
                            }
                        }
                    });
                });

                ui.separator();

                // Task List
                ui.label("Created Task Templates:");
                egui::ScrollArea::vertical().id_salt("tasks_scroll").max_height(250.0).show(ui, |ui| {
                    for idx in 0..self.tasks.len() {
                        let task = &self.tasks[idx];
                        let is_selected = self.selected_task_index == Some(idx);
                        
                        let text = format!("{} ({}m)", task.name, task.default_duration);
                        let response = if is_selected {
                            ui.add(egui::Button::new(
                                egui::RichText::new(&text).color(egui::Color32::BLACK)
                            ).fill(egui::Color32::WHITE))
                        } else {
                            ui.button(&text)
                        };

                        if response.clicked() {
                            if is_selected {
                                self.selected_task_index = None;
                            } else {
                                self.selected_task_index = Some(idx);
                            }
                        }
                    }
                });

                ui.separator();

                // Statistics Telemetry
                ui.label(egui::RichText::new(" [ TELEMETRY ] ").strong());
                ui.separator();

                let mut total_planned_mins = 0;
                let mut completed_mins = 0;
                let mut no_plan_mins = 0;

                for block in &self.schedule_blocks {
                    if block.task_id.is_some() {
                        total_planned_mins += block.duration_minutes;
                        if block.is_done {
                            completed_mins += block.duration_minutes;
                        }
                    } else {
                        no_plan_mins += block.duration_minutes;
                    }
                }

                let planned_hours = total_planned_mins as f32 / 60.0;
                let completed_hours = completed_mins as f32 / 60.0;
                let no_plan_hours = no_plan_mins as f32 / 60.0;

                ui.label(format!("Planned Time: {:.1} hrs", planned_hours));
                ui.label(format!("No Plan Time : {:.1} hrs", no_plan_hours));
                ui.label(format!("Completed Time: {:.1} hrs", completed_hours));

                if total_planned_mins > 0 {
                    let ratio = completed_mins as f32 / total_planned_mins as f32;
                    ui.horizontal(|ui| {
                        ui.label("Completion Ratio:");
                        ui.add(egui::ProgressBar::new(ratio).text(format!("{:.0}%", ratio * 100.0)));
                    });
                } else {
                    ui.label("Completion Ratio: 0% (No tasks scheduled)");
                }
            });
        });
    }
}
