use libc::{c_int, c_uint, c_char, c_void};
use std::ffi::CString;
use std::slice;

use eframe::egui;
use rfd::FileDialog;
use image::{ImageBuffer, Rgb};

//
// Minimal FFI bindings for LibRaw
//

#[repr(C)]
pub struct LibRawData {
    _private: [u8; 0],
}

#[repr(C)]
pub struct LibRawProcessedImage {
    pub type_: c_int,
    pub colors: c_int,
    pub height: c_int,
    pub width: c_int,
    pub bits: c_int,
    pub data: *mut c_void,
    pub data_size: c_int,
    // Other fields omitted.
}

extern "C" {
    fn libraw_init(flags: c_uint) -> *mut LibRawData;
    fn libraw_open_file(raw: *mut LibRawData, filename: *const c_char) -> c_int;
    fn libraw_unpack(raw: *mut LibRawData) -> c_int;
    // Removed call to libraw_adjust_output_parameters for now.
    fn libraw_dcraw_process(raw: *mut LibRawData) -> c_int;
    fn libraw_dcraw_make_mem_image(raw: *mut LibRawData, err: *mut c_int) -> *mut LibRawProcessedImage;
    fn libraw_dcraw_clear_mem(image: *mut LibRawProcessedImage);
    fn libraw_close(raw: *mut LibRawData);
}

/// Decodes an ARW file using LibRaw and returns a tuple of (RGB data, width, height).
fn decode_arw_file(path: &str) -> Result<(Vec<u8>, u32, u32), String> {
    unsafe {
        let raw = libraw_init(0);
        if raw.is_null() {
            return Err("Failed to initialize LibRaw".into());
        }
        let c_path = CString::new(path).map_err(|e| e.to_string())?;
        let ret = libraw_open_file(raw, c_path.as_ptr());
        if ret != 0 {
            libraw_close(raw);
            return Err(format!("libraw_open_file failed with error code {}", ret));
        }
        let ret = libraw_unpack(raw);
        if ret != 0 {
            libraw_close(raw);
            return Err(format!("libraw_unpack failed with error code {}", ret));
        }
        // Note: We have removed the call to libraw_adjust_output_parameters here.
        let ret = libraw_dcraw_process(raw);
        if ret != 0 {
            libraw_close(raw);
            return Err(format!("libraw_dcraw_process failed with error code {}", ret));
        }
        let mut err: c_int = 0;
        let processed_image = libraw_dcraw_make_mem_image(raw, &mut err as *mut c_int);
        if processed_image.is_null() || err != 0 {
            libraw_close(raw);
            return Err(format!("libraw_dcraw_make_mem_image failed with error code {}", err));
        }
        let image = &*processed_image;
        if image.data.is_null() {
            libraw_dcraw_clear_mem(processed_image);
            libraw_close(raw);
            return Err("Processed image data pointer is null".into());
        }
        let width = image.width as u32;
        let height = image.height as u32;
        let data_size = image.data_size as usize;
        // For an 8-bit RGB image, we expect data_size = width * height * 3.
        let expected_size = (width as usize)
            .checked_mul(height as usize)
            .and_then(|v| v.checked_mul(3))
            .ok_or("Image dimensions too large")?;
        if data_size != expected_size {
            libraw_dcraw_clear_mem(processed_image);
            libraw_close(raw);
            return Err(format!(
                "Processed image data size ({}) does not match expected size ({})",
                data_size, expected_size
            ));
        }
        let data_slice = slice::from_raw_parts(image.data as *const u8, data_size);
        let image_data = data_slice.to_vec();
        libraw_dcraw_clear_mem(processed_image);
        libraw_close(raw);
        Ok((image_data, width, height))
    }
}

//
// GUI Application using eframe/egui and rfd for file dialogs
//

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
                    "arw_image",
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
    let _ = eframe::run_native(
        "LibRaw ARW Viewer",
        native_options,
        Box::new(|_cc| Box::new(LibRawViewerApp::new())),
    );
}
