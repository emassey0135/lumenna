//! The toolbar of the screen's own actions (`ViewModel::screen_actions`), above the list: what
//! the place shown offers beside its rows (New Task, Filter Tasks, the day's moves, Undo, Redo),
//! in the core's order and titles, as the Mac's toolbar and the web's buttons show them.
//!
//! A stock `ToolbarWindow32`, so a screen reader reads it as one toolbar of buttons, each named
//! by its text. It is one Tab stop, as File Explorer's command bar is: the arrow keys, Home and
//! End move through its buttons and Enter or Space presses the one in hand. The dialog manager,
//! which runs over the whole window, would take those keys for itself, so the toolbar asks for
//! them (`WM_GETDLGCODE`) and moves its hot item itself, telling screen readers which button has
//! focus. A button is run as the menu runs the same command (`App::run_command`): `Key(id)`, or
//! the window's own for the filter field and the keyboard help.

use std::cell::RefCell;

use lumenna_app::ScreenAction;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::Controls::NMHDR;
use windows::Win32::UI::Accessibility::NotifyWinEvent;
use windows::Win32::UI::Controls::{
    BTNS_AUTOSIZE, BTNS_BUTTON, BTNS_SHOWTEXT, CCS_NODIVIDER, CCS_NOPARENTALIGN, CCS_NORESIZE, ILC_COLOR32, ImageList_Create, TB_ADDBUTTONSW,
    TB_BUTTONCOUNT, TB_BUTTONSTRUCTSIZE, TB_DELETEBUTTON, TB_ENABLEBUTTON, TB_GETHOTITEM, TB_GETMAXSIZE, TB_ISBUTTONENABLED,
    TB_SETHOTITEM, TB_SETIMAGELIST, TBBUTTON, TBSTATE_ENABLED, TBSTYLE_FLAT, TBSTYLE_LIST, TBSTYLE_TRANSPARENT, TOOLBARCLASSNAMEW,
    CDDS_ITEMPREPAINT, CDDS_PREPAINT, CDRF_DODEFAULT, CDRF_NOTIFYITEMDRAW, NM_CUSTOMDRAW, NMTBCUSTOMDRAW,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{VK_DOWN, VK_END, VK_HOME, VK_LEFT, VK_RETURN, VK_RIGHT, VK_SPACE, VK_UP};
use windows::Win32::UI::Shell::{DefSubclassProc, SetWindowSubclass};
use windows::Win32::UI::WindowsAndMessaging::{
    DLGC_WANTARROWS, DLGC_WANTMESSAGE, EVENT_OBJECT_FOCUS, GetParent, MSG, OBJID_CLIENT, SendMessageW, WM_CHAR, WM_COMMAND,
    WM_GETDLGCODE, WM_KEYDOWN, WM_KILLFOCUS, WM_SETFOCUS, WS_TABSTOP,
};

use super::a11y;
use super::controls;

/// The command number of the toolbar's first button; each next is one more. Clear of the menu
/// bar's numbers and of 1 and 2 (`IDOK`, `IDCANCEL`).
pub const FIRST: u16 = 0x6000;

/// The toolbar, and what it shows.
pub struct Toolbar {
    pub hwnd: HWND,
    /// The actions shown, in order: button `FIRST + i` is the `i`th.
    shown: RefCell<Vec<ScreenAction>>,
    /// Each button's text, kept for as long as the button shows it.
    texts: RefCell<Vec<Vec<u16>>>,
}

impl Toolbar {
    /// An empty toolbar in `parent`, named as the core names a screen's own actions.
    pub fn create(parent: HWND) -> Self {
        let name = &super::words().screen_actions;
        // Transparent, so the window behind draws its background, dark or light.
        let style = WS_TABSTOP.0 | TBSTYLE_FLAT | TBSTYLE_LIST | TBSTYLE_TRANSPARENT | (CCS_NORESIZE | CCS_NOPARENTALIGN | CCS_NODIVIDER) as u32;
        let hwnd = controls::create(parent, TOOLBARCLASSNAMEW, name, style, 0, 0);
        controls::send(hwnd, TB_BUTTONSTRUCTSIZE, size_of::<TBBUTTON>(), 0);
        // Text alone, but over an image list of one blank pixel, each button naming its first
        // image, which draws nothing: a toolbar with no images, or a button with none
        // (`I_IMAGENONE`), is read by Windows' accessibility proxy as a menu bar of menu items,
        // and a screen reader would say so. With images it is a toolbar of buttons.
        let images = unsafe { ImageList_Create(1, 1, ILC_COLOR32, 0, 1) };
        controls::send(hwnd, TB_SETIMAGELIST, 0, images.0 as isize);
        a11y::set_name(hwnd, name);
        unsafe {
            let _ = SetWindowSubclass(hwnd, Some(procedure), 1, 0);
        }
        Self { hwnd, shown: RefCell::new(Vec::new()), texts: RefCell::new(Vec::new()) }
    }

    /// Shows `actions`: the buttons made again only when what they are changes, else each
    /// enabled or not as the app says, so a turn that changes nothing leaves focus alone. Whether the
    /// buttons were made again, which may change how wide and tall the toolbar is.
    pub fn draw(&self, actions: &[ScreenAction]) -> bool {
        let same = {
            let shown = self.shown.borrow();
            shown.len() == actions.len() && shown.iter().zip(actions).all(|(was, now)| was.id == now.id && was.title == now.title)
        };
        if same {
            for (at, action) in actions.iter().enumerate() {
                let was = self.shown.borrow()[at].enabled;
                if was != action.enabled {
                    self.enable(at, action.enabled);
                }
            }
            *self.shown.borrow_mut() = actions.to_vec();
            return false;
        }
        let hot = self.hot();
        while controls::send(self.hwnd, TB_BUTTONCOUNT, 0, 0) > 0 {
            controls::send(self.hwnd, TB_DELETEBUTTON, 0, 0);
        }
        let texts: Vec<Vec<u16>> = actions.iter().map(|action| text(action).encode_utf16().chain([0]).collect()).collect();
        let buttons: Vec<TBBUTTON> = actions
            .iter()
            .zip(&texts)
            .enumerate()
            .map(|(at, (action, text))| TBBUTTON {
                // The blank image: see `create`.
                iBitmap: 0,
                idCommand: i32::from(FIRST) + at as i32,
                fsState: if action.enabled { TBSTATE_ENABLED as u8 } else { 0 },
                fsStyle: (BTNS_BUTTON | BTNS_AUTOSIZE | BTNS_SHOWTEXT) as u8,
                iString: text.as_ptr() as isize,
                ..Default::default()
            })
            .collect();
        controls::send(self.hwnd, TB_ADDBUTTONSW, buttons.len(), buttons.as_ptr() as isize);
        *self.texts.borrow_mut() = texts;
        *self.shown.borrow_mut() = actions.to_vec();
        // Focus stays in the toolbar on a button that is there.
        if controls::focused() == self.hwnd {
            self.set_hot(hot.min(actions.len().saturating_sub(1) as i32), 1);
        }
        true
    }

    /// The action of command number `id`, if it is one of the toolbar's.
    pub fn action(&self, id: u16) -> Option<ScreenAction> {
        let at = usize::from(id.checked_sub(FIRST)?);
        self.shown.borrow().get(at).cloned()
    }

    /// How tall it is, its buttons as they are now drawn.
    pub fn height(&self, minimum: i32) -> i32 {
        let mut size = windows::Win32::Foundation::SIZE::default();
        controls::send(self.hwnd, TB_GETMAXSIZE, 0, &raw mut size as isize);
        size.cy.max(minimum)
    }

    fn enable(&self, at: usize, enabled: bool) {
        // Never take away the button in hand while the toolbar has focus: move on first.
        if !enabled && controls::focused() == self.hwnd && self.hot() == at as i32 {
            self.step(1);
        }
        controls::send(self.hwnd, TB_ENABLEBUTTON, usize::from(FIRST) + at, isize::from(enabled));
    }

    fn hot(&self) -> i32 {
        controls::send(self.hwnd, TB_GETHOTITEM, 0, 0) as i32
    }

    fn count(&self) -> i32 {
        controls::send(self.hwnd, TB_BUTTONCOUNT, 0, 0) as i32
    }

    fn enabled(&self, at: i32) -> bool {
        controls::send(self.hwnd, TB_ISBUTTONENABLED, usize::from(FIRST) + at as usize, 0) != 0
    }

    /// Puts the button at `at` in hand, or the next enabled one going `step`'s way, and tells a
    /// screen reader it has focus.
    fn set_hot(&self, at: i32, step: i32) {
        let count = self.count();
        if count == 0 {
            return;
        }
        let Some(found) = (0..count).map(|offset| (at + offset * step).rem_euclid(count)).find(|&index| self.enabled(index)) else {
            return;
        };
        controls::send(self.hwnd, TB_SETHOTITEM, found as usize, 0);
        unsafe { NotifyWinEvent(EVENT_OBJECT_FOCUS, self.hwnd, OBJID_CLIENT.0, found + 1) };
    }

    /// Moves the button in hand by `step`, wrapping round.
    fn step(&self, step: i32) {
        let hot = self.hot();
        let from = if hot < 0 { if step > 0 { -1 } else { 0 } } else { hot };
        self.set_hot(from + step, step);
    }

    /// Presses the button in hand: its command, to the window, as a click sends it.
    fn press(&self) {
        let hot = self.hot();
        if hot < 0 || !self.enabled(hot) {
            return;
        }
        unsafe {
            if let Ok(parent) = GetParent(self.hwnd) {
                SendMessageW(parent, WM_COMMAND, Some(WPARAM(usize::from(FIRST) + hot as usize)), Some(LPARAM(self.hwnd.0 as isize)));
            }
        }
    }
}

/// A button's text: the core's title, Title Case, with Windows' ellipsis where it asks first.
fn text(action: &ScreenAction) -> String {
    if action.asks { format!("{}...", action.title) } else { action.title.clone() }
}

/// The toolbar's keys: the arrows, Home and End move through it, Enter and Space press.
unsafe extern "system" fn procedure(hwnd: HWND, message: u32, wparam: WPARAM, lparam: LPARAM, _id: usize, _data: usize) -> LRESULT {
    match message {
        WM_GETDLGCODE => {
            let mut code = DLGC_WANTARROWS as isize;
            let msg = lparam.0 as *const MSG;
            if !msg.is_null() {
                let msg = unsafe { &*msg };
                if msg.message == WM_KEYDOWN && matches!(msg.wParam.0 as u16, k if k == VK_RETURN.0 || k == VK_SPACE.0 || k == VK_HOME.0 || k == VK_END.0) {
                    code |= DLGC_WANTMESSAGE as isize;
                }
            }
            LRESULT(code)
        }
        WM_KEYDOWN => {
            let key = wparam.0 as u16;
            let ours = [VK_LEFT, VK_UP, VK_RIGHT, VK_DOWN, VK_HOME, VK_END, VK_RETURN, VK_SPACE].iter().any(|k| k.0 == key);
            if !ours {
                return unsafe { DefSubclassProc(hwnd, message, wparam, lparam) };
            }
            // Pressed after the key is handled: a button's command can rebuild the toolbar.
            let mut pressed = false;
            super::app::with(|app| {
                let toolbar = &app.toolbar;
                match key {
                    k if k == VK_LEFT.0 || k == VK_UP.0 => toolbar.step(-1),
                    k if k == VK_RIGHT.0 || k == VK_DOWN.0 => toolbar.step(1),
                    k if k == VK_HOME.0 => toolbar.set_hot(0, 1),
                    k if k == VK_END.0 => toolbar.set_hot(toolbar.count() - 1, -1),
                    _ => pressed = true,
                }
            });
            if pressed {
                super::app::with(|app| app.defer(|app| app.toolbar.press()));
            }
            LRESULT(0)
        }
        // Enter's and Space's characters, pressed already.
        WM_CHAR if wparam.0 == usize::from(b'\r') || wparam.0 == usize::from(b' ') => LRESULT(0),
        WM_SETFOCUS => {
            let result = unsafe { DefSubclassProc(hwnd, message, wparam, lparam) };
            super::app::with(|app| {
                let hot = app.toolbar.hot();
                app.toolbar.set_hot(hot.max(0), 1);
            });
            result
        }
        WM_KILLFOCUS => {
            controls::send(hwnd, TB_SETHOTITEM, usize::MAX, 0);
            unsafe { DefSubclassProc(hwnd, message, wparam, lparam) }
        }
        _ => unsafe { DefSubclassProc(hwnd, message, wparam, lparam) },
    }
}

/// What the toolbar asks of the window as it draws: while dark, its buttons' text in the dark
/// theme's colour, which a stock toolbar does not take from the window.
pub fn notify(header: &NMHDR, lparam: LPARAM) -> Option<isize> {
    if header.code != NM_CUSTOMDRAW {
        return None;
    }
    let (_, text) = super::dark::field()?;
    let draw = unsafe { &mut *(lparam.0 as *mut NMTBCUSTOMDRAW) };
    match draw.nmcd.dwDrawStage {
        CDDS_PREPAINT => Some(CDRF_NOTIFYITEMDRAW as isize),
        CDDS_ITEMPREPAINT => {
            draw.clrText = text;
            Some(CDRF_DODEFAULT as isize)
        }
        _ => None,
    }
}
