//! Modal dialogs and property sheets from templates built in memory.
//!
//! The dialog manager is Windows' own: Tab and Shift-Tab, Enter for the default button,
//! Escape to cancel, Alt and a mnemonic to reach a field, and a static label naming the field
//! after it — all of which screen readers have handled for thirty years. Building the
//! template here rather than in a `.rc` file keeps the app free of a resource compiler, and
//! keeps each dialog's layout beside the code that runs it.

use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::UI::Controls::{
    PROPSHEETHEADERW_V2, PROPSHEETHEADERW_V2_1, PROPSHEETHEADERW_V2_2, PROPSHEETPAGEW, PROPSHEETPAGEW_0, PSCB_INITIALIZED,
    PSH_NOAPPLYNOW, PSH_NOCONTEXTHELP, PSH_PROPSHEETPAGE, PSH_USECALLBACK, PSP_DLGINDIRECT, PropertySheetW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    DLGTEMPLATE, DS_CENTER, DS_MODALFRAME, DS_SETFONT, DWLP_MSGRESULT, DialogBoxIndirectParamW, EndDialog,
    GWLP_USERDATA, GetDlgItem, GetWindowLongPtrW, IDCANCEL, IDOK, SetWindowLongPtrW,
    WINDOW_LONG_PTR_INDEX, WM_COMMAND, WM_CTLCOLORBTN, WM_CTLCOLORDLG, WM_CTLCOLOREDIT, WM_CTLCOLORLISTBOX,
    WM_CTLCOLORSTATIC, WM_INITDIALOG, WM_VKEYTOITEM, WS_CAPTION, WS_CHILD, WS_DISABLED, WS_POPUP, WS_SYSMENU,
    WS_VISIBLE,
};

use windows::core::{HSTRING, PCWSTR};

use super::{controls, dark, font};

/// The stock classes a dialog item can be.
#[derive(Clone, Copy)]
pub enum Class {
    Button = 0x80,
    Edit = 0x81,
    Static = 0x82,
    ListBox = 0x83,
    ComboBox = 0x85,
}

/// A dialog template: the dialog, then its items in tab order. Sizes are dialog units, which
/// scale with the dialog's font.
pub struct Template {
    words: Vec<u16>,
    count: u16,
    /// Where the font's size sits, set when the template is fitted to the screen.
    font_at: usize,
    face: String,
    points: u16,
}

/// Where the item count sits in the header, to be filled in last.
const COUNT_AT: usize = 4;
/// Where the width and height sit, after the count and the position.
const SIZE_AT: usize = 7;
/// What a property sheet adds around a page, in dialog units: its margins, the tab strip,
/// and the row of buttons.
const SHEET_AROUND: (i16, i16) = (14, 52);

impl Template {
    pub fn new(title: &str, width: i16, height: i16) -> Self {
        let mut template = Self { words: Vec::new(), count: 0, font_at: 0, face: String::new(), points: 0 };
        let style = WS_POPUP.0 | WS_CAPTION.0 | WS_SYSMENU.0 | (DS_MODALFRAME | DS_SETFONT | DS_CENTER) as u32;
        template.dword(style);
        template.dword(0);
        template.words.push(0); // the item count
        for value in [0, 0, width, height] {
            template.words.push(value as u16);
        }
        template.words.push(0); // no menu
        template.words.push(0); // the standard dialog class
        template.string(title);
        template.font();
        template
    }

    /// A page of a property sheet: a child the sheet places, titled on its tab.
    pub fn page(title: &str, width: i16, height: i16) -> Self {
        let mut template = Self { words: Vec::new(), count: 0, font_at: 0, face: String::new(), points: 0 };
        template.dword(WS_CHILD.0 | WS_DISABLED.0 | WS_CAPTION.0 | DS_SETFONT as u32);
        template.dword(0);
        template.words.push(0);
        for value in [0, 0, width, height] {
            template.words.push(value as u16);
        }
        template.words.push(0);
        template.words.push(0);
        template.string(title);
        template.font();
        template
    }

    /// The system's message font, so a dialog grows with Text size as well as DPI, and its
    /// layout, in dialog units, grows with it.
    fn font(&mut self) {
        let font = font::dialog_font();
        self.font_at = self.words.len();
        self.words.push(font.points);
        self.string(&font.face);
        self.face = font.face;
        self.points = font.points;
    }

    /// The largest size up to the message font's at which this fits the screen of `owner`,
    /// with `extra` dialog units around it.
    fn fitting(&self, owner: Option<HWND>, extra: (i16, i16)) -> u16 {
        let (width, height) = (self.words[SIZE_AT] as i16, self.words[SIZE_AT + 1] as i16);
        font::fitting_points(owner, &self.face, self.points, width, height, extra)
    }

    /// Adds an item. `style` gets `WS_CHILD | WS_VISIBLE` added.
    #[allow(clippy::too_many_arguments)]
    pub fn item(mut self, class: Class, text: &str, id: u16, style: u32, x: i16, y: i16, width: i16, height: i16) -> Self {
        self.start_item(style, [x, y, width, height], id);
        self.words.push(0xFFFF);
        self.words.push(class as u16);
        self.end_item(text);
        self
    }

    /// Adds an item of a class named rather than one of the stock six, such as the hotkey
    /// control.
    #[allow(clippy::too_many_arguments)]
    pub fn named(mut self, class: &str, text: &str, id: u16, style: u32, x: i16, y: i16, width: i16, height: i16) -> Self {
        self.start_item(style, [x, y, width, height], id);
        self.string(class);
        self.end_item(text);
        self
    }

    fn start_item(&mut self, style: u32, place: [i16; 4], id: u16) {
        if self.words.len() % 2 == 1 {
            self.words.push(0); // each item starts on a four-byte boundary
        }
        self.dword(WS_CHILD.0 | WS_VISIBLE.0 | style);
        self.dword(0);
        for value in place {
            self.words.push(value as u16);
        }
        self.words.push(id);
    }

    fn end_item(&mut self, text: &str) {
        self.string(text);
        self.words.push(0); // no creation data
        self.count += 1;
    }

    /// Sets the dialog's size, for one whose height depends on which items it has.
    pub fn resize(mut self, width: i16, height: i16) -> Self {
        self.words[SIZE_AT] = width as u16;
        self.words[SIZE_AT + 1] = height as u16;
        self
    }

    fn dword(&mut self, value: u32) {
        self.words.push((value & 0xFFFF) as u16);
        self.words.push((value >> 16) as u16);
    }

    fn string(&mut self, text: &str) {
        self.words.extend(text.encode_utf16());
        self.words.push(0);
    }

    /// The template, written in a font of `points`, four-byte aligned as
    /// `DialogBoxIndirectParamW` requires.
    fn build(&self, points: u16) -> Vec<u32> {
        let mut words = self.words.clone();
        words[COUNT_AT] = self.count;
        words[self.font_at] = points;
        if words.len() % 2 == 1 {
            words.push(0);
        }
        words.chunks(2).map(|pair| u32::from(pair[0]) | (u32::from(pair[1]) << 16)).collect()
    }
}

/// What a dialog, or a page of a property sheet, does.
pub trait Dialog {
    fn template(&self) -> Template;

    /// Fills the dialog in once its controls exist. Returns whether it placed the focus
    /// itself; otherwise the first field gets it.
    fn init(&self, _hwnd: HWND) -> bool {
        false
    }

    /// A control's command. `Some` closes the dialog with that result; a page cannot close
    /// its sheet, so there it is ignored.
    fn command(&self, _hwnd: HWND, id: u16, _code: u16) -> Option<isize> {
        match i32::from(id) {
            id if id == IDOK.0 => Some(1),
            id if id == IDCANCEL.0 => Some(0),
            _ => None,
        }
    }

    /// Any other message — a notification, or a result posted from another thread. `Some`
    /// is the message's result.
    fn message(&self, _hwnd: HWND, _message: u32, _wparam: WPARAM, _lparam: LPARAM) -> Option<isize> {
        None
    }
}

/// Runs a dialog until it closes, and returns what it closed with.
pub fn run(owner: Option<HWND>, dialog: &dyn Dialog) -> isize {
    let template = dialog.template();
    let template = template.build(template.fitting(owner, (0, 0)));
    // The dialog procedure gets a pointer to this reference, which lives until this returns,
    // and the dialog cannot outlive the call.
    let reference: &dyn Dialog = dialog;
    let pointer = &raw const reference;
    unsafe {
        DialogBoxIndirectParamW(
            Some(controls::instance()),
            template.as_ptr().cast::<DLGTEMPLATE>(),
            owner,
            Some(procedure),
            LPARAM(pointer as isize),
        )
    }
}

/// Runs a property sheet of `pages` until it is closed, starting on page `start`.
///
/// The sheet has one button, Close: like the Mac's Settings, every page applies a change as
/// it is made, so there is nothing for OK to apply or Cancel to take back. Escape closes it.
pub fn sheet(owner: HWND, title: &str, pages: &[&dyn Dialog], start: usize) {
    let templates: Vec<Template> = pages.iter().map(|page| page.template()).collect();
    // One size for every page, the largest at which each fits with the sheet's tabs, margins
    // and buttons around it.
    let points = templates.iter().map(|template| template.fitting(Some(owner), SHEET_AROUND)).min().unwrap_or(9);
    let templates: Vec<Vec<u32>> = templates.iter().map(|template| template.build(points)).collect();
    // As for `run`: each page's procedure gets a pointer to its reference here.
    let references: Vec<&dyn Dialog> = pages.to_vec();
    let mut descriptions: Vec<PROPSHEETPAGEW> = templates
        .iter()
        .zip(&references)
        .map(|(template, reference)| PROPSHEETPAGEW {
            dwSize: size_of::<PROPSHEETPAGEW>() as u32,
            dwFlags: PSP_DLGINDIRECT,
            hInstance: controls::instance(),
            Anonymous1: PROPSHEETPAGEW_0 { pResource: template.as_ptr().cast_mut().cast::<DLGTEMPLATE>() },
            pfnDlgProc: Some(page_procedure),
            lParam: LPARAM(std::ptr::from_ref(reference) as isize),
            ..Default::default()
        })
        .collect();
    let caption = HSTRING::from(title);
    let mut header = PROPSHEETHEADERW_V2 {
        dwSize: size_of::<PROPSHEETHEADERW_V2>() as u32,
        dwFlags: PSH_PROPSHEETPAGE | PSH_NOAPPLYNOW | PSH_NOCONTEXTHELP | PSH_USECALLBACK,
        hwndParent: owner,
        hInstance: controls::instance(),
        pszCaption: PCWSTR(caption.as_ptr()),
        nPages: descriptions.len() as u32,
        Anonymous2: PROPSHEETHEADERW_V2_1 { nStartPage: start as u32 },
        Anonymous3: PROPSHEETHEADERW_V2_2 { ppsp: descriptions.as_mut_ptr() },
        pfnCallback: Some(sheet_callback),
        ..Default::default()
    };
    unsafe {
        PropertySheetW(&mut header);
    }
}

/// Once the sheet exists: OK hidden, and Cancel called what it does, Close.
unsafe extern "system" fn sheet_callback(hwnd: HWND, message: u32, _lparam: LPARAM) -> i32 {
    if message == PSCB_INITIALIZED {
        // The sheet's frame is drawn by its own procedure, which knows no dark mode.
        unsafe {
            let _ = windows::Win32::UI::Shell::SetWindowSubclass(hwnd, Some(dark::sheet_colours), 1, 0);
        }
        dark::window(hwnd);
        controls::show(item(hwnd, IDOK.0 as u16), false);
        controls::set_text(item(hwnd, IDCANCEL.0 as u16), "Close");
    }
    0
}

/// A control in a dialog, by identifier.
pub fn item(hwnd: HWND, id: u16) -> HWND {
    unsafe { GetDlgItem(Some(hwnd), i32::from(id)).unwrap_or_default() }
}

unsafe extern "system" fn procedure(hwnd: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> isize {
    unsafe {
        if message == WM_INITDIALOG {
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, lparam.0);
        }
        dispatch(hwnd, message, wparam, lparam, false)
    }
}

/// A page: the same, but its reference arrives inside the page's description.
unsafe extern "system" fn page_procedure(hwnd: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> isize {
    unsafe {
        if message == WM_INITDIALOG {
            let page = &*(lparam.0 as *const PROPSHEETPAGEW);
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, page.lParam.0);
        }
        dispatch(hwnd, message, wparam, lparam, true)
    }
}

unsafe fn dispatch(hwnd: HWND, message: u32, wparam: WPARAM, lparam: LPARAM, page: bool) -> isize {
    unsafe {
        let pointer = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const &dyn Dialog;
        if pointer.is_null() {
            return 0;
        }
        let dialog = *pointer;
        if let WM_CTLCOLORSTATIC | WM_CTLCOLORBTN | WM_CTLCOLORDLG | WM_CTLCOLOREDIT | WM_CTLCOLORLISTBOX = message
            && let Some(brush) = dark::colour(message, wparam, lparam)
        {
            // A dialog procedure answers these with the brush itself.
            return brush.0;
        }
        match message {
            WM_INITDIALOG => {
                // A page's title bar is its sheet's.
                if page {
                    dark::controls_in(hwnd);
                } else {
                    dark::window(hwnd);
                }
                isize::from(!dialog.init(hwnd))
            }
            WM_COMMAND => {
                let id = controls::low_word(wparam.0);
                let code = controls::high_word(wparam.0);
                if let Some(result) = dialog.command(hwnd, id, code)
                    && !page
                {
                    let _ = EndDialog(hwnd, result);
                }
                1
            }
            _ => match dialog.message(hwnd, message, wparam, lparam) {
                // A few messages take a dialog procedure's own return as their answer.
                Some(result) if message == WM_VKEYTOITEM => result,
                Some(result) => {
                    // The rest — notifications above all — take it from DWLP_MSGRESULT.
                    SetWindowLongPtrW(hwnd, WINDOW_LONG_PTR_INDEX(DWLP_MSGRESULT as i32), result);
                    1
                }
                None => 0,
            },
        }
    }
}
