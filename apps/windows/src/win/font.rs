//! The system's message font, which the main window and every dialog are written in, so both
//! grow with Windows' Text size setting (Settings, Accessibility, Text size), not only with
//! DPI.
//!
//! `LUMENNA_TEXT_SCALE` multiplies it, for the UI tests: they check every dialog at the
//! largest Text size, 225%, without changing the person's own setting.

use windows::Win32::Foundation::{HWND, RECT};
use windows::Win32::Graphics::Gdi::{
    CreateFontW, DeleteObject, GetDC, GetMonitorInfoW, GetTextExtentPoint32W, GetTextMetricsW, HGDIOBJ, LOGFONTW,
    MONITOR_DEFAULTTONEAREST, MONITOR_DEFAULTTOPRIMARY, MONITORINFO, MonitorFromWindow, ReleaseDC, SelectObject,
    TEXTMETRICW,
};
use windows::Win32::UI::HiDpi::{GetDpiForSystem, GetDpiForWindow, SystemParametersInfoForDpi};
use windows::Win32::UI::WindowsAndMessaging::{NONCLIENTMETRICSW, SPI_GETNONCLIENTMETRICS};
use windows::core::PCWSTR;

/// The test override of Text size, if one is set.
fn scale_override() -> Option<f32> {
    std::env::var("LUMENNA_TEXT_SCALE").ok()?.parse::<f32>().ok().filter(|scale| *scale >= 1.0 && *scale <= 4.0)
}

/// The message font at `dpi`: its face, and its size with Text size applied.
pub fn message_font(dpi: u32) -> LOGFONTW {
    let mut metrics = NONCLIENTMETRICSW { cbSize: size_of::<NONCLIENTMETRICSW>() as u32, ..Default::default() };
    unsafe {
        let _ = SystemParametersInfoForDpi(SPI_GETNONCLIENTMETRICS.0, metrics.cbSize, Some((&raw mut metrics).cast()), 0, dpi);
    }
    let mut font = metrics.lfMessageFont;
    if let Some(scale) = scale_override() {
        font.lfHeight = (font.lfHeight as f32 * scale).round() as i32;
    }
    font
}

/// The font a dialog template names: the message font's face, and its size in points, which
/// the dialog manager scales by the monitor's DPI as it creates the dialog.
pub struct DialogFont {
    pub points: u16,
    pub face: String,
}

/// The message font as a dialog template takes it.
pub fn dialog_font() -> DialogFont {
    let font = message_font(96);
    let length = font.lfFaceName.iter().position(|&c| c == 0).unwrap_or(font.lfFaceName.len());
    let face = String::from_utf16_lossy(&font.lfFaceName[..length]);
    // At 96 DPI, a point is 96/72 pixels; lfHeight is the character height, negative.
    let points = (f64::from(font.lfHeight.unsigned_abs()) * 72.0 / 96.0).round() as u16;
    DialogFont { points: points.max(8), face: if face.is_empty() { "Segoe UI".to_owned() } else { face } }
}

/// The largest size up to `wanted` points at which a template of `width` by `height` dialog
/// units, plus `extra` around it, fits the work area of the monitor `owner` is on. Text size
/// at its largest would otherwise put a large form off the screen.
pub fn fitting_points(owner: Option<HWND>, face: &str, wanted: u16, width: i16, height: i16, extra: (i16, i16)) -> u16 {
    let (dpi, area) = unsafe {
        let monitor = match owner {
            Some(owner) => MonitorFromWindow(owner, MONITOR_DEFAULTTONEAREST),
            None => MonitorFromWindow(HWND::default(), MONITOR_DEFAULTTOPRIMARY),
        };
        let mut info = MONITORINFO { cbSize: size_of::<MONITORINFO>() as u32, ..Default::default() };
        let _ = GetMonitorInfoW(monitor, &mut info);
        let dpi = owner.map_or_else(|| GetDpiForSystem(), |owner| GetDpiForWindow(owner)).max(96);
        (dpi, info.rcWork)
    };
    let RECT { left, top, right, bottom } = area;
    // The frame and caption, generously, at this DPI.
    let chrome = (dpi as i32 * 40 / 96, dpi as i32 * 70 / 96);
    let fits = |points: u16| {
        let (unit_x, unit_y) = base_units(face, points, dpi);
        let w = i32::from(width + extra.0) * unit_x / 4 + chrome.0;
        let h = i32::from(height + extra.1) * unit_y / 8 + chrome.1;
        w <= right - left && h <= bottom - top
    };
    (8..=wanted).rev().find(|&points| fits(points)).unwrap_or(8)
}

/// A font's dialog base units at `dpi`: the average width of its letters and its height, in
/// pixels, as the dialog manager reckons them.
fn base_units(face: &str, points: u16, dpi: u32) -> (i32, i32) {
    let face: Vec<u16> = face.encode_utf16().chain(Some(0)).collect();
    let height = -(i32::from(points) * dpi as i32 / 72);
    unsafe {
        let font = CreateFontW(height, 0, 0, 0, 400, 0, 0, 0, Default::default(), Default::default(), Default::default(), Default::default(), 0, PCWSTR(face.as_ptr()));
        let dc = GetDC(None);
        let old = SelectObject(dc, HGDIOBJ(font.0));
        let mut metrics = TEXTMETRICW::default();
        let _ = GetTextMetricsW(dc, &mut metrics);
        let letters: Vec<u16> = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz".encode_utf16().collect();
        let mut size = Default::default();
        let _ = GetTextExtentPoint32W(dc, &letters, &mut size);
        SelectObject(dc, old);
        ReleaseDC(None, dc);
        let _ = DeleteObject(HGDIOBJ(font.0));
        ((size.cx / 26 + 1) / 2, metrics.tmHeight)
    }
}
