use libc::{c_int, c_uint, c_char, c_void};
use std::ffi::CString;
use std::slice;

// FFI bindings and types for LibRaw.
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
    // Other fields are omitted.
}

extern "C" {
    fn libraw_init(flags: c_uint) -> *mut LibRawData;
    fn libraw_open_file(raw: *mut LibRawData, filename: *const c_char) -> c_int;
    fn libraw_unpack(raw: *mut LibRawData) -> c_int;
    fn libraw_set_output_bps(raw: *mut LibRawData, bps: c_int);
    fn libraw_dcraw_process(raw: *mut LibRawData) -> c_int;
    // We'll use the thumbnail extraction function.
    fn libraw_dcraw_make_mem_thumb(raw: *mut LibRawData, err: *mut c_int) -> *mut LibRawProcessedImage;
    // Full image extraction (for fallback).
    fn libraw_dcraw_make_mem_image(raw: *mut LibRawData, err: *mut c_int) -> *mut LibRawProcessedImage;
    fn libraw_dcraw_clear_mem(image: *mut LibRawProcessedImage);
    fn libraw_close(raw: *mut LibRawData);
}

/// Helper function to extract image data from a processed image.
/// Expects the image to be an 8-bit RGB image.
unsafe fn extract_image_data(image: &LibRawProcessedImage) -> Result<(Vec<u8>, u32, u32), String> {
    if image.data.is_null() {
        return Err("Processed image data pointer is null".into());
    }
    let width = image.width as u32;
    let height = image.height as u32;
    let data_size = image.data_size as usize;
    // For an 8-bit RGB image, expected size = width * height * 3.
    let expected_size = (width as usize)
        .checked_mul(height as usize)
        .and_then(|v| v.checked_mul(3))
        .ok_or("Image dimensions too large")?;
    if data_size != expected_size {
        return Err(format!(
            "Processed image data size ({}) does not match expected size ({})",
            data_size, expected_size
        ));
    }
    let data_slice = slice::from_raw_parts(image.data as *const u8, data_size);
    Ok((data_slice.to_vec(), width, height))
}

/// Decodes an ARW file using LibRaw.
/// It first tries to extract the embedded thumbnail. If that fails (error code -4),
/// it falls back to extracting the full image.
pub fn decode_arw_file(path: &str) -> Result<(Vec<u8>, u32, u32), String> {
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
        // Force output to 8 bits per channel.
        libraw_set_output_bps(raw, 8);
        let ret = libraw_dcraw_process(raw);
        if ret != 0 {
            libraw_close(raw);
            return Err(format!("libraw_dcraw_process failed with error code {}", ret));
        }
        let mut err: c_int = 0;
        let processed_thumb = libraw_dcraw_make_mem_thumb(raw, &mut err as *mut c_int);
        let result = if processed_thumb.is_null() {
            if err == -4 {
                // No thumbnail found; try full image extraction.
                println!("Thumbnail not available (error -4), falling back to full image extraction.");
                let processed_image = libraw_dcraw_make_mem_image(raw, &mut err as *mut c_int);
                if processed_image.is_null() || err != 0 {
                    libraw_close(raw);
                    return Err(format!("Full image extraction failed with error code {}", err));
                }
                extract_image_data(&*processed_image)
                    .and_then(|(data, w, h)| {
                        libraw_dcraw_clear_mem(processed_image);
                        Ok((data, w, h))
                    })
            } else {
                libraw_close(raw);
                return Err(format!("Thumbnail extraction failed with error code {}", err));
            }
        } else {
            let res = extract_image_data(&*processed_thumb);
            libraw_dcraw_clear_mem(processed_thumb);
            res
        };
        libraw_close(raw);
        result
    }
}
