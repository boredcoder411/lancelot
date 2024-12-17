#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release

use eframe::egui;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::thread;

fn main() -> eframe::Result {
    env_logger::init(); // Log to stderr (if you run with `RUST_LOG=debug`).
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([320.0, 240.0]),
        ..Default::default()
    };
    eframe::run_native(
        "App Launcher",
        options,
        Box::new(|_cc| Ok(Box::<MyApp>::default())),
    )
}

#[derive(Default)]
struct MyApp {
    selected_item: Option<String>,
    items: Arc<Mutex<Vec<AppInfo>>>, // Use Arc and Mutex for thread-safe access
    search_query: String,
}

#[derive(Debug, Clone)]
struct AppInfo {
    name: String,
    command: String,
    icon: Option<String>,
}

impl MyApp {
    fn load_desktop_files(&mut self) {
        let dir = "/usr/share/applications";
        let items = Arc::clone(&self.items);

        // Spawn a separate thread to read the desktop files
        thread::spawn(move || {
            let mut apps = Vec::new();
            let path = PathBuf::from(dir);
            if path.is_dir() {
                if let Ok(entries) = fs::read_dir(path) {
                    for entry in entries.flatten() {
                        if let Some(ext) = entry.path().extension() {
                            if ext == "desktop" {
                                if let Ok(app_info) = MyApp::parse_desktop_file(entry.path()) {
                                    apps.push(app_info);
                                }
                            }
                        }
                    }
                }
            }
            // Update the shared items with the results
            *items.lock().unwrap() = apps;
        });
    }

    fn parse_desktop_file(path: PathBuf) -> Result<AppInfo, String> {
        let content = fs::read_to_string(&path).map_err(|e| e.to_string())?;
        let mut name = String::new();
        let mut command = String::new();
        let mut icon = None;

        for line in content.lines() {
            if line.starts_with("Name=") {
                name = line[5..].to_string();
            } else if line.starts_with("Exec=") {
                command = line[5..].to_string();
            } else if line.starts_with("Icon=") {
                icon = Some(line[5..].to_string());
            }
        }

        if name.is_empty() || command.is_empty() {
            return Err("Missing required fields in .desktop file".to_string());
        }

        Ok(AppInfo { name, command, icon })
    }
}

impl eframe::App for MyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Ensure the data is loaded at the start
        if self.items.lock().unwrap().is_empty() {
            self.load_desktop_files();
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Select an app to launch:");

            // Search bar
            ui.horizontal(|ui| {
                ui.label("Search:");
                ui.text_edit_singleline(&mut self.search_query);
            });

            // Filtered items
            let items = self.items.lock().unwrap();
            let filtered_items: Vec<&AppInfo> = items
                .iter()
                .filter(|item| {
                    self.search_query.is_empty() || item.name.to_lowercase().contains(&self.search_query.to_lowercase())
                })
                .collect();

            for item in filtered_items {
                ui.vertical(|ui| {
                    if let Some(icon) = &item.icon {
                        // do nothing for now
                    }
                    if ui.button(&item.name).clicked() {
                        self.selected_item = Some(item.command.clone());
                    }
                });
            }

            if let Some(command) = &self.selected_item {
                ui.label(format!("Launching: {}", command));

                // Launch the selected command
                if let Err(e) = Command::new(command).spawn() {
                    ui.label(format!("Failed to launch: {}", e));
                } else {
                    ui.label("Launched successfully!");
                }
                self.selected_item = None; // Reset selection
            }
        });
    }
}
