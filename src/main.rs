#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use eframe::egui;
use freedesktop_desktop_entry::{default_paths, get_languages_from_env, Iter};
use icon_loader::icon_loader_hicolor;
use std::collections::HashMap;
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::thread;

fn main() -> eframe::Result {
    env_logger::init();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([400.0, 300.0]),
        ..Default::default()
    };
    eframe::run_native(
        "App Launcher",
        options,
        Box::new(|_cc| Ok(Box::<MyApp>::default())),
    )
}

fn sanitize_command(command: &str) -> String {
    // Define placeholders to remove
    let placeholders = ["%u", "%U", "%f", "%F", "%d", "%D", "%n", "%N", "%i", "%c", "%k"];
    let mut sanitized_command = Vec::new();

    for part in command.split_whitespace() {
        // Remove placeholder arguments but keep everything else
        if placeholders.iter().any(|&ph| part.contains(ph)) {
            continue;
        }
        sanitized_command.push(part);
    }

    sanitized_command.join(" ")
}

#[derive(Default)]
struct MyApp {
    selected_item: Option<String>,
    items: Arc<Mutex<Vec<AppInfo>>>,
    search_query: String,
    icon_cache: Arc<Mutex<HashMap<String, egui::TextureHandle>>>,
}

#[derive(Debug, Clone)]
struct AppInfo {
    name: String,
    command: String,
    icon: Option<String>,
}

impl MyApp {
    /// Loads desktop files using freedesktop_desktop_entry
    fn load_desktop_files(&self) {
        let locales = get_languages_from_env();
        let items = Arc::clone(&self.items);
        thread::spawn(move || {
            let mut apps = Vec::new();
            for entry in Iter::new(default_paths()).entries(Some(&locales)) {
                if let Some(app_info) = MyApp::parse_desktop_entry(&entry, &locales) {
                    apps.push(app_info);
                }
            }
            *items.lock().unwrap() = apps;
        });
    }

    /// Parses a freedesktop desktop entry into `AppInfo`
    fn parse_desktop_entry(
        entry: &freedesktop_desktop_entry::DesktopEntry,
        locales: &[String],
    ) -> Option<AppInfo> {
        let name = entry.name(&locales)?;
        let command = entry.exec()?;
        let icon = entry.icon();

        // Sanitize the command: remove placeholders like %u, %U, %f, and %F
        let command = command
            .split_whitespace()
            .filter(|part| !part.starts_with('%'))
            .collect::<Vec<&str>>()
            .join(" ");

        Some(AppInfo {
            name: name.to_string(),
            command: command.to_string(),
            icon: icon.map(|i| i.to_string()),
        })
    }

    /// Loads an application icon by name as a texture for display
    fn load_icon(&self, ctx: &egui::Context, icon_name: &str) -> Option<egui::TextureHandle> {
        let mut icon_cache = self.icon_cache.lock().unwrap();

        // Check if the icon is already cached
        if let Some(texture) = icon_cache.get(icon_name) {
            return Some(texture.clone());
        }

        // Attempt to load the icon using icon_loader_hicolor
        if let Some(icon) = icon_loader_hicolor().load_icon(icon_name) {
            // Retrieve the icon file path for the desired size (e.g., 64x64)
            if let Some(path) = icon.file_for_size(64).path().to_str() {
                // Load the image
                if let Ok(img) = image::ImageReader::open(path)
                    .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))
                    .and_then(|r| {
                        r.decode()
                            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))
                    })
                {
                    let size = [img.width() as usize, img.height() as usize];
                    let pixels = match img.as_flat_samples_u8() {
                        Some(pixels) => pixels,
                        None => return None,
                    };
                    let color_image = egui::ColorImage::from_rgba_unmultiplied(size, pixels.as_slice());
                    let texture = ctx.load_texture(icon_name, color_image, Default::default());
                    icon_cache.insert(icon_name.to_string(), texture.clone());
                    return Some(texture);
                }
            }
        }

        None
    }
}

impl eframe::App for MyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if self.items.lock().unwrap().is_empty() {
            self.load_desktop_files();
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Select an App to Launch");

            // Search bar
            ui.horizontal(|ui| {
                ui.label("Search:");
                ui.text_edit_singleline(&mut self.search_query);
            });

            let items = self.items.lock().unwrap().clone();
            let filtered_items: Vec<AppInfo> = items
                .into_iter()
                .filter(|item| {
                    self.search_query.is_empty()
                        || item
                            .name
                            .to_lowercase()
                            .contains(&self.search_query.to_lowercase())
                })
                .collect();

            ui.separator();
            egui::ScrollArea::vertical().show(ui, |ui| {
                for item in filtered_items {
                    ui.horizontal(|ui| {
                        // Display the app icon if available
                        if let Some(icon_name) = &item.icon {
                            println!("Loading icon: {}", icon_name);
                            if let Some(texture) = self.load_icon(ctx, icon_name) {
                                ui.add(egui::Image::new(&texture).max_size(egui::vec2(24.0, 24.0))); // Adjust size if needed
                            } else {
                                // Fallback text if the icon can't be loaded
                                ui.label("📄");
                            }
                        } else {
                            // Fallback for apps without an icon
                            ui.label("📄");
                        }

                        // App name as a clickable button
                        if ui.button(&item.name).clicked() {
                            self.selected_item = Some(item.command.clone());
                        }
                    });
                    ui.separator();
                }
            });

            // Launch the selected application
            if let Some(command) = &self.selected_item {
                // sanitize the command before launching
                let command = sanitize_command(command);
                ui.label(format!("Launching: {}", command));
                if let Err(e) = Command::new(command).spawn() {
                    ui.colored_label(egui::Color32::RED, format!("Failed to launch: {}", e));
                } else {
                    ui.colored_label(egui::Color32::GREEN, "Launched successfully!");
                }
                self.selected_item = None;
            }
        });
    }
}
