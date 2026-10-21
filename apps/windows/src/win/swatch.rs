//! A colour's mark beside a name: a project's, label's, saved filter's or block's swatch
//! (`lumenna_records::Swatch`), a filled circle drawn before a tree item's text or a pick's
//! choice. It is never the name: the name, and the colour's word, are in the text a screen
//! reader reads, and an image is no part of an item's name to MSAA or UI Automation.
//!
//! The swatch's `light` value on a light window, its `dark` on a dark one, and nothing under a
//! high-contrast theme, where the system's colours rule.

use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

use lumenna_records::Swatch;
use windows::Win32::Foundation::{COLORREF, HWND, RECT};
use windows::Win32::Graphics::Gdi::{
    BI_RGB, BITMAPINFO, BITMAPINFOHEADER, CreateDIBSection, DIB_RGB_COLORS, DeleteObject, HBITMAP, HDC,
    InvalidateRect,
};
use windows::Win32::UI::Controls::{
    HIMAGELIST, ILC_COLOR32, ILD_NORMAL, ImageList_Add, ImageList_Create, ImageList_Destroy, ImageList_Draw,
    TVM_SETIMAGELIST, TVSIL_NORMAL,
};
use windows::Win32::UI::HiDpi::{GetDpiForWindow, GetSystemMetricsForDpi};
use windows::Win32::UI::WindowsAndMessaging::SM_CXSMICON;

use super::{controls, dark};

/// Whether marks are drawn, and in which appearance: `None` under high contrast.
fn mode() -> Option<bool> {
    (!dark::high_contrast()).then(dark::on)
}

/// What a swatch is drawn in now, if it is drawn.
fn colour(swatch: &Swatch) -> Option<COLORREF> {
    let dark = mode()?;
    let hex = if dark { &swatch.dark } else { &swatch.light };
    let value = u32::from_str_radix(hex.strip_prefix('#')?, 16).ok()?;
    let (r, g, b) = ((value >> 16) & 0xFF, (value >> 8) & 0xFF, value & 0xFF);
    Some(COLORREF(r | g << 8 | b << 16))
}

/// A mark's side, in pixels: a small icon's at the window's DPI.
pub fn side(hwnd: HWND) -> i32 {
    unsafe { GetSystemMetricsForDpi(SM_CXSMICON, GetDpiForWindow(hwnd).max(96)) }
}

/// A `side`-pixel square holding a filled circle in `colour` (none: all clear), with its
/// alpha premultiplied as an image list draws a 32-bit image.
fn bitmap(side: i32, colour: Option<COLORREF>) -> HBITMAP {
    let info = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: side,
            biHeight: -side,
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            ..Default::default()
        },
        ..Default::default()
    };
    let mut bits = std::ptr::null_mut();
    let Ok(bitmap) = (unsafe { CreateDIBSection(None, &info, DIB_RGB_COLORS, &mut bits, None, 0) }) else {
        return HBITMAP::default();
    };
    let count = (side * side) as usize;
    let pixels = unsafe { std::slice::from_raw_parts_mut(bits.cast::<u32>(), count) };
    let Some(COLORREF(value)) = colour else {
        pixels.fill(0);
        return bitmap;
    };
    let (r, g, b) = (value & 0xFF, (value >> 8) & 0xFF, (value >> 16) & 0xFF);
    // Five eighths of the square, centred, its edge smoothed by sampling each pixel 4 by 4.
    let centre = f64::from(side) / 2.0;
    let radius = f64::from(side) * 5.0 / 16.0;
    for y in 0..side {
        for x in 0..side {
            let mut inside = 0u32;
            for sy in 0..4 {
                for sx in 0..4 {
                    let dx = f64::from(x) + (f64::from(sx) + 0.5) / 4.0 - centre;
                    let dy = f64::from(y) + (f64::from(sy) + 0.5) / 4.0 - centre;
                    inside += u32::from(dx * dx + dy * dy <= radius * radius);
                }
            }
            let alpha = inside * 255 / 16;
            let pre = |c: u32| c * alpha / 255;
            pixels[(y * side + x) as usize] = alpha << 24 | pre(r) << 16 | pre(g) << 8 | pre(b);
        }
    }
    bitmap
}

/// A tree's marks: an image list of a clear image and each swatch its rows have shown, by name,
/// given to the tree only while a row has one, so a tree without colours has no gap before
/// its text.
pub struct Marks {
    hwnd: HWND,
    list: Cell<HIMAGELIST>,
    /// The swatches the list holds, the image after the clear one each.
    held: RefCell<Vec<Swatch>>,
    /// What the list was drawn for: its side, and the appearance.
    drawn: Cell<(i32, Option<bool>)>,
    /// Whether a row has a swatch, as last drawn.
    wanted: Cell<bool>,
}

thread_local! {
    static EVERY: RefCell<Vec<Weak<Marks>>> = const { RefCell::new(Vec::new()) };
}

impl Marks {
    pub fn new(hwnd: HWND) -> Rc<Self> {
        let marks = Rc::new(Self {
            hwnd,
            list: Cell::new(HIMAGELIST::default()),
            held: RefCell::new(Vec::new()),
            drawn: Cell::new((0, None)),
            wanted: Cell::new(false),
        });
        EVERY.with(|every| every.borrow_mut().push(Rc::downgrade(&marks)));
        marks
    }

    /// The image a row shows: the clear one for none. Kept under high contrast too, where the
    /// tree has no image list to draw it from, so the marks come back with the list.
    pub fn image(&self, swatch: Option<&Swatch>) -> i32 {
        let Some(swatch) = swatch else { return 0 };
        let known = self.held.borrow().iter().position(|held| held.name == swatch.name);
        let index = known.unwrap_or_else(|| {
            self.held.borrow_mut().push(swatch.clone());
            self.held.borrow().len() - 1
        });
        if known.is_none() && !self.list.get().is_invalid() {
            let (side, _) = self.drawn.get();
            let image = bitmap(side, colour(swatch));
            unsafe {
                ImageList_Add(self.list.get(), image, None);
                let _ = DeleteObject(image.into());
            }
        }
        index as i32 + 1
    }

    /// Gives the tree its marks while `wanted`, drawn for its DPI and the appearance now, and
    /// takes them away otherwise.
    pub fn show(&self, wanted: bool) {
        self.wanted.set(wanted);
        let now = (side(self.hwnd), mode());
        let wanted = wanted && now.1.is_some();
        let list = self.list.get();
        if !wanted {
            if !list.is_invalid() {
                controls::send(self.hwnd, TVM_SETIMAGELIST, TVSIL_NORMAL as usize, 0);
                unsafe {
                    let _ = ImageList_Destroy(Some(list));
                }
                self.list.set(HIMAGELIST::default());
            }
            return;
        }
        if !list.is_invalid() && self.drawn.get() == now {
            return;
        }
        let (side, _) = now;
        let made = unsafe { ImageList_Create(side, side, ILC_COLOR32, 1 + self.held.borrow().len() as i32, 4) };
        for swatch in std::iter::once(None).chain(self.held.borrow().iter().map(Some)) {
            let image = bitmap(side, swatch.and_then(colour));
            unsafe {
                ImageList_Add(made, image, None);
                let _ = DeleteObject(image.into());
            }
        }
        controls::send(self.hwnd, TVM_SETIMAGELIST, TVSIL_NORMAL as usize, made.0);
        if !list.is_invalid() {
            unsafe {
                let _ = ImageList_Destroy(Some(list));
            }
        }
        self.list.set(made);
        self.drawn.set(now);
        unsafe {
            let _ = InvalidateRect(Some(self.hwnd), None, true);
        }
    }
}

impl Drop for Marks {
    fn drop(&mut self) {
        let list = self.list.get();
        if !list.is_invalid() {
            unsafe {
                let _ = ImageList_Destroy(Some(list));
            }
        }
    }
}

/// The appearance or the DPI changed: every tree with marks draws them again for it, a
/// high-contrast theme taking them away and its end bringing them back.
pub fn redraw_all() {
    EVERY.with(|every| {
        every.borrow_mut().retain(|marks| marks.strong_count() > 0);
        for marks in every.borrow().iter().filter_map(Weak::upgrade) {
            marks.show(marks.wanted.get());
        }
    });
}

/// Draws a swatch's mark at `at`'s left, `side` pixels square, centred down it, for a list
/// that draws its own items; the width it took, or 0 where nothing is drawn.
pub fn draw(dc: HDC, at: &RECT, swatch: Option<&Swatch>, side: i32) -> i32 {
    let Some(colour) = swatch.and_then(colour) else { return 0 };
    unsafe {
        let list = ImageList_Create(side, side, ILC_COLOR32, 1, 0);
        let image = bitmap(side, Some(colour));
        ImageList_Add(list, image, None);
        let _ = DeleteObject(image.into());
        let top = at.top + (at.bottom - at.top - side) / 2;
        let _ = ImageList_Draw(list, 0, dc, at.left, top, ILD_NORMAL);
        let _ = ImageList_Destroy(Some(list));
    }
    side
}
