//! Windows: the shell's document icon for a file type, rendered to PNG.
//! Depends only on `windows`, std and the sibling `icon_pixels` module, so
//! it can be compile-checked from a scratch crate on non-Windows hosts.

use std::ffi::c_void;

use windows::Win32::Graphics::Gdi::{
    BI_RGB, BITMAP, BITMAPINFO, BITMAPINFOHEADER, CreateCompatibleDC, DIB_RGB_COLORS, DeleteDC,
    DeleteObject, GetDIBits, GetObjectW, HBITMAP,
};
use windows::Win32::Storage::FileSystem::FILE_ATTRIBUTE_NORMAL;
use windows::Win32::UI::Controls::{IImageList, ILD_TRANSPARENT};
use windows::Win32::UI::Shell::{
    SHFILEINFOW, SHGFI_SYSICONINDEX, SHGFI_USEFILEATTRIBUTES, SHGetFileInfoW, SHGetImageList,
    SHIL_EXTRALARGE, SHIL_JUMBO,
};
use windows::Win32::UI::WindowsAndMessaging::{DestroyIcon, GetIconInfo, HICON, ICONINFO};
use windows::core::PCWSTR;

/// The icon Explorer shows for files with `extension`, as a square PNG.
/// `SHGFI_USEFILEATTRIBUTES` makes the shell answer from the registry for a
/// made-up name, so no file is touched. The icon comes from the system
/// image list at 48px (or 256px when more is asked for); None when the type
/// has no icon or conversion fails.
pub fn file_type_icon_png(extension: &str, size_px: usize) -> Option<Vec<u8>> {
    if extension.is_empty() || size_px == 0 {
        return None;
    }
    let name: Vec<u16> = format!("file.{extension}\0").encode_utf16().collect();
    let mut info = SHFILEINFOW::default();
    let found = unsafe {
        SHGetFileInfoW(
            PCWSTR(name.as_ptr()),
            FILE_ATTRIBUTE_NORMAL,
            Some(&mut info),
            std::mem::size_of::<SHFILEINFOW>() as u32,
            SHGFI_SYSICONINDEX | SHGFI_USEFILEATTRIBUTES,
        )
    };
    if found == 0 {
        return None;
    }

    let list_kind = if size_px > 48 {
        SHIL_JUMBO
    } else {
        SHIL_EXTRALARGE
    };
    let image_list: IImageList = unsafe { SHGetImageList(list_kind as i32) }.ok()?;
    let icon: HICON = unsafe { image_list.GetIcon(info.iIcon, ILD_TRANSPARENT.0) }.ok()?;
    let rgba = icon_rgba(icon);
    unsafe {
        let _ = DestroyIcon(icon);
    }
    let (width, height, pixels) = rgba?;
    super::icon_pixels::trimmed_png(width, height, &pixels)
}

/// Straight (non-premultiplied) RGBA pixels of an icon, top-down.
fn icon_rgba(icon: HICON) -> Option<(u32, u32, Vec<u8>)> {
    let mut icon_info = ICONINFO::default();
    unsafe { GetIconInfo(icon, &mut icon_info) }.ok()?;
    let color = icon_info.hbmColor;
    let mask = icon_info.hbmMask;
    let result = read_pixels(color, mask);
    unsafe {
        if !color.is_invalid() {
            let _ = DeleteObject(color.into());
        }
        if !mask.is_invalid() {
            let _ = DeleteObject(mask.into());
        }
    }
    result
}

fn read_pixels(color: HBITMAP, mask: HBITMAP) -> Option<(u32, u32, Vec<u8>)> {
    // Monochrome icons have no colour bitmap; the app falls back to its tile.
    if color.is_invalid() {
        return None;
    }
    let mut bitmap = BITMAP::default();
    let got = unsafe {
        GetObjectW(
            color.into(),
            std::mem::size_of::<BITMAP>() as i32,
            Some(&mut bitmap as *mut BITMAP as *mut c_void),
        )
    };
    if got == 0 || bitmap.bmWidth <= 0 || bitmap.bmHeight <= 0 {
        return None;
    }
    let (width, height) = (bitmap.bmWidth as u32, bitmap.bmHeight as u32);
    let byte_count = (width as usize) * (height as usize) * 4;

    let dc = unsafe { CreateCompatibleDC(None) };
    if dc.is_invalid() {
        return None;
    }
    let mut header = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: width as i32,
            // Negative height: rows top-down, the order PNG wants.
            biHeight: -(height as i32),
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            ..Default::default()
        },
        ..Default::default()
    };
    let mut pixels = vec![0u8; byte_count];
    let color_lines = unsafe {
        GetDIBits(
            dc,
            color,
            0,
            height,
            Some(pixels.as_mut_ptr() as *mut c_void),
            &mut header,
            DIB_RGB_COLORS,
        )
    };
    let mut mask_pixels = vec![0u8; byte_count];
    let mask_lines = if mask.is_invalid() {
        0
    } else {
        unsafe {
            GetDIBits(
                dc,
                mask,
                0,
                height,
                Some(mask_pixels.as_mut_ptr() as *mut c_void),
                &mut header,
                DIB_RGB_COLORS,
            )
        }
    };
    unsafe {
        let _ = DeleteDC(dc);
    }
    if color_lines == 0 {
        return None;
    }

    // GDI hands back BGRA. Icons without an alpha channel carry their
    // transparency in the mask instead (white = transparent).
    let has_alpha = pixels.chunks_exact(4).any(|pixel| pixel[3] != 0);
    for (index, pixel) in pixels.chunks_exact_mut(4).enumerate() {
        pixel.swap(0, 2);
        if !has_alpha {
            let masked = mask_lines != 0 && mask_pixels[index * 4] != 0;
            pixel[3] = if masked { 0 } else { 255 };
        }
    }
    Some((width, height, pixels))
}
