mod decoder;

use decoder::decode_arw_file;
use eframe::egui;
use rfd::FileDialog;
use image::{ImageBuffer, Rgb};

struct LibRawViewerApp {
    texture: Option<egui::TextureHandle>,
    image_data: Option<(Vec<u8>, u32, u32)>,
}

impl LibRawViewerApp {
    fn new() -> Self {
        Self {
            texture: None,
            image_data: None,
        }
    }

    fn load_arw(&mut self, path: &str, ctx: &egui::Context) {
        match decode_arw_file(path) {
            Ok((data, width, height)) => {
                self.image_data = Some((data.clone(), width, height));
                let pixels: Vec<egui::Color32> = data
                    .chunks(3)
                    .map(|chunk| egui::Color32::from_rgb(chunk[0], chunk[1], chunk[2]))
                    .collect();
                let color_image = egui::ColorImage {
                    size: [width as usize, height as usize],
                    pixels,
                };
                self.texture = Some(ctx.load_texture(
                    "arw_thumb",
                    color_image,
                    egui::TextureOptions::default(),
                ));
            }
            Err(e) => {
                eprintln!("Error decoding ARW: {}", e);
            }
        }
    }

    fn save_png(&self, path: &str) -> Result<(), String> {
        if let Some((data, width, height)) = &self.image_data {
            let buffer: ImageBuffer<Rgb<u8>, _> =
                ImageBuffer::from_raw(*width, *height, data.clone())
                    .ok_or("Failed to create image buffer")?;
            buffer.save(path).map_err(|e| e.to_string())
        } else {
            Err("No image loaded".into())
        }
    }
}

impl eframe::App for LibRawViewerApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            if ui.button("Load ARW File").clicked() {
                if let Some(path) = FileDialog::new().add_filter("ARW", &["arw"]).pick_file() {
                    let path_str = path.to_string_lossy().to_string();
                    self.load_arw(&path_str, ctx);
                }
            }
            if let Some(texture) = &self.texture {
                ui.image(texture, texture.size_vec2());
            }
            if ui.button("Save as PNG").clicked() {
                if let Some(save_path) = FileDialog::new().save_file() {
                    let save_path_str = save_path.to_string_lossy().to_string();
                    match self.save_png(&save_path_str) {
                        Ok(_) => println!("Saved PNG to {}", save_path_str),
                        Err(e) => eprintln!("Error saving PNG: {}", e),
                    }
                }
            }
        });
    }
}

fn main() {
    let native_options = eframe::NativeOptions::default();
    eframe::run_native(
        "LibRaw ARW Viewer",
        native_options,
        Box::new(|_cc| Box::new(LibRawViewerApp::new())),
    );
}
