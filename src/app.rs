#![allow(clippy::collapsible_if)]

use eframe::egui;
use crate::db::Db;
use crate::stack::{self, ScheduleBlock, Task};
use chrono::Timelike;

pub struct LockstepApp {
    db: Db,
    current_day: String,
    schedule_blocks: Vec<ScheduleBlock>,
    tasks: Vec<Task>,
    selected_task_index: Option<usize>,
    new_task_name: String,
    new_task_duration: String,
    new_task_category: String,
    new_task_notes: String,
    new_task_category_select: String,
    selected_task_category_select: String,
    error_message: Option<String>,
    
    // Notes editing buffer for the selected template task
    selected_task_notes_buf: String,
    selected_task_category_buf: String,
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
            new_task_category: String::new(),
            new_task_notes: String::new(),
            new_task_category_select: "Unassigned".to_string(),
            selected_task_category_select: "Unassigned".to_string(),
            error_message: None,
            selected_task_notes_buf: String::new(),
            selected_task_category_buf: String::new(),
        }
    }

    fn reload_all(&mut self) {
        self.schedule_blocks = self.db.get_or_create_schedule(&self.current_day).unwrap_or_default();
        self.tasks = self.db.get_all_tasks().unwrap_or_default();
        
        // Refresh selected task buffer fields if any are active
        if let Some(sel_idx) = self.selected_task_index {
            if sel_idx < self.tasks.len() {
                self.selected_task_notes_buf = self.tasks[sel_idx].notes.clone();
                self.selected_task_category_buf = self.tasks[sel_idx].category.clone();
                self.selected_task_category_select = self.tasks[sel_idx].category.clone();
            } else {
                self.selected_task_index = None;
            }
        }
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

fn draw_pie_chart(ui: &mut egui::Ui, categories: &[(String, i32)]) {
    let total_mins: i32 = categories.iter().map(|(_, m)| m).sum();
    if total_mins == 0 {
        ui.label("No active schedule blocks.");
        return;
    }

    let desired_size = egui::vec2(130.0, 130.0);
    let (rect, _) = ui.allocate_exact_size(desired_size, egui::Sense::hover());
    let painter = ui.painter_at(rect);

    let cx = rect.center().x;
    let cy = rect.center().y;
    let radius = 55.0;

    let mut start_angle: f32 = 0.0;
    let shades = [
        egui::Color32::from_gray(255), // White
        egui::Color32::from_gray(60),  // Dark gray
        egui::Color32::from_gray(180), // Light gray
        egui::Color32::from_gray(120), // Medium gray
        egui::Color32::from_gray(30),  // Very dark gray
        egui::Color32::from_gray(220), // Off-white
    ];

    for (idx, (_, mins)) in categories.iter().enumerate() {
        let fraction = *mins as f32 / total_mins as f32;
        let sweep_angle = fraction * 2.0 * std::f32::consts::PI;
        let end_angle = start_angle + sweep_angle;

        if fraction > 0.001 {
            let mut points = vec![egui::pos2(cx, cy)];
            let steps = (sweep_angle * 12.0).ceil() as i32;
            let steps = std::cmp::max(6, steps);
            for step in 0..=steps {
                let angle = start_angle + (sweep_angle * step as f32 / steps as f32);
                let px = cx + radius * angle.cos();
                let py = cy + radius * angle.sin();
                points.push(egui::pos2(px, py));
            }
            points.push(egui::pos2(cx, cy));

            let color = shades[idx % shades.len()];
            painter.add(egui::Shape::convex_polygon(
                points,
                color,
                egui::Stroke::new(1.0, egui::Color32::BLACK),
            ));
        }

        start_angle = end_angle;
    }
    
    painter.circle_stroke(
        egui::pos2(cx, cy),
        radius,
        egui::Stroke::new(1.5, egui::Color32::WHITE)
    );
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

        // Error Banner
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

        // Calculate Responsive layout widths
        let is_narrow = ui.available_width() < 600.0;

        // Render contents based on responsive width state
        if is_narrow {
            egui::ScrollArea::vertical().id_salt("responsive_scroll").show(ui, |ui| {
                self.render_timeline_column(ui);
                ui.separator();
                self.render_telemetry_column(ui);
            });
        } else {
            ui.columns(2, |columns| {
                columns[0].vertical(|ui| {
                    self.render_timeline_column(ui);
                });
                columns[1].vertical(|ui| {
                    self.render_telemetry_column(ui);
                });
            });
        }
    }
}

impl LockstepApp {
    fn render_timeline_column(&mut self, ui: &mut egui::Ui) {
        ui.label(egui::RichText::new(" [ TIMELINE / SCHEDULE ] ").strong());
        ui.separator();

        let now = chrono::Local::now();
        let current_minute = now.hour() as i32 * 60 + now.minute() as i32;

        egui::ScrollArea::vertical().id_salt("timeline_scroll").show(ui, |ui| {
            let mut trigger_db_save = false;
            let num_blocks = self.schedule_blocks.len();

            for idx in 0..num_blocks {
                let block = &self.schedule_blocks[idx];
                let start_str = format_time(block.start_minute);
                let end_str = format_time(block.start_minute + block.duration_minutes);
                let block_name = block.task_name.clone();
                let duration = block.duration_minutes;
                let is_done_val = block.is_done;
                let is_task = !block.task_ids.is_empty();
                let block_notes = block.notes.clone();

                // Check if current time falls in this block
                let is_active_now = block.start_minute <= current_minute
                    && current_minute < block.start_minute + duration;

                // Height proportional to time: height = max(40.0, duration * 0.75)
                let proportional_height = f32::max(40.0, duration as f32 * 0.75);

                // Highlight active block with a white border
                let stroke_color = if is_active_now {
                    egui::Color32::WHITE
                } else {
                    egui::Color32::from_gray(60)
                };

                let mut action_taken = false;

                ui.group(|ui| {
                    ui.set_min_height(proportional_height);
                    ui.style_mut().visuals.widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0, stroke_color);

                    ui.vertical(|ui| {
                        // Row 1: Time, Cursor, checkbox, and split/delete actions
                        ui.horizontal_wrapped(|ui| {
                            // Time Indicator
                            ui.label(egui::RichText::new(format!("{} - {} ({}m)", start_str, end_str, duration)).code().color(egui::Color32::WHITE));
                            
                            // Active Cursor Marker
                            if is_active_now {
                                ui.label(egui::RichText::new(" [ NOW ] ").strong().color(egui::Color32::WHITE));
                            }

                            // Checkbox for status
                            if is_task {
                                let checkbox_label = if is_done_val {
                                    egui::RichText::new(&block_name).strikethrough().color(egui::Color32::GRAY)
                                } else {
                                    egui::RichText::new(&block_name).strong().color(egui::Color32::WHITE)
                                };
                                
                                let mut temp_done = is_done_val;
                                if ui.checkbox(&mut temp_done, checkbox_label).changed() {
                                    self.schedule_blocks[idx].is_done = temp_done;
                                    trigger_db_save = true;
                                    action_taken = true;
                                }
                            } else {
                                ui.label(egui::RichText::new(&block_name).color(egui::Color32::DARK_GRAY));
                            }

                            // Controls align right for main actions
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if is_task {
                                    // Delete Task Block
                                    if ui.button("Delete").clicked() {
                                        if stack::delete_block(&mut self.schedule_blocks, idx) {
                                            trigger_db_save = true;
                                            action_taken = true;
                                        }
                                    }
                                } else {
                                    // Split & Insert Selected Task
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
                                                    action_taken = true;
                                                }
                                            }
                                        }
                                    }
                                }

                                // Double Up Button (Multi-task assignment)
                                if let Some(sel_idx) = self.selected_task_index {
                                    if sel_idx < self.tasks.len() {
                                        let task = &self.tasks[sel_idx];
                                        if !self.schedule_blocks[idx].task_ids.contains(&task.id) {
                                            if ui.button("+ Link").clicked() {
                                                if stack::append_task_to_block(
                                                    &mut self.schedule_blocks,
                                                    idx,
                                                    task.id,
                                                    task.name.clone(),
                                                ) {
                                                    trigger_db_save = true;
                                                    action_taken = true;
                                                }
                                            }
                                        }
                                    }
                                }
                            });
                        });

                        // Row 2: Increments wrapping beautifully
                        ui.horizontal_wrapped(|ui| {
                            ui.label(egui::RichText::new("Adjust:").size(11.0).color(egui::Color32::GRAY));
                            if ui.button("-60m").clicked() {
                                if stack::adjust_block_duration(&mut self.schedule_blocks, idx, -60) {
                                    trigger_db_save = true;
                                    action_taken = true;
                                } else {
                                    self.error_message = Some("Cannot shrink: block duration must be greater than zero.".to_string());
                                }
                            }
                            if ui.button("-30m").clicked() {
                                if stack::adjust_block_duration(&mut self.schedule_blocks, idx, -30) {
                                    trigger_db_save = true;
                                    action_taken = true;
                                } else {
                                    self.error_message = Some("Cannot shrink: block duration must be greater than zero.".to_string());
                                }
                            }
                            if ui.button("-5m").clicked() {
                                if stack::adjust_block_duration(&mut self.schedule_blocks, idx, -5) {
                                    trigger_db_save = true;
                                    action_taken = true;
                                } else {
                                    self.error_message = Some("Cannot shrink: block duration must be greater than zero.".to_string());
                                }
                            }
                            if ui.button("+5m").clicked() {
                                if stack::adjust_block_duration(&mut self.schedule_blocks, idx, 5) {
                                    trigger_db_save = true;
                                    action_taken = true;
                                } else {
                                    self.error_message = Some("Cannot expand by 5m: downstream buffer limit reached.".to_string());
                                }
                            }
                            if ui.button("+30m").clicked() {
                                if stack::adjust_block_duration(&mut self.schedule_blocks, idx, 30) {
                                    trigger_db_save = true;
                                    action_taken = true;
                                } else {
                                    self.error_message = Some("Cannot expand by 30m: downstream buffer limit reached.".to_string());
                                }
                            }
                            if ui.button("+60m").clicked() {
                                if stack::adjust_block_duration(&mut self.schedule_blocks, idx, 60) {
                                    trigger_db_save = true;
                                    action_taken = true;
                                } else {
                                    self.error_message = Some("Cannot expand by 60m: downstream buffer limit reached.".to_string());
                                }
                            }
                        });

                        // Expandable block notes input to keep timeline uncluttered
                        ui.collapsing("Notes", |ui| {
                            ui.horizontal(|ui| {
                                let mut temp_notes = block_notes.clone();
                                ui.text_edit_singleline(&mut temp_notes);
                                if temp_notes != block_notes {
                                    if ui.button("Save").clicked() {
                                        self.schedule_blocks[idx].notes = temp_notes;
                                        trigger_db_save = true;
                                        action_taken = true;
                                    }
                                }
                            });
                        });
                    });
                });

                // CRITICAL BUG FIX: Break out of the rendering loop immediately 
                // if a block has been deleted or mutated to prevent index out of bounds panic.
                if action_taken {
                    break;
                }
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
    }

    fn render_telemetry_column(&mut self, ui: &mut egui::Ui) {
        ui.label(egui::RichText::new(" [ TASK ARMORY & TELEMETRY ] ").strong());
        ui.separator();

        // Create New Task Form
        ui.group(|ui| {
            ui.label(egui::RichText::new("Create Task Template").strong());
            
            // Row 1: Name and Duration
            ui.horizontal(|ui| {
                ui.label("Name:");
                ui.text_edit_singleline(&mut self.new_task_name);
                ui.label("Min:");
                ui.add(egui::TextEdit::singleline(&mut self.new_task_duration).desired_width(35.0));
            });

            // Row 2: Category Selector
            ui.horizontal(|ui| {
                ui.label("Category:");
                
                let mut existing_categories: Vec<String> = self.tasks.iter()
                    .map(|t| t.category.clone())
                    .filter(|c| !c.is_empty() && c != "Unassigned")
                    .collect();
                existing_categories.sort();
                existing_categories.dedup();
                existing_categories.insert(0, "Unassigned".to_string());
                existing_categories.push("[ New Category ]".to_string());
                
                egui::ComboBox::from_id_salt("new_task_category_combo")
                    .selected_text(&self.new_task_category_select)
                    .show_ui(ui, |ui| {
                        for cat in &existing_categories {
                            ui.selectable_value(&mut self.new_task_category_select, cat.clone(), cat);
                        }
                    });
            });

            // Row 3: New category input text field (shown only if [ New Category ] is selected)
            if self.new_task_category_select == "[ New Category ]" {
                ui.horizontal(|ui| {
                    ui.label("New Category Name:");
                    ui.text_edit_singleline(&mut self.new_task_category);
                });
            }

            // Row 4: Notes
            ui.horizontal(|ui| {
                ui.label("Notes:");
                ui.text_edit_singleline(&mut self.new_task_notes);
            });

            // Row 5: Create template button
            ui.vertical_centered_justified(|ui| {
                if ui.button("Create Template").clicked() {
                    if !self.new_task_name.trim().is_empty() {
                        if let Ok(dur) = self.new_task_duration.trim().parse::<i32>() {
                            if dur > 0 {
                                let cat = if self.new_task_category_select == "[ New Category ]" {
                                    if self.new_task_category.trim().is_empty() {
                                        "Unassigned"
                                    } else {
                                        self.new_task_category.trim()
                                    }
                                } else {
                                    self.new_task_category_select.as_str()
                                };
                                if let Err(e) = self.db.add_task(
                                    &self.new_task_name,
                                    dur,
                                    &self.new_task_notes,
                                    cat,
                                ) {
                                    self.error_message = Some(format!("Failed to create task: {}", e));
                                } else {
                                    self.new_task_name.clear();
                                    self.new_task_duration = "30".to_string();
                                    self.new_task_category.clear();
                                    self.new_task_category_select = "Unassigned".to_string();
                                    self.new_task_notes.clear();
                                    self.reload_all();
                                }
                            }
                        }
                    }
                }
            });
        });

        ui.separator();

        // Task List (grouped by Category)
        ui.label(egui::RichText::new("Task Templates:").strong());
        
        let mut categories_list: Vec<String> = self.tasks.iter().map(|t| t.category.clone()).collect();
        categories_list.sort();
        categories_list.dedup();

        egui::ScrollArea::vertical().id_salt("tasks_scroll").max_height(160.0).show(ui, |ui| {
            for cat in &categories_list {
                ui.label(egui::RichText::new(format!(":: {}", cat.to_uppercase())).strong().color(egui::Color32::LIGHT_GRAY));
                
                for idx in 0..self.tasks.len() {
                    let task = &self.tasks[idx];
                    if &task.category != cat {
                        continue;
                    }
                    
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
                            self.selected_task_notes_buf = task.notes.clone();
                            self.selected_task_category_buf = task.category.clone();
                            self.selected_task_category_select = task.category.clone();
                        }
                    }
                }
            }
        });

        // Selected Template Detail & Notes Editor
        if let Some(sel_idx) = self.selected_task_index {
            if sel_idx < self.tasks.len() {
                let task_id = self.tasks[sel_idx].id;
                let task_name = self.tasks[sel_idx].name.clone();
                let task_notes = self.tasks[sel_idx].notes.clone();
                let task_category = self.tasks[sel_idx].category.clone();
                
                let mut should_reload = false;
                
                ui.group(|ui| {
                    ui.label(egui::RichText::new(format!("Selected Template: {}", task_name)).strong());
                    
                    // Row 1: Category dropdown selection
                    ui.horizontal(|ui| {
                        ui.label("Category:");
                        
                        let mut existing_categories: Vec<String> = self.tasks.iter()
                            .map(|t| t.category.clone())
                            .filter(|c| !c.is_empty() && c != "Unassigned")
                            .collect();
                        existing_categories.sort();
                        existing_categories.dedup();
                        existing_categories.insert(0, "Unassigned".to_string());
                        existing_categories.push("[ New Category ]".to_string());
                        
                        egui::ComboBox::from_id_salt("selected_task_category_combo")
                            .selected_text(&self.selected_task_category_select)
                            .show_ui(ui, |ui| {
                                for cat in &existing_categories {
                                    ui.selectable_value(&mut self.selected_task_category_select, cat.clone(), cat);
                                }
                            });
                    });
                    
                    // Row 2: Custom Category Text Field
                    if self.selected_task_category_select == "[ New Category ]" {
                        ui.horizontal(|ui| {
                            ui.label("New Category Name:");
                            ui.text_edit_singleline(&mut self.selected_task_category_buf);
                        });
                    }

                    // Save category if modified
                    let target_cat = if self.selected_task_category_select == "[ New Category ]" {
                        self.selected_task_category_buf.trim().to_string()
                    } else {
                        self.selected_task_category_select.clone()
                    };

                    if target_cat != task_category && !target_cat.is_empty() {
                        ui.horizontal(|ui| {
                            if ui.button("Save Category").clicked() {
                                let _ = self.db.update_task_category(task_id, &target_cat);
                                should_reload = true;
                            }
                        });
                    }
                    
                    // Row 3: Notes
                    ui.label("Notes:");
                    ui.text_edit_multiline(&mut self.selected_task_notes_buf);
                    if self.selected_task_notes_buf != task_notes {
                        if ui.button("Save Notes").clicked() {
                            let _ = self.db.update_task_notes(task_id, &self.selected_task_notes_buf);
                            should_reload = true;
                        }
                    }
                });

                if should_reload {
                    self.reload_all();
                }
            }
        }

        ui.separator();

        // Statistics Telemetry & Pie Chart
        ui.label(egui::RichText::new(" [ TELEMETRY ] ").strong());
        ui.separator();

        let mut total_planned_mins = 0;
        let mut completed_mins = 0;
        let mut no_plan_mins = 0;
        
        // Duration sum grouped by category
        let mut category_stats: std::collections::HashMap<String, i32> = std::collections::HashMap::new();

        for block in &self.schedule_blocks {
            if !block.task_ids.is_empty() {
                total_planned_mins += block.duration_minutes;
                if block.is_done {
                    completed_mins += block.duration_minutes;
                }
                
                // Group by category of the first task inside the block
                let primary_task_id = block.task_ids[0];
                let cat = self.tasks.iter()
                    .find(|t| t.id == primary_task_id)
                    .map(|t| t.category.clone())
                    .unwrap_or_else(|| "Unassigned".to_string());
                
                *category_stats.entry(cat).or_insert(0) += block.duration_minutes;
            } else {
                no_plan_mins += block.duration_minutes;
            }
        }

        let planned_hours = total_planned_mins as f32 / 60.0;
        let completed_hours = completed_mins as f32 / 60.0;
        let no_plan_hours = no_plan_mins as f32 / 60.0;

        let mut category_vec: Vec<(String, i32)> = category_stats.into_iter().collect();
        category_vec.sort_by(|a, b| b.1.cmp(&a.1)); // Sort largest slices first

        ui.columns(2, |telemetry_cols| {
            telemetry_cols[0].vertical(|ui| {
                ui.label(format!("Planned Time: {:.1} hrs", planned_hours));
                ui.label(format!("No Plan Time : {:.1} hrs", no_plan_hours));
                ui.label(format!("Completed Time: {:.1} hrs", completed_hours));

                if total_planned_mins > 0 {
                    let ratio = completed_mins as f32 / total_planned_mins as f32;
                    ui.horizontal(|ui| {
                        ui.label("Completed Ratio:");
                        ui.add(egui::ProgressBar::new(ratio).text(format!("{:.0}%", ratio * 100.0)));
                    });
                } else {
                    ui.label("Completed Ratio: 0% (No tasks scheduled)");
                }
            });

            telemetry_cols[1].vertical(|ui| {
                if total_planned_mins > 0 {
                    draw_pie_chart(ui, &category_vec);
                } else {
                    ui.label("No data for pie chart.");
                }
            });
        });

        // Display Pie Chart Legend
        if total_planned_mins > 0 {
            ui.label(egui::RichText::new("Category Allocation Legend:").strong().size(11.0));
            let shades = [
                egui::Color32::from_gray(255), // White
                egui::Color32::from_gray(60),  // Dark gray
                egui::Color32::from_gray(180), // Light gray
                egui::Color32::from_gray(120), // Medium gray
                egui::Color32::from_gray(30),  // Very dark gray
                egui::Color32::from_gray(220), // Off-white
            ];
            ui.horizontal_wrapped(|ui| {
                for (idx, (cat, mins)) in category_vec.iter().enumerate() {
                    let pct = (*mins as f32 / total_planned_mins as f32) * 100.0;
                    let color = shades[idx % shades.len()];
                    
                    ui.horizontal(|ui| {
                        let (rect, _) = ui.allocate_exact_size(egui::vec2(8.0, 8.0), egui::Sense::hover());
                        ui.painter().rect_filled(rect, 0.0, color);
                        ui.label(format!("{}: {:.0}%", cat, pct));
                    });
                }
            });
        }
    }
}
