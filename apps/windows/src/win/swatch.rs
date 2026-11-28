//! A colour's mark and an emoji beside a name: a project's, label's, saved filter's or block's
//! swatch (`lumenna_records::Swatch`), a filled circle drawn before a tree item's text or a
//! pick's choice, and a task's, project's or block's emoji after it, before the text. Neither is
//! ever the name: the name, and the colour's word, are in the text a screen reader reads, the
//! emoji in no text at all, and an image is no part of an item's name to MSAA or UI Automation.
//!
//! The swatch's `light` value on a light window, its `dark` on a dark one, and nothing under a
//! high-contrast theme, where the system's colours rule. The emoji is drawn in colour by
//! DirectWrite (Segoe UI Emoji) into the same image.

use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

use lumenna_records::Swatch;
use windows::Win32::Foundation::{COLORREF, HWND, RECT};
use windows::Win32::Graphics::Direct2D::Common::{D2D_RECT_F, D2D1_ALPHA_MODE_PREMULTIPLIED, D2D1_COLOR_F, D2D1_PIXEL_FORMAT};
use windows::Win32::Graphics::Direct2D::{
    D2D1_DRAW_TEXT_OPTIONS_CLIP, D2D1_DRAW_TEXT_OPTIONS_ENABLE_COLOR_FONT, D2D1_FACTORY_TYPE_SINGLE_THREADED,
    D2D1_FEATURE_LEVEL_DEFAULT, D2D1_RENDER_TARGET_PROPERTIES, D2D1_RENDER_TARGET_TYPE_DEFAULT,
    D2D1_RENDER_TARGET_USAGE_NONE, D2D1_TEXT_ANTIALIAS_MODE_GRAYSCALE, D2D1CreateFactory, ID2D1Factory,
};
use windows::Win32::Graphics::DirectWrite::{
    DWRITE_FACTORY_TYPE_SHARED, DWRITE_FONT_STRETCH_NORMAL, DWRITE_FONT_STYLE_NORMAL, DWRITE_FONT_WEIGHT_NORMAL,
    DWRITE_MEASURING_MODE_NATURAL, DWRITE_PARAGRAPH_ALIGNMENT_CENTER, DWRITE_TEXT_ALIGNMENT_CENTER, DWriteCreateFactory,
    IDWriteFactory,
};
use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM;
use windows::Win32::Graphics::Gdi::{
    BI_RGB, BITMAPINFO, BITMAPINFOHEADER, CreateCompatibleDC, CreateDIBSection, DIB_RGB_COLORS, DeleteDC, DeleteObject,
    GdiFlush, HBITMAP, HDC, InvalidateRect, SelectObject,
};
use windows::Win32::UI::Controls::{
    HIMAGELIST, ILC_COLOR32, ILD_NORMAL, ImageList_Add, ImageList_Create, ImageList_Destroy, ImageList_Draw,
    TVM_SETIMAGELIST, TVSIL_NORMAL,
};
use windows::Win32::UI::HiDpi::{GetDpiForWindow, GetSystemMetricsForDpi};
use windows::Win32::UI::WindowsAndMessaging::SM_CXSMICON;
use windows::core::w;

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

/// Which places a tree's images have, a `side` square each, left to right: the colour's, then
/// the emoji's. Its rows share them, so their text lines up; a place no row fills is not there,
/// so a tree without emoji has no gap for one.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Places {
    pub colour: bool,
    pub emoji: bool,
}

impl Places {
    fn count(self) -> i32 {
        i32::from(self.colour) + i32::from(self.emoji)
    }
}

/// An image of `places`, `side` pixels tall, holding a filled circle in `colour` in the colour's
/// place and `emoji` in the emoji's (none: clear), with its alpha premultiplied as an image list
/// draws a 32-bit image.
fn bitmap(side: i32, places: Places, colour: Option<COLORREF>, emoji: Option<&str>) -> HBITMAP {
    let width = side * places.count().max(1);
    let info = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: width,
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
    let count = (width * side) as usize;
    let pixels = unsafe { std::slice::from_raw_parts_mut(bits.cast::<u32>(), count) };
    pixels.fill(0);
    if let Some(COLORREF(value)) = colour.filter(|_| places.colour) {
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
                pixels[(y * width + x) as usize] = alpha << 24 | pre(r) << 16 | pre(g) << 8 | pre(b);
            }
        }
    }
    if let Some(emoji) = emoji.filter(|emoji| places.emoji && !emoji.is_empty()) {
        let left = if places.colour { side } else { 0 };
        // One not drawn is only a mark missing: the row's text is what is read either way.
        let _ = draw_emoji(bitmap, left, side, emoji);
    }
    bitmap
}

thread_local! {
    /// Direct2D's and DirectWrite's factories, made once, on the window's thread.
    static FACTORIES: RefCell<Option<(ID2D1Factory, IDWriteFactory)>> = const { RefCell::new(None) };
}

/// The factories, made the first time they are wanted.
fn factories() -> windows::core::Result<(ID2D1Factory, IDWriteFactory)> {
    FACTORIES.with(|made| {
        if let Some(both) = made.borrow().clone() {
            return Ok(both);
        }
        let d2d: ID2D1Factory = unsafe { D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, None)? };
        let dwrite: IDWriteFactory = unsafe { DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED)? };
        *made.borrow_mut() = Some((d2d.clone(), dwrite.clone()));
        Ok((d2d, dwrite))
    })
}

/// Draws `emoji` into `bitmap`'s `side` square at `left`, in colour where the font has it:
/// Direct2D drawing into the bitmap through a device context, keeping its alpha, which GDI's
/// own text would lose, and its colour, which GDI's text never has.
fn draw_emoji(bitmap: HBITMAP, left: i32, side: i32, emoji: &str) -> windows::core::Result<()> {
    let (d2d, dwrite) = factories()?;
    let properties = D2D1_RENDER_TARGET_PROPERTIES {
        r#type: D2D1_RENDER_TARGET_TYPE_DEFAULT,
        pixelFormat: D2D1_PIXEL_FORMAT { format: DXGI_FORMAT_B8G8R8A8_UNORM, alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED },
        // A pixel to a unit: the side is in pixels already.
        dpiX: 96.0,
        dpiY: 96.0,
        usage: D2D1_RENDER_TARGET_USAGE_NONE,
        minLevel: D2D1_FEATURE_LEVEL_DEFAULT,
    };
    let size = side as f32;
    let format = unsafe {
        dwrite.CreateTextFormat(
            w!("Segoe UI Emoji"),
            None,
            DWRITE_FONT_WEIGHT_NORMAL,
            DWRITE_FONT_STYLE_NORMAL,
            DWRITE_FONT_STRETCH_NORMAL,
            size * 0.75,
            w!(""),
        )?
    };
    unsafe {
        format.SetTextAlignment(DWRITE_TEXT_ALIGNMENT_CENTER)?;
        format.SetParagraphAlignment(DWRITE_PARAGRAPH_ALIGNMENT_CENTER)?;
    }
    let wide: Vec<u16> = emoji.encode_utf16().collect();
    unsafe {
        let dc = CreateCompatibleDC(None);
        let old = SelectObject(dc, bitmap.into());
        let drawn = (|| -> windows::core::Result<()> {
            let target = d2d.CreateDCRenderTarget(&properties)?;
            let area = RECT { left, top: 0, right: left + side, bottom: side };
            target.BindDC(dc, &area)?;
            target.BeginDraw();
            target.SetTextAntialiasMode(D2D1_TEXT_ANTIALIAS_MODE_GRAYSCALE);
            target.Clear(Some(&D2D1_COLOR_F { r: 0.0, g: 0.0, b: 0.0, a: 0.0 }));
            // A glyph without colour takes the text's colour in the appearance now.
            let ink = if mode() == Some(true) { 1.0 } else { 0.0 };
            let brush = target.CreateSolidColorBrush(&D2D1_COLOR_F { r: ink, g: ink, b: ink, a: 1.0 }, None)?;
            let layout = D2D_RECT_F { left: 0.0, top: 0.0, right: size, bottom: size };
            target.DrawText(
                &wide,
                &format,
                &layout,
                &brush,
                D2D1_DRAW_TEXT_OPTIONS_ENABLE_COLOR_FONT | D2D1_DRAW_TEXT_OPTIONS_CLIP,
                DWRITE_MEASURING_MODE_NATURAL,
            );
            target.EndDraw(None, None)
        })();
        let _ = GdiFlush();
        SelectObject(dc, old);
        let _ = DeleteDC(dc);
        drawn
    }
}

/// What one image shows: a colour, by its swatch, and an emoji.
type Mark = (Option<Swatch>, Option<String>);

/// A tree's marks: an image list of a clear image and each colour and emoji its rows have shown,
/// given to the tree only while a row has one, so a tree without any has no gap before its text.
pub struct Marks {
    hwnd: HWND,
    list: Cell<HIMAGELIST>,
    /// The marks the list holds, the image after the clear one each.
    held: RefCell<Vec<Mark>>,
    /// What the list was drawn for: its side, the appearance, and its places.
    drawn: Cell<(i32, Option<bool>, Places)>,
    /// Which places the rows fill, as last drawn.
    wanted: Cell<Places>,
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
            drawn: Cell::new((0, None, Places::default())),
            wanted: Cell::new(Places::default()),
        });
        EVERY.with(|every| every.borrow_mut().push(Rc::downgrade(&marks)));
        marks
    }

    /// The image a row shows: the clear one for none. Kept under high contrast too, where the
    /// tree has no image list to draw it from, so the marks come back with the list.
    pub fn image(&self, swatch: Option<&Swatch>, emoji: Option<&str>) -> i32 {
        let emoji = emoji.filter(|emoji| !emoji.is_empty());
        if swatch.is_none() && emoji.is_none() {
            return 0;
        }
        let same = |held: &Mark| held.0.as_ref().map(|s| &s.name) == swatch.map(|s| &s.name) && held.1.as_deref() == emoji;
        let known = self.held.borrow().iter().position(same);
        let index = known.unwrap_or_else(|| {
            self.held.borrow_mut().push((swatch.cloned(), emoji.map(str::to_owned)));
            self.held.borrow().len() - 1
        });
        if known.is_none() && !self.list.get().is_invalid() {
            let (side, _, places) = self.drawn.get();
            let image = bitmap(side, places, swatch.and_then(colour), emoji);
            unsafe {
                ImageList_Add(self.list.get(), image, None);
                let _ = DeleteObject(image.into());
            }
        }
        index as i32 + 1
    }

    /// Gives the tree its marks while a row fills one of `places`, drawn for its DPI and the
    /// appearance now, and takes them away otherwise.
    pub fn show(&self, places: Places) {
        self.wanted.set(places);
        let now = (side(self.hwnd), mode(), places);
        let wanted = places.count() > 0 && now.1.is_some();
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
        let (side, _, places) = now;
        let made = unsafe { ImageList_Create(side * places.count(), side, ILC_COLOR32, 1 + self.held.borrow().len() as i32, 4) };
        let clear: Mark = (None, None);
        for (swatch, emoji) in std::iter::once(&clear).chain(self.held.borrow().iter()) {
            let image = bitmap(side, places, swatch.as_ref().and_then(colour), emoji.as_deref());
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
        let image = bitmap(side, Places { colour: true, emoji: false }, Some(colour), None);
        ImageList_Add(list, image, None);
        let _ = DeleteObject(image.into());
        let top = at.top + (at.bottom - at.top - side) / 2;
        let _ = ImageList_Draw(list, 0, dc, at.left, top, ILD_NORMAL);
        let _ = ImageList_Destroy(Some(list));
    }
    side
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows::Win32::Graphics::Gdi::{DIBSECTION, GetObjectW};

    /// The image's pixels, premultiplied BGRA, a row at a time.
    fn pixels(bitmap: HBITMAP) -> Vec<u32> {
        let mut section = DIBSECTION::default();
        let read = unsafe { GetObjectW(bitmap.into(), size_of::<DIBSECTION>() as i32, Some((&raw mut section).cast())) };
        assert!(read > 0, "the image is a section of its own");
        let count = (section.dsBm.bmWidth * section.dsBm.bmHeight) as usize;
        unsafe { std::slice::from_raw_parts(section.dsBm.bmBits.cast::<u32>(), count) }.to_vec()
    }

    #[test]
    fn an_emoji_is_drawn_in_colour_in_its_own_place_after_the_colour_and_a_row_without_either_is_clear() {
        let side = 32;
        let red = COLORREF(0x0000_00FF);
        let image = bitmap(side, Places { colour: true, emoji: true }, Some(red), Some("🍎"));
        let drawn = pixels(image);
        unsafe {
            let _ = DeleteObject(image.into());
        }
        let (colour, emoji): (Vec<u32>, Vec<u32>) = drawn
            .chunks(2 * side as usize)
            .flat_map(|row| row.iter().enumerate())
            .fold((Vec::new(), Vec::new()), |(mut colour, mut emoji), (x, pixel)| {
                if x < side as usize { colour.push(*pixel) } else { emoji.push(*pixel) }
                (colour, emoji)
            });
        assert!(colour.iter().any(|pixel| pixel >> 24 == 0xFF), "the colour's circle is in the first place");
        assert!(emoji.iter().any(|pixel| pixel >> 24 > 0), "the emoji is drawn in the second");
        let channels = |pixel: &u32| ((pixel >> 16) & 0xFF, (pixel >> 8) & 0xFF, pixel & 0xFF);
        assert!(emoji.iter().map(channels).any(|(r, g, b)| r != g || g != b), "in colour, not the text's grey");
        let clear = bitmap(side, Places { colour: false, emoji: true }, None, None);
        assert!(pixels(clear).iter().all(|pixel| *pixel == 0), "a row without either is clear");
        unsafe {
            let _ = DeleteObject(clear.into());
        }
    }
}
