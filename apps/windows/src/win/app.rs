//! The main window: three panes and a status line, the menu bar, and the message loop.
//!
//! The panes are the places (a tree), the view for the place chosen, and the chosen task's
//! details, as Outlook and Explorer lay theirs out. **F6 and Shift+F6 move between them.**
//! Nothing provides that for free — the dialog manager does Tab, arrows, Escape and
//! mnemonics only — and without it a screen reader user reaches the details by tabbing
//! through everything in between.
//!
//! The status line along the bottom is a live region: what a change did is written there
//! and read out, after the row focus moved to.
//!
//! Closing the window hides it; the app stays resident, syncing, until Exit.

use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::rc::Rc;
use std::sync::Arc;

use lumenna_records::{ActionKind, Change, Result};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    COLOR_BTNFACE, CreateFontIndirectW, DeleteObject, GetDC, GetTextMetricsW, HBRUSH, HFONT, LOGFONTW,
    ReleaseDC, SelectObject, TEXTMETRICW,
};
use windows::Win32::System::Com::{COINIT_APARTMENTTHREADED, CoInitializeEx};
use windows::Win32::UI::Controls::{
    ICC_DATE_CLASSES, ICC_HOTKEY_CLASS, ICC_STANDARD_CLASSES, ICC_TAB_CLASSES, ICC_TREEVIEW_CLASSES, INITCOMMONCONTROLSEX,
    InitCommonControlsEx, LIM_LARGE, LIM_SMALL, LoadIconMetric, NM_RCLICK, NMHDR,
    WC_STATICW,
};
use windows::Win32::UI::HiDpi::GetDpiForWindow;
use windows::Win32::System::SystemServices::{SS_ENDELLIPSIS, SS_NOPREFIX};
use windows::Win32::UI::Controls::{EM_SETSEL, EM_UNDO};
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::{HSTRING, PCWSTR, w};

use lumenna_app::{Capitals, DayMove, DetailsEvent, Offered};
use lumenna_ffi::ShellEvent;

use super::asking;
use super::block_form;
use super::blocks::BlockList;
use super::clock::Formats;
use super::{pairing, proposal, reminders, swatch, system};
use super::shell::{Relay, WM_TURN};
use super::controls::{self, rect};
use super::core::{Poster, WM_EVENT, WM_SAY, said, taken};
use super::toast::{Answered, Toasts, WM_REMINDER};
use super::dark;
use super::day::DayView;
use super::font;
use super::help;
use super::detail::Detail;
use super::menu;
use super::prompts;
use super::quick_add;
use super::sidebar::Sidebar;
use super::actions;
use super::tasks::TaskList;
use super::settings::{self, Page};
use super::shortcuts;
use crate::shortcut::{self, Kind};
use super::tray::{self, WM_SHOW_RUNNING, WM_TRAY};
use super::view::{Metrics, View};
use super::{a11y, core::sentence};
use lumenna_records::places::{Place, SidebarGroup};
use crate::{profile, speech};

/// Runs a deferred action: something a notification asked for that changes the window it
/// came from, so it waits until that window has finished sending it.
const WM_DEFERRED: u32 = WM_APP + 3;

/// The app's icon, as build.rs embeds it.
pub const ICON: u16 = 1;

const TIMER_REFRESH: usize = 1;
/// Looks every 50 milliseconds whether the app is still, to ask the question it has open.
const TIMER_ASK: usize = 2;
/// Looks every 50 milliseconds whether the app is still, to write the field being typed in
/// where the app's value differs.
const TIMER_SETTLE: usize = 3;
/// Looks every 50 milliseconds whether the app is still, to do a way in from outside.
const TIMER_LAUNCH: usize = 4;

/// Something to do once the notification being handled has returned.
type Deferred = Box<dyn FnOnce(&App)>;

/// The icon chosen from the keyboard: `NIN_SELECT | NINF_KEY`, which the bindings lack.
const NIN_KEYSELECT: u32 = windows::Win32::UI::Shell::NIN_SELECT | 1;

/// A dialog's own status line, while the dialog is in front.
struct Live {
    status: HWND,
    /// Closed by the app this turn: what it says now waits for the dialog to go.
    closing: bool,
    after: Vec<String>,
}

/// The view in the middle pane.
#[derive(Clone)]
pub enum Content {
    Tasks(Rc<TaskList>),
    Day(Rc<DayView>),
    Blocks(Rc<BlockList>),
}

impl Content {
    fn view(&self) -> &dyn View {
        match self {
            Self::Tasks(view) => &**view,
            Self::Day(view) => &**view,
            Self::Blocks(view) => &**view,
        }
    }
}

pub struct App {
    /// The Crux app, over this profile's store: what every screen shows, and where it sends
    /// what the person does.
    pub crux: lumenna_ffi::App,
    /// The store as the app's own connection holds it, for what runs without the screens:
    /// reminders, backups.
    device: Arc<lumenna_ffi::Device>,
    /// Where the profile is.
    pub directory: std::path::PathBuf,
    /// The last turn's sequence and its landing count: focus moves only when the count does.
    seq: Cell<u32>,
    landing: Cell<u32>,
    /// Whether the app's open question is being asked now.
    asking: Cell<bool>,
    /// The last line typed into a question, asked again with it when refused.
    pub typed: RefCell<String>,
    /// The app's block form, while its dialog is open.
    block_form: RefCell<Option<Rc<block_form::Form>>>,
    /// The app's proposal screen, while its dialog is open.
    proposal: RefCell<Option<Rc<proposal::Screen>>>,
    /// The status lines of the dialogs open in front, the frontmost last: where what the app
    /// says goes.
    live: RefCell<Vec<Live>>,
    pub main: HWND,
    accelerators: HACCEL,
    font: Cell<HFONT>,
    /// The message font's height and face the font was made from, to tell whether a setting
    /// change changed it.
    font_made_from: Cell<(i32, [u16; 32])>,
    metrics: Cell<Metrics>,
    panes: [HWND; 3],
    status: HWND,
    pub sidebar: Sidebar,
    content: RefCell<Option<Content>>,
    pub detail: Detail,
    deferred: RefCell<VecDeque<Deferred>>,
    /// Where focus was when the window was last active, to put it back on return.
    last_focus: Cell<HWND>,
    /// The Settings sheet, while it is open.
    settings: RefCell<Option<Rc<settings::Sheet>>>,
    /// The pairing dialog, while it is open.
    pairing: RefCell<Option<Rc<pairing::Pairing>>>,
    icon: HICON,
    taskbar_created: u32,
    /// Windows' notifications, for reminders; `None` where it has none to give.
    toasts: Option<Toasts>,
    /// Whether this process gives this device's reminders: another may.
    reminders: Cell<bool>,
    /// A task's or block's reminder list, while its dialog is open.
    reminder_list: RefCell<Option<Rc<reminders::Reminders>>>,
    /// Quick add, while its dialog is open.
    quick_add: RefCell<Option<Rc<quick_add::QuickAdd>>>,
    /// Seconds since the app started, for what is done every few of them.
    ticks: Cell<u32>,
    /// A way in from outside (`--launch`, a Jump List's task) waiting for the app to be still.
    launching: RefCell<Option<String>>,
}

thread_local! {
    static APP: RefCell<Option<Rc<App>>> = const { RefCell::new(None) };
}

/// The app, if it has started. Cloned out, so no borrow is held while it runs — window
/// procedures are re-entered whenever a control sends a notification.
fn app() -> Option<Rc<App>> {
    APP.with(|app| app.borrow().clone())
}

pub fn run() {
    unsafe {
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        let controls = INITCOMMONCONTROLSEX {
            dwSize: size_of::<INITCOMMONCONTROLSEX>() as u32,
            dwICC: ICC_STANDARD_CLASSES | ICC_TREEVIEW_CLASSES | ICC_TAB_CLASSES | ICC_HOTKEY_CLASS | ICC_DATE_CLASSES,
        };
        let _ = InitCommonControlsEx(&controls);
    }
    let mut arguments = profile::arguments(std::env::args().skip(1));
    // A package's Jump List hands its arguments to Windows' activation, not the command line.
    if arguments.launch.is_none() {
        arguments.launch = profile::arguments(super::package::launched_with()).launch;
    }
    let Some(directory) = profile::directory(arguments.profile) else {
        prompts::fail(HWND::default(), "Lumenna cannot find a folder for its data. Set LUMENNA_PROFILE to one.");
        return;
    };
    let Some(_instance) = tray::claim(&directory, arguments.launch.as_deref()) else { return };
    let device = match lumenna_ffi::Device::new(directory.display().to_string()) {
        Ok(device) => device,
        Err(error) => {
            prompts::fail(HWND::default(), &format!("Lumenna could not open its data. {}", sentence(&error)));
            return;
        }
    };
    let Some(app) = App::create(device, directory.clone(), &tray::class_name(&directory)) else { return };
    APP.with(|slot| *slot.borrow_mut() = Some(Rc::clone(&app)));
    if arguments.no_shortcuts {
        shortcuts::leave_to_another_copy();
    }
    // Opened at sign-in: by the Run key with `--background`, or by a package's startup task.
    // A way in from outside is shown, whatever else it was started with.
    app.start(arguments.launch.is_none() && (arguments.background || super::package::started_at_sign_in()));
    if let Some(id) = arguments.launch {
        app.launch(id);
    }
    message_loop(&app);
    APP.with(|slot| slot.borrow_mut().take());
}

/// Every setting as the app holds it this moment, with what its field shows: empty until the
/// app has them.
pub fn current_settings() -> Vec<(lumenna_records::Setting, String)> {
    app()
        .and_then(|app| app.crux.view().settings)
        .map(|settings| settings.settings.into_iter().map(|shown| (shown.setting, shown.typed)).collect())
        .unwrap_or_default()
}

/// Runs `then` with the window, from a dialog that holds no reference to it.
pub fn with(then: impl FnOnce(&App)) {
    if let Some(app) = app() {
        then(&app);
    }
}

/// Sends what the person did to the app, from a dialog that holds no reference to the window.
pub fn send(event: ShellEvent) {
    if let Some(app) = app() {
        app.crux.send(event);
    }
}

fn message_loop(app: &App) {
    let mut message = MSG::default();
    unsafe {
        while GetMessageW(&mut message, None, 0, 0).0 > 0 {
            let ours = controls::within(app.main, message.hwnd);
            if ours && TranslateAcceleratorW(app.main, app.accelerators, &message) != 0 {
                continue;
            }
            // The dialog manager, over the whole window: Tab and Shift+Tab through every
            // pane, mnemonics, Enter and Escape.
            if ours && IsDialogMessageW(app.main, &message).as_bool() {
                continue;
            }
            let _ = TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    }
}

impl App {
    fn create(device: lumenna_ffi::Device, directory: std::path::PathBuf, class: &str) -> Option<Rc<Self>> {
        // Before any window, so menus are made in the mode.
        dark::refresh();
        let instance = controls::instance();
        // The app's own (build.rs), at the sizes this display wants: the small one for the title
        // bar and the notification area, the large for Alt+Tab.
        let load = |size| unsafe {
            LoadIconMetric(Some(instance), PCWSTR(ICON as usize as _), size)
                .or_else(|_| LoadIconW(None, IDI_APPLICATION))
                .unwrap_or_default()
        };
        let (icon, large) = (load(LIM_SMALL), load(LIM_LARGE));
        let class = HSTRING::from(class);
        unsafe {
            let cursor = LoadCursorW(None, IDC_ARROW).unwrap_or_default();
            let background = HBRUSH((COLOR_BTNFACE.0 + 1) as usize as _);
            let main_class = WNDCLASSEXW {
                cbSize: size_of::<WNDCLASSEXW>() as u32,
                lpfnWndProc: Some(main_procedure),
                hInstance: instance,
                hIcon: large,
                hIconSm: icon,
                hCursor: cursor,
                hbrBackground: background,
                lpszClassName: windows::core::PCWSTR(class.as_ptr()),
                ..Default::default()
            };
            RegisterClassExW(&main_class);
            let pane_class = WNDCLASSEXW {
                cbSize: size_of::<WNDCLASSEXW>() as u32,
                lpfnWndProc: Some(pane_procedure),
                hInstance: instance,
                hCursor: cursor,
                hbrBackground: background,
                lpszClassName: w!("LumennaPane"),
                ..Default::default()
            };
            RegisterClassExW(&pane_class);
        }
        let main = unsafe {
            CreateWindowExW(
                WS_EX_CONTROLPARENT,
                &class,
                w!("Lumenna"),
                WS_OVERLAPPEDWINDOW | WS_CLIPCHILDREN,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                1100,
                720,
                None,
                Some(menu::bar()),
                Some(instance),
                None,
            )
            .ok()?
        };
        fit_to_screen(main);
        let pane = |name: &str| {
            
            unsafe {
                CreateWindowExW(
                    WS_EX_CONTROLPARENT,
                    w!("LumennaPane"),
                    &HSTRING::from(name),
                    WS_CHILD | WS_VISIBLE | WS_CLIPCHILDREN,
                    0,
                    0,
                    0,
                    0,
                    Some(main),
                    None,
                    Some(instance),
                    None,
                )
                .unwrap_or_default()
            }
        };
        // In this order, which is also the order Tab goes through them.
        let panes = [pane("Places"), pane("List"), pane("Task details")];
        let status = controls::create(main, WC_STATICW, "", SS_NOPREFIX.0 | SS_ENDELLIPSIS.0, 0, 0);
        a11y::make_live(status);
        let sidebar = Sidebar::create(panes[0]);
        let detail = Detail::create(panes[2]);
        let relay = Arc::new(Relay(Poster::new(main)));
        let crux = match lumenna_ffi::App::new(&device, "windows".to_owned(), Arc::new(Formats), relay) {
            Ok(crux) => crux,
            Err(error) => {
                prompts::fail(main, &format!("Lumenna could not start. {}", sentence(&error)));
                return None;
            }
        };
        crux.send(ShellEvent::Clock(proposal::style()));
        // Title Case, as Windows writes its buttons and menus.
        crux.send(ShellEvent::Capitals(Capitals::Title));
        // How pairing's sentences call this device; it finds devices on its own network.
        crux.send(ShellEvent::PairingAs { called: "this PC".to_owned(), nearby: true });
        // Whether it opens at sign-in, the app's setting of keeping Lumenna running.
        crux.send(settings::background());
        // Resident, so in front for as long as it runs: the device's sync process once another
        // device is paired, which then answers the command surface at the profile's pipe, as
        // the daemon would, so Emacs and the BTSpeak app reach it.
        crux.serve_commands("Lumenna for Windows".to_owned(), system::computer_name());
        let app = Rc::new(Self {
            crux,
            device: Arc::new(device),
            directory,
            seq: Cell::new(0),
            landing: Cell::new(0),
            asking: Cell::new(false),
            typed: RefCell::new(String::new()),
            block_form: RefCell::new(None),
            proposal: RefCell::new(None),
            live: RefCell::new(Vec::new()),
            main,
            accelerators: menu::accelerators(),
            font: Cell::new(HFONT::default()),
            font_made_from: Cell::new((0, [0; 32])),
            metrics: Cell::new(Metrics { scale: 1.0, line: 16, field: 24, button: 26 }),
            panes,
            status,
            sidebar,
            content: RefCell::new(None),
            detail,
            deferred: RefCell::new(VecDeque::new()),
            last_focus: Cell::new(HWND::default()),
            settings: RefCell::new(None),
            pairing: RefCell::new(None),
            icon,
            taskbar_created: tray::taskbar_created(),
            toasts: Toasts::new(),
            reminders: Cell::new(false),
            reminder_list: RefCell::new(None),
            quick_add: RefCell::new(None),
            ticks: Cell::new(0),
            launching: RefCell::new(None),
        });
        app.update_font();
        dark::window(main);
        Some(app)
    }

    /// Shows the window on Today — unless started in the background, at sign-in, when it
    /// waits in the notification area — and starts what runs beside it.
    fn start(&self, background: bool) {
        self.go(Place::Today, true);
        unsafe {
            if !background {
                let _ = ShowWindow(self.main, SW_SHOWDEFAULT);
            }
            let _ = SetTimer(Some(self.main), TIMER_REFRESH, 1000, None);
        }
        tray::add_icon(self.main, self.icon);
        super::jump_list::publish(self.directory.clone());
        let taken = shortcuts::register_all(self.main);
        self.back_up_if_due();
        self.give_reminders();
        if !taken.is_empty() {
            self.say(&shortcut::taken(&taken, shortcuts::another_copy_open(self.main)));
        }
    }

    // -------------------------------------------------------------------------------------
    // Saying, failing, changing
    // -------------------------------------------------------------------------------------

    /// Writes a sentence on the status line, which reads it out: the proposal screen's own
    /// while it is open, since a dialog in front hides the window's.
    pub fn say(&self, text: &str) {
        if text.is_empty() {
            return;
        }
        let status = {
            let mut live = self.live.borrow_mut();
            match live.last_mut() {
                // Closing: said where it goes next, once the dialog has gone.
                Some(top) if top.closing => return top.after.push(text.to_owned()),
                Some(top) => top.status,
                None => self.status,
            }
        };
        controls::set_text(status, text);
        a11y::changed(status);
    }

    /// A dialog with a status line of its own opens: what the app says goes there until it
    /// closes, since a dialog in front hides the window's.
    pub fn open_live(&self, status: HWND) {
        self.live.borrow_mut().push(Live { status, closing: false, after: Vec::new() });
    }

    /// The dialog showing `status` now says through `to`: a property sheet's page turned.
    pub fn move_live(&self, status: HWND, to: HWND) {
        if let Some(live) = self.live.borrow_mut().iter_mut().find(|live| live.status == status) {
            live.status = to;
        }
    }

    /// The dialog showing `status` is closing: what the app says now is said once it has gone.
    pub fn closing_live(&self, status: HWND) {
        if let Some(live) = self.live.borrow_mut().iter_mut().find(|live| live.status == status) {
            live.closing = true;
        }
    }

    /// The dialog showing `status` has gone: what was said as it closed is said where it goes
    /// next.
    pub fn close_live(&self, status: HWND) {
        let after = {
            let mut live = self.live.borrow_mut();
            let Some(at) = live.iter().position(|live| live.status == status) else { return };
            live.remove(at).after
        };
        self.say(&after.join(" "));
    }

    /// Says what a change did, and anything else worth saying.
    pub fn say_change(&self, change: &Change) {
        self.say(&speech::sentence(&speech::announcement(&change.announcement, &change.notices)));
    }

    pub fn fail(&self, message: &str) {
        prompts::fail(self.main, message);
    }

    /// The store changed outside the app — a reminder's button, written through the device:
    /// the app takes it in.
    pub fn store_changed(&self) {
        self.crux.tick();
    }

    /// A turn the app left: drawn, its question asked once the app is still, then what it
    /// says said.
    fn take_turn(&self, turn: lumenna_ffi::Turn) {
        if turn.seq <= self.seq.replace(turn.seq) {
            return;
        }
        let landed = turn.view.landing != self.landing.replace(turn.view.landing);
        // What the app holds this moment, ahead of any turn still queued: where it is, and
        // which of its dialogs are open.
        let now = self.crux.view();
        self.follow(now.place.clone());
        self.sidebar.draw(self, &turn.view);
        let still = self.crux.is_still();
        let mut typing = self.detail.draw(&turn.view, &turn.lines, still);
        let list = self.task_list().filter(|list| turn.view.place.as_ref() == Some(&list.place));
        if let Some(list) = &list {
            list.draw(self, &turn.view, &turn.lines, landed);
        }
        if let Some(blocks) = self.blocks().filter(|_| turn.view.place == Some(Place::Blocks)) {
            blocks.draw(self, &turn.view, &turn.lines, landed);
        }
        let day = self.day().filter(|_| turn.view.place == Some(Place::Today));
        if let Some(day) = &day {
            day.draw(self, &turn.view, &turn.lines, landed);
        }
        let form = self.block_form.borrow().clone();
        if let Some(form) = &form {
            typing |= form.draw(self, now.block_form.as_ref(), still);
        }
        let screen = self.proposal.borrow().clone();
        if let Some(screen) = &screen {
            screen.draw(self, now.proposal.as_ref());
        }
        if let Some(sheet) = self.settings_open() {
            sheet.draw(self, &now, &self.crux.lines(), still);
        }
        let pairing = self.pairing.borrow().clone();
        if let Some(pairing) = pairing {
            pairing.draw(self, now.devices.as_ref().and_then(|devices| devices.pairing.as_ref()));
        }
        // Quick add closed by the app, its task added: what it says waits for the dialog to go,
        // which hides nothing of the window's but takes focus with it.
        let adding = self.quick_add.borrow().clone();
        let added = adding.as_ref().is_some_and(|open| {
            open.draw(self, now.line.as_ref(), now.add_refused.as_ref());
            now.line.is_none()
        });
        let open_list = self.reminder_list.borrow().clone();
        if let Some(open_list) = &open_list {
            open_list.draw(self, now.reminders.as_ref(), &self.crux.lines(), &now.reminders_held);
        }
        if typing {
            unsafe {
                let _ = SetTimer(Some(self.main), TIMER_SETTLE, 50, None);
            }
        }
        // Opening at sign-in as the person chose it, done, and the app told how it went.
        if let Some(on) = turn.background {
            settings::keep_running(self, on);
        }
        // An export the app made, saved where the person chooses, once this turn is drawn.
        for file in turn.files {
            self.defer(move |app| settings::save(app.owner(), &file));
        }
        let opens = (turn.view.block_form.is_some() && form.is_none())
            || (turn.view.proposal.is_some() && screen.is_none())
            || (turn.view.reminders.is_some() && open_list.is_none());
        if (turn.view.asking.is_some() || opens) && !self.asking.get() {
            unsafe {
                let _ = SetTimer(Some(self.main), TIMER_ASK, 50, None);
            }
        }
        // The tree says a row it folded itself is expanded or collapsed; a proposal about to
        // open is said by its screen, once it is up.
        let folded = list.is_some_and(|list| list.folded_by_tree.replace(false))
            | day.is_some_and(|day| day.folded_by_tree.replace(false))
            | self.sidebar.folded_by_tree.replace(false);
        let opening = turn.view.proposal.as_ref().filter(|_| screen.is_none()).map(|p| p.announcement.clone());
        let said: Vec<String> = turn
            .said
            .into_iter()
            .filter(|said| !(folded && (said == "Expanded" || said == "Collapsed")))
            .filter(|said| opening.as_ref() != Some(said))
            .collect();
        let said = said.join(" ");
        if added {
            self.defer(move |app| app.say(&said));
        } else {
            self.say(&said);
        }
    }

    /// Where the app went by itself — a place renamed, merged or deleted, the day — the window
    /// goes too. Judged by where the app is now: a turn made before the person opened a place
    /// still names the one they left.
    fn follow_place(&self) {
        self.follow(self.crux.view().place);
    }

    fn follow(&self, place: Option<Place>) {
        if let Some(now) = place
            && self.place_shown().as_ref() != Some(&now)
        {
            self.present(now);
        }
    }

    /// Asks the app's open question once the app is still: not from a turn made before the
    /// last answer reached it.
    fn ask_when_still(&self) {
        if !self.crux.is_still() || self.asking.get() {
            return;
        }
        unsafe {
            let _ = KillTimer(Some(self.main), TIMER_ASK);
        }
        // Only a question counts as asking: a dialog of the app's — a reminder list — asks
        // questions of its own while it is open.
        let now = self.crux.view();
        if now.asking.is_some() {
            self.asking.set(true);
            asking::ask(self);
            self.asking.set(false);
        } else if let Some(form) = now.block_form.filter(|_| self.block_form.borrow().is_none()) {
            block_form::run(self, form);
        } else if let Some(screen) = now.proposal.filter(|_| self.proposal.borrow().is_none()) {
            proposal::run(self, screen);
        } else if let Some(list) = now.reminders.filter(|_| self.reminder_list.borrow().is_none()) {
            reminders::run(self, list);
        }
    }

    /// Does a way in from outside (`lumenna_records::launch`), with the window shown: once the
    /// app is still, so it runs over the store the app has read.
    pub fn launch(&self, id: String) {
        self.show_window();
        *self.launching.borrow_mut() = Some(id);
        unsafe {
            let _ = SetTimer(Some(self.main), TIMER_LAUNCH, 50, None);
        }
    }

    /// Hands the app the way in waiting, once it is still; once still again, the app having
    /// opened its quick-add line (New Task) opens the window's quick add, which draws it. Any
    /// question it asked is asked as every question is.
    fn launch_when_still(&self) {
        if !self.crux.is_still() {
            return;
        }
        let waiting = self.launching.borrow_mut().take();
        if let Some(id) = waiting {
            self.crux.send(ShellEvent::Launch(id));
            return;
        }
        unsafe {
            let _ = KillTimer(Some(self.main), TIMER_LAUNCH);
        }
        if self.crux.view().line.is_some() && self.quick_add.borrow().is_none() {
            self.quick_add(false);
        }
    }

    /// Once the app is still, the field being typed in takes the app's value where it differs:
    /// a change from elsewhere, or the app's own correction of what was typed.
    fn settle_typing(&self) {
        if !self.crux.is_still() {
            return;
        }
        unsafe {
            let _ = KillTimer(Some(self.main), TIMER_SETTLE);
        }
        let now = self.crux.view();
        self.detail.draw(&now, &self.crux.lines(), true);
        let form = self.block_form.borrow().clone();
        if let Some(form) = form {
            form.draw(self, now.block_form.as_ref(), true);
        }
    }

    /// Quick add opens, or closes.
    pub fn open_quick_add(&self, open: Option<Rc<quick_add::QuickAdd>>) {
        *self.quick_add.borrow_mut() = open;
    }

    /// A reminder list's dialog opens, or closes.
    pub fn open_reminders(&self, list: Option<Rc<reminders::Reminders>>) {
        *self.reminder_list.borrow_mut() = list;
    }

    /// The pairing dialog opens, or closes.
    pub fn open_pairing(&self, pairing: Option<Rc<pairing::Pairing>>) {
        *self.pairing.borrow_mut() = pairing;
    }

    /// Settings opens, or closes.
    pub fn open_settings(&self, sheet: Option<Rc<settings::Sheet>>) {
        *self.settings.borrow_mut() = sheet;
    }

    /// Settings, while it is open.
    pub fn settings_open(&self) -> Option<Rc<settings::Sheet>> {
        self.settings.borrow().clone()
    }

    /// Every setting as the core describes it — its name, what it takes, what it can be — for
    /// a settings page to lay itself out by before the app's own arrive.
    pub fn described_settings(&self) -> Vec<lumenna_records::Setting> {
        self.device.settings(None).map(|list| list.settings).unwrap_or_default()
    }

    /// Takes a backup now, into this device's backups, as the automatic ones are; says where.
    /// Takes the automatic backup if one is due, off the window's thread, saying so only if it
    /// failed.
    fn back_up_if_due(&self) {
        let device = Arc::clone(&self.device);
        let window = Poster::new(self.main);
        std::thread::spawn(move || {
            if let Err(error) = device.back_up_if_due() {
                window.say(format!("The automatic backup failed. {}", sentence(&error)));
            }
        });
    }

    pub fn back_up(&self) -> Result<String> {
        let done = self.device.backup()?;
        Ok(speech::sentence(&speech::announcement(&done.announcement, &done.notices)))
    }

    /// What dialogs this window opens are owned by: the dialog in front, else the window.
    pub fn owner(&self) -> HWND {
        let status = self.live.borrow().last().map(|live| live.status);
        status.map_or(self.main, |status| unsafe { GetAncestor(status, GA_ROOT) })
    }

    /// The proposal screen's dialog opens, or closes.
    pub fn open_proposal(&self, screen: Option<Rc<proposal::Screen>>) {
        *self.proposal.borrow_mut() = screen;
    }

    /// The block form's dialog opens, or closes.
    pub fn open_block_form(&self, form: Option<Rc<block_form::Form>>) {
        *self.block_form.borrow_mut() = form;
    }

    /// The block form's dialog, while it is open: what a refusal is said over.
    pub fn block_form_window(&self) -> Option<HWND> {
        self.block_form.borrow().as_ref().and_then(|form| form.window())
    }

    /// Runs `action` once the notification being handled has returned.
    pub fn defer(&self, action: impl FnOnce(&App) + 'static) {
        self.deferred.borrow_mut().push_back(Box::new(action));
        unsafe {
            let _ = PostMessageW(Some(self.main), WM_DEFERRED, WPARAM(0), LPARAM(0));
        }
    }

    /// Opens a popup menu of `(command, text)` at a point on screen, and returns the command
    /// chosen.
    pub fn popup(&self, items: &[(u16, &str)], at: POINT) -> Option<u16> {
        let popup = menu::popup(items);
        let chosen = unsafe {
            let chosen = TrackPopupMenuEx(popup, (TPM_RETURNCMD | TPM_RIGHTBUTTON).0, at.x, at.y, self.main, None);
            let _ = DestroyMenu(popup);
            chosen.0
        };
        u16::try_from(chosen).ok().filter(|c| *c != 0)
    }

    pub fn set_title(&self, title: &str) {
        controls::set_text(self.main, &format!("{title} - Lumenna"));
    }

    // -------------------------------------------------------------------------------------
    // Places and panes
    // -------------------------------------------------------------------------------------

    fn content(&self) -> Option<Content> {
        self.content.borrow().clone()
    }

    pub fn task_list(&self) -> Option<Rc<TaskList>> {
        match self.content()? {
            Content::Tasks(list) => Some(list),
            _ => None,
        }
    }

    pub fn day(&self) -> Option<Rc<DayView>> {
        match self.content()? {
            Content::Day(day) => Some(day),
            _ => None,
        }
    }

    pub fn blocks(&self) -> Option<Rc<BlockList>> {
        match self.content()? {
            Content::Blocks(blocks) => Some(blocks),
            _ => None,
        }
    }

    /// The place the middle pane shows.
    fn place_shown(&self) -> Option<Place> {
        Some(match self.content()? {
            Content::Tasks(list) => list.place.clone(),
            Content::Day(_) => Place::Today,
            Content::Blocks(_) => Place::Blocks,
        })
    }

    /// Goes to a place: shows it, the sidebar following, and — when `focus` — moves focus
    /// into it.
    pub fn go(&self, place: Place, focus: bool) {
        self.show_place(place);
        if focus
            && let Some(content) = self.content() {
                controls::focus(content.view().focus_target());
            }
    }

    /// Opens a place: the app goes there, and the middle pane shows it.
    pub fn show_place(&self, place: Place) {
        self.crux.send(ShellEvent::Open(place.clone()));
        self.present(place);
    }

    /// Shows a place in the middle pane, and clears the details until something is chosen.
    fn present(&self, place: Place) {
        // Out of the slot before its windows go, so the notifications their destruction
        // sends find no view to reach.
        let old = self.content.borrow_mut().take();
        if let Some(old) = old {
            for hwnd in old.view().windows() {
                a11y::clear(hwnd);
                unsafe {
                    let _ = DestroyWindow(hwnd);
                }
            }
        }
        let pane = self.panes[1];
        let content = match &place {
            Place::Today => Content::Day(DayView::create(self, pane)),
            Place::Blocks => Content::Blocks(BlockList::create(self, pane)),
            _ => Content::Tasks(TaskList::create(self, pane, place.clone())),
        };
        self.set_title(&place.title());
        *self.content.borrow_mut() = Some(content.clone());
        self.apply_font(pane);
        dark::controls_in(pane);
        self.layout();
    }

    /// The panes F6 goes through, by what takes focus in each: the details only when they
    /// show a task.
    fn pane_targets(&self) -> Vec<(HWND, HWND)> {
        let mut targets = vec![(self.panes[0], self.sidebar.tree.hwnd)];
        if let Some(content) = self.content() {
            targets.push((self.panes[1], content.view().focus_target()));
        }
        if let Some(target) = self.detail.focus_target() {
            targets.push((self.panes[2], target));
        }
        targets
    }

    fn move_pane(&self, step: isize) {
        let targets = self.pane_targets();
        if targets.is_empty() {
            return;
        }
        let focus = controls::focused();
        let count = targets.len() as isize;
        let current = targets.iter().position(|(pane, _)| controls::within(*pane, focus));
        let next = match current {
            Some(index) => (index as isize + step).rem_euclid(count),
            None if step > 0 => 0,
            None => count - 1,
        };
        controls::focus(targets[next as usize].1);
    }

    /// Opens the selected task's details: focus to the first field.
    pub fn open_detail(&self) {
        if let Some(target) = self.detail.focus_target() {
            controls::focus(target);
        }
    }

    // -------------------------------------------------------------------------------------
    // Layout and fonts
    // -------------------------------------------------------------------------------------

    /// The system's message font at the window's DPI: what Windows' Text size setting enlarges.
    fn message_font(&self) -> LOGFONTW {
        font::message_font(unsafe { GetDpiForWindow(self.main) }.max(96))
    }

    /// A setting changed. Windows' Text size, among others, changes the message font, and
    /// which message announces it is not documented, so any change is looked at, and the
    /// window is laid out again only if the font is not what it was.
    fn setting_changed(&self) {
        // Dark or light, and whether a high-contrast theme came or went.
        if dark::refresh() {
            dark::window(self.main);
        }
        // Colours' marks in the appearance now, or none under high contrast.
        swatch::redraw_all();
        let font = self.message_font();
        if self.font_made_from.get() != (font.lfHeight, font.lfFaceName) {
            self.update_font();
            self.layout();
        }
    }

    /// The system's message font at the window's DPI, given to every control.
    fn update_font(&self) {
        let dpi = unsafe { GetDpiForWindow(self.main) }.max(96);
        let message = self.message_font();
        self.font_made_from.set((message.lfHeight, message.lfFaceName));
        let font = unsafe { CreateFontIndirectW(&message) };
        let line = unsafe {
            let dc = GetDC(Some(self.main));
            let old = SelectObject(dc, font.into());
            let mut text = TEXTMETRICW::default();
            let _ = GetTextMetricsW(dc, &mut text);
            SelectObject(dc, old);
            ReleaseDC(Some(self.main), dc);
            text.tmHeight + text.tmExternalLeading
        };
        let scale = dpi as f32 / 96.0;
        let px = |n: f32| (n * scale).round() as i32;
        self.metrics.set(Metrics { scale, line, field: line + px(8.0), button: line + px(10.0) });
        let old = self.font.replace(font);
        self.apply_font(self.main);
        if !old.is_invalid() {
            unsafe {
                let _ = DeleteObject(old.into());
            }
        }
    }

    fn apply_font(&self, parent: HWND) {
        unsafe extern "system" fn give(hwnd: HWND, font: LPARAM) -> windows::core::BOOL {
            controls::send(hwnd, WM_SETFONT, font.0 as usize, 1);
            true.into()
        }
        unsafe {
            let _ = EnumChildWindows(Some(parent), Some(give), LPARAM(self.font.get().0 as isize));
        }
    }

    fn layout(&self) {
        let mut client = RECT::default();
        unsafe {
            let _ = GetClientRect(self.main, &mut client);
        }
        let (width, height) = (client.right, client.bottom);
        let m = self.metrics.get();
        let status_height = m.line + m.px(10);
        let body = (height - status_height).max(0);
        let sidebar = m.px(230).min(width / 4);
        let detail = m.px(340).min(width / 3);
        let middle = (width - sidebar - detail).max(0);
        controls::place(self.panes[0], rect(0, 0, sidebar, body));
        controls::place(self.panes[1], rect(sidebar, 0, middle, body));
        controls::place(self.panes[2], rect(sidebar + middle, 0, detail, body));
        controls::place(self.status, rect(m.gap(), body + m.px(5), width - 2 * m.gap(), m.line));
        self.sidebar.layout(sidebar, body, m);
        if let Some(content) = self.content() {
            content.view().layout(middle, body, m);
        }
        self.detail.layout(detail, body, m);
    }

    // -------------------------------------------------------------------------------------
    // Showing and hiding
    // -------------------------------------------------------------------------------------

    pub fn show_window(&self) {
        unsafe {
            if IsIconic(self.main).as_bool() {
                let _ = ShowWindow(self.main, SW_RESTORE);
            } else {
                let _ = ShowWindow(self.main, SW_SHOW);
            }
            let _ = SetForegroundWindow(self.main);
        }
    }

    fn hide_window(&self) {
        unsafe {
            let _ = ShowWindow(self.main, SW_HIDE);
        }
    }

    fn exit(&self) {
        unsafe {
            let _ = KillTimer(Some(self.main), TIMER_REFRESH);
        }
        shortcuts::unregister_all(self.main);
        tray::remove_icon(self.main);
        unsafe {
            let _ = DestroyWindow(self.main);
        }
    }

    /// Quick add: from New Task, starting with the place shown, so a task added while
    /// looking at a project lands in it; or from anywhere — the global shortcut, or the
    /// notification area's New Task — starting empty, over whatever is in front.
    fn quick_add(&self, from_anywhere: bool) {
        let prefix = if from_anywhere { String::new() } else { self.task_list().map(|l| l.quick_add_prefix()).unwrap_or_default() };
        let owner = (!from_anywhere || controls::is_visible(self.main)).then_some(self.main);
        // The app adds it, holds what it made, and says so.
        quick_add::run(self, owner, &prefix);
    }

    // -------------------------------------------------------------------------------------
    // Commands
    // -------------------------------------------------------------------------------------

    /// Enter, from the dialog manager: what it does depends on where focus is.
    fn enter(&self) {
        let focus = controls::focused();
        if controls::within(self.panes[1], focus) {
            if let Some(content) = self.content() {
                content.view().enter(self, focus);
            }
        } else if controls::within(self.panes[2], focus) {
            self.detail.save(self);
        } else if focus == self.sidebar.tree.hwnd {
            self.move_pane(1);
        }
    }

    /// Escape: from the details or a field, back to the list.
    fn escape(&self) {
        let focus = controls::focused();
        if controls::within(self.panes[1], focus) {
            if let Some(content) = self.content() {
                content.view().escape(self, focus);
            }
        } else if controls::within(self.panes[2], focus)
            && let Some(content) = self.content() {
                controls::focus(content.view().focus_target());
            }
    }

    /// Whether focus is in a text field, where the edit commands are the field's own.
    fn editing(&self) -> Option<HWND> {
        let focus = controls::focused();
        let mut class = [0u16; 16];
        let length = unsafe { GetClassNameW(focus, &mut class) };
        let class = String::from_utf16_lossy(&class[..length.max(0) as usize]);
        class.eq_ignore_ascii_case("Edit").then_some(focus)
    }

    /// The task the Task menu acts on: the one in the details when focus is there, else the
    /// one selected in the list or the day.
    fn task_in_hand(&self) -> Option<String> {
        if controls::within(self.panes[2], controls::focused()) {
            return self.detail.task_id();
        }
        match self.content()? {
            Content::Tasks(list) => list.selected().map(|row| row.id),
            Content::Day(day) => day.selected_task(),
            Content::Blocks(_) => None,
        }
    }

    /// What can be done to the task the Task menu acts on.
    fn task_actions_in_hand(&self) -> Option<Vec<Offered>> {
        if let Some(Content::Tasks(list)) = self.content()
            && !controls::within(self.panes[2], controls::focused())
        {
            return list.selected_actions();
        }
        self.detail_actions_in_hand()
    }

    /// What can be done to the task the details show, when the Task menu acts on it: focus is
    /// there, or the day's sitting shows its task there.
    fn detail_actions_in_hand(&self) -> Option<Vec<Offered>> {
        if controls::within(self.panes[2], controls::focused()) {
            return self.detail.actions();
        }
        match self.content()? {
            Content::Day(day) => day.selected_task().and_then(|_| self.detail.actions()),
            Content::Tasks(_) | Content::Blocks(_) => None,
        }
    }

    /// A Task-menu command: the task in hand's action of that kind, as the core offers it, so
    /// Mark Done marks a done task not done, and in the trash Restore restores.
    fn act_on_task_in_hand(&self, kinds: &[ActionKind]) {
        // The list's row in hand is the app's, and so is saying when it has no such action.
        if !controls::within(self.panes[2], controls::focused())
            && let Some(list) = self.task_list()
        {
            if list.selected().is_none() {
                return self.say("No task is selected");
            }
            return self.crux.send(ShellEvent::Run(kinds.to_vec()));
        }
        let Some(offered) = self.detail_actions_in_hand() else {
            return self.say("No task is selected");
        };
        let Some(action) = actions::of_kind(&offered, kinds) else {
            // Why the task the details show has none, as the app says it: beside the day that
            // is a sitting's task, not the row in hand.
            return self.crux.send(ShellEvent::Details(DetailsEvent::Run(kinds.to_vec())));
        };
        self.detail.act(self, action);
    }

    /// Names the Task menu's Mark Done item as the task in hand's own, Mark Not Done when it
    /// is done, as the menu opens.
    fn name_task_menu(&self, popup: HMENU) {
        let kinds = [ActionKind::MarkDone, ActionKind::MarkNotDone];
        let Some(action) = self.task_actions_in_hand().and_then(|offered| actions::of_kind(&offered, &kinds)) else { return };
        let text = HSTRING::from(format!("&{}\tCtrl+K", action.title));
        unsafe {
            let _ = ModifyMenuW(popup, u32::from(menu::MARK_DONE), MF_BYCOMMAND | MF_STRING, usize::from(menu::MARK_DONE), &text);
        }
    }
    pub fn menu_command(&self, id: u16) {
        match id {
            1 => self.enter(),
            2 => self.escape(),
            menu::NEW_TASK => self.quick_add(false),
            menu::NEW_BLOCK => {
                // On the day shown, which is today from anywhere else.
                if self.day().is_none() {
                    self.go(Place::Today, true);
                }
                self.crux.send(ShellEvent::Block(lumenna_app::BlockEvent::New { start: None, minutes: None }));
            }
            menu::NEW_PROJECT => self.sidebar.add(self, SidebarGroup::Projects),
            menu::NEW_LABEL => self.sidebar.add(self, SidebarGroup::Labels),
            menu::NEW_FILTER => self.sidebar.add(self, SidebarGroup::Filters),
            menu::SYNC_NOW => self.crux.send(ShellEvent::Devices(lumenna_app::DevicesEvent::SyncNow)),
            menu::BACK_UP => match self.back_up() {
                Ok(said) => self.say(&said),
                Err(error) => self.fail(&sentence(&error)),
            },
            menu::SETTINGS => settings::show(self, Page::General),
            menu::EXPORT_IMPORT => settings::show(self, Page::Export),
            menu::RESTORE_BACKUP => settings::import(self.main, "Restore from a Backup", &settings::BACKUPS),
            menu::CLOSE_WINDOW => self.hide_window(),
            menu::EXIT => self.exit(),

            menu::UNDO | menu::REDO => {
                // In a field, its own typing; elsewhere, the store.
                if let Some(field) = self.editing() {
                    controls::send(field, EM_UNDO, 0, 0);
                    return;
                }
                self.crux.send(if id == menu::UNDO { ShellEvent::Undo } else { ShellEvent::Redo });
            }
            menu::CUT | menu::COPY | menu::PASTE | menu::SELECT_ALL => {
                if let Some(field) = self.editing() {
                    match id {
                        menu::CUT => controls::send(field, WM_CUT, 0, 0),
                        menu::COPY => controls::send(field, WM_COPY, 0, 0),
                        menu::PASTE => controls::send(field, WM_PASTE, 0, 0),
                        _ => controls::send(field, EM_SETSEL, 0, -1),
                    };
                }
            }
            menu::FILTER => {
                if self.task_list().is_none() {
                    self.go(Place::Tasks, false);
                }
                if let Some(list) = self.task_list() {
                    list.focus_filter();
                }
            }

            menu::GO_TODAY => self.go(Place::Today, true),
            menu::GO_TASKS => self.go(Place::Tasks, true),
            menu::GO_BLOCKS => self.go(Place::Blocks, true),
            menu::GO_TRASH => self.go(Place::Trash, true),
            menu::GO_HISTORY => self.go(Place::History, true),
            menu::NEXT_PANE => self.move_pane(1),
            menu::PREVIOUS_PANE => self.move_pane(-1),

            menu::OPEN_TASK => {
                if self.task_in_hand().is_some() {
                    self.open_detail();
                }
            }
            menu::SAVE_TASK => self.detail.save(self),
            menu::PROPERTIES => {
                // The details are already the task's own form.
                if !controls::within(self.panes[2], controls::focused())
                    && !self.content().is_some_and(|content| content.view().properties(self))
                {
                    self.say("No task or block is selected");
                }
            }
            menu::KEYBOARD_HELP => help::show(self.main),
            command if !task_kinds(command).is_empty() => self.act_on_task_in_hand(task_kinds(command)),
            command if day_kind(command).is_some() => {
                if self.day().is_none() {
                    self.go(Place::Today, true);
                }
                if let (Some(day), Some(kind)) = (self.day(), day_kind(command)) {
                    day.act_on_day(self, kind);
                }
            }
            menu::PREVIOUS_DAY | menu::NEXT_DAY | menu::GO_TO_NOW | menu::GO_TO_DAY => {
                let step = match id {
                    menu::PREVIOUS_DAY => DayMove::Previous,
                    menu::NEXT_DAY => DayMove::Next,
                    menu::GO_TO_NOW => DayMove::Now,
                    _ => DayMove::Ask,
                };
                // The app goes to the day from wherever it is, and the window with it.
                self.crux.send(ShellEvent::Day(step));
                self.follow_place();
                if let Some(day) = self.day() {
                    controls::focus(day.tree.hwnd);
                }
            }

            menu::SHOW_WINDOW => self.show_window(),
            menu::ABOUT => {
                let text = format!(
                    "Lumenna {}\n\nA task manager and day planner.\nData folder: {}",
                    env!("CARGO_PKG_VERSION"),
                    self.directory.display()
                );
                unsafe {
                    MessageBoxW(Some(self.main), &HSTRING::from(text), w!("About Lumenna"), MB_OK | MB_ICONINFORMATION);
                }
            }
            _ => {}
        }
    }

    fn on_command(&self, wparam: WPARAM, lparam: LPARAM) {
        let id = controls::low_word(wparam.0);
        let code = controls::high_word(wparam.0);
        let control = HWND(lparam.0 as _);
        if control.is_invalid() {
            self.menu_command(id);
            return;
        }
        if let Some(content) = self.content()
            && content.view().command(self, control, id, code) {
                return;
            }
        if self.detail.command(self, control, id, code) {
            return;
        }
        // A focused push button and Enter, which the dialog manager reports as the button.
        if i32::from(id) == IDOK.0 || i32::from(id) == IDCANCEL.0 {
            self.menu_command(id);
        }
    }

    fn on_notify(&self, lparam: LPARAM) -> Option<isize> {
        let header = unsafe { &*(lparam.0 as *const NMHDR) };
        if header.code == NM_RCLICK {
            // A right-click on a tree, at the pointer. Handled here rather than left to the
            // tree's own WM_CONTEXTMENU, which reaches the window naming the pane, not the tree.
            let position = unsafe { GetMessagePos() } as usize;
            let at = POINT {
                x: i32::from(controls::low_word(position) as i16),
                y: i32::from(controls::high_word(position) as i16),
            };
            self.context_menu(header.hwndFrom, Some(at));
            return Some(1);
        }
        if header.hwndFrom == self.sidebar.tree.hwnd {
            return self.sidebar.notify(self, header, lparam);
        }
        self.content()?.view().notify(self, header, lparam)
    }

    fn on_context_menu(&self, control: HWND, lparam: LPARAM) {
        // From the keyboard — Shift+F10 or the Applications key — the point is -1, -1, as two
        // 16-bit halves. A tree view passes that on altered, naming the pane it is in rather
        // than itself; so a request from a pane is from the keyboard, for what has focus in it.
        let (x, y) = (controls::low_word(lparam.0 as usize) as i16, controls::high_word(lparam.0 as usize) as i16);
        let from_pane = self.panes.contains(&control);
        let control = if from_pane { controls::focused() } else { control };
        let point = (!from_pane && (x, y) != (-1, -1)).then(|| POINT { x: i32::from(x), y: i32::from(y) });
        self.context_menu(control, point);
    }

    fn context_menu(&self, control: HWND, point: Option<POINT>) {
        if self.sidebar.context_menu(self, control, point) {
            return;
        }
        if let Some(content) = self.content() {
            content.view().context_menu(self, control, point);
        }
    }

    fn on_tray(&self, wparam: WPARAM, lparam: LPARAM) {
        let event = u32::from(controls::low_word(lparam.0 as usize));
        match event {
            windows::Win32::UI::Shell::NIN_SELECT | NIN_KEYSELECT | WM_LBUTTONDBLCLK => {
                self.show_window();
            }
            WM_CONTEXTMENU => {
                let at = POINT {
                    x: i32::from(controls::low_word(wparam.0) as i16),
                    y: i32::from(controls::high_word(wparam.0) as i16),
                };
                // The documented dance for a notification-area menu: the window in front
                // first, or the menu does not close when focus leaves it.
                unsafe {
                    let _ = SetForegroundWindow(self.main);
                }
                let items = [
                    (menu::SHOW_WINDOW, "&Show Lumenna"),
                    (menu::NEW_TASK, "&New Task..."),
                    (menu::SYNC_NOW, "S&ync Now"),
                    (0, ""),
                    (menu::EXIT, "E&xit"),
                ];
                let chosen = self.popup(&items, at);
                unsafe {
                    let _ = PostMessageW(Some(self.main), WM_NULL, WPARAM(0), LPARAM(0));
                }
                match chosen {
                    Some(menu::NEW_TASK) => self.quick_add(true),
                    Some(command) => self.menu_command(command),
                    None => {}
                }
            }
            _ => {}
        }
    }

    /// Once a second: the app looks whether another process wrote or the minute moved, and
    /// redraws for either.
    fn on_timer(&self) {
        self.crux.tick();
        let ticks = self.ticks.get().wrapping_add(1);
        self.ticks.set(ticks);
        // Every few seconds, while another process gives them, whether it has stopped; and
        // what has come due, every twenty.
        if (!self.reminders.get() && ticks.is_multiple_of(5)) || ticks.is_multiple_of(20) {
            self.give_reminders();
        }
    }

    // -------------------------------------------------------------------------------------
    // Reminders
    // -------------------------------------------------------------------------------------

    /// Shows what has come due as toasts, while this process is the one on the device that
    /// gives them: another (`lum notify`, another copy) may hold that right, and this takes
    /// it when that one stops.
    fn give_reminders(&self) {
        let Some(toasts) = &self.toasts else { return };
        // The device's own, as the Mac gives them: it takes the right to give them while no
        // other process has it, and keeps it.
        let Ok(due) = self.device.deliver_reminders(None) else { return };
        self.reminders.set(!due.elsewhere);
        let window = Poster::new(self.main);
        for reminder in &due.due {
            toasts.remind(reminder, window);
        }
        if !due.missed_summary.is_empty() {
            toasts.summary(&due.missed_summary, window);
        }
    }

    /// A toast answered: its button's dismissal or snooze, or what it is about opened.
    fn reminder_answered(&self, answered: Answered) {
        let done = match answered {
            Answered::Dismiss(reminder) => self.device.dismiss_reminder(reminder.key, reminder.occurrence),
            Answered::Snooze(reminder) => self.device.snooze_reminder(reminder.key, reminder.occurrence, None),
            Answered::Carry(reminder) => self.device.carry_rest_forward(format!("{}@{}", reminder.target, reminder.occurrence)),
            other => return self.reminder_opened(other),
        };
        match done {
            Ok(change) => {
                self.store_changed();
                self.say_change(&change);
            }
            Err(error) => self.fail(&sentence(&error)),
        }
    }

    /// A toast's body, or its Replan: the window, at what it is about.
    fn reminder_opened(&self, answered: Answered) {
        match answered {
            Answered::Dismiss(_) | Answered::Snooze(_) | Answered::Carry(_) => {}
            Answered::Open(reminder) => {
                self.show_window();
                if reminder.kind == "block" {
                    // Its day, the block in hand, which the app lands on.
                    self.crux.send(ShellEvent::ShowBlock { series: reminder.target.clone(), date: reminder.occurrence.clone() });
                    self.follow_place();
                    if let Some(day) = self.day() {
                        controls::focus(day.tree.hwnd);
                    }
                } else {
                    self.go(Place::Tasks, true);
                    if let Some(list) = self.task_list() {
                        list.land_on(&reminder.target);
                    }
                }
            }
            Answered::Summary => self.show_window(),
            // Today's own Replan from Now, as the Day menu runs it, once the window is in front.
            Answered::Replan => {
                self.show_window();
                self.crux.send(ShellEvent::Day(DayMove::Now));
                self.follow_place();
                self.crux.send(ShellEvent::Proposal(lumenna_app::ProposalEvent::Replan));
            }
        }
    }
}

/// Sizes the main window at 1100 by 720 at 100%, scaled to its screen's DPI, and keeps it
/// within the screen's work area. In pixels alone it was a postage stamp at 200% and wider
/// than a 1024-pixel screen.
fn fit_to_screen(window: HWND) {
    let (dpi, work) = font::screen(Some(window));
    let scaled = |size: i32| size * dpi as i32 / 96;
    let width = scaled(1100).min(work.right - work.left);
    let height = scaled(720).min(work.bottom - work.top);
    let mut placed = RECT::default();
    unsafe {
        let _ = GetWindowRect(window, &mut placed);
        let left = placed.left.clamp(work.left, work.right - width);
        let top = placed.top.clamp(work.top, work.bottom - height);
        let _ = SetWindowPos(window, None, left, top, width, height, SWP_NOZORDER | SWP_NOACTIVATE);
    }
}

unsafe extern "system" fn main_procedure(hwnd: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        if message == WM_GETMINMAXINFO {
            let info = &mut *(lparam.0 as *mut MINMAXINFO);
            let scale = GetDpiForWindow(hwnd).max(96) as f32 / 96.0;
            info.ptMinTrackSize = POINT { x: (860.0 * scale) as i32, y: (560.0 * scale) as i32 };
            return LRESULT(0);
        }
        let Some(app) = app() else {
            return DefWindowProcW(hwnd, message, wparam, lparam);
        };
        match message {
            WM_COMMAND => app.on_command(wparam, lparam),
            WM_NOTIFY => return LRESULT(app.on_notify(lparam).unwrap_or(0)),
            WM_CONTEXTMENU => app.on_context_menu(HWND(wparam.0 as _), lparam),
            WM_INITMENUPOPUP => app.name_task_menu(HMENU(wparam.0 as _)),
            WM_SIZE => app.layout(),
            WM_SETTINGCHANGE => app.setting_changed(),
            WM_CTLCOLORSTATIC | WM_CTLCOLORBTN | WM_CTLCOLOREDIT | WM_CTLCOLORLISTBOX => {
                if let Some(answer) = dark::colour(message, wparam, lparam) {
                    return answer;
                }
                return DefWindowProcW(hwnd, message, wparam, lparam);
            }
            WM_ERASEBKGND => {
                return dark::erase(hwnd, wparam).unwrap_or_else(|| DefWindowProcW(hwnd, message, wparam, lparam));
            }
            // Common controls take the system's colours from this, which only a top-level
            // window is sent.
            WM_SYSCOLORCHANGE => {
                unsafe extern "system" fn tell(child: HWND, _: LPARAM) -> windows::core::BOOL {
                    controls::send(child, WM_SYSCOLORCHANGE, 0, 0);
                    true.into()
                }
                let _ = EnumChildWindows(Some(hwnd), Some(tell), LPARAM(0));
            }
            WM_DPICHANGED => {
                let suggested = &*(lparam.0 as *const RECT);
                let _ = SetWindowPos(
                    hwnd,
                    None,
                    suggested.left,
                    suggested.top,
                    suggested.right - suggested.left,
                    suggested.bottom - suggested.top,
                    SWP_NOZORDER | SWP_NOACTIVATE,
                );
                app.update_font();
                app.layout();
                swatch::redraw_all();
            }
            WM_ACTIVATE => {
                // Put focus back where it was, as a dialog does: otherwise returning with
                // Alt+Tab leaves it on the window itself, where a screen reader finds nothing.
                if u32::from(controls::low_word(wparam.0)) == WA_INACTIVE {
                    let focus = controls::focused();
                    if controls::within(hwnd, focus) {
                        app.last_focus.set(focus);
                    }
                } else {
                    // Back in front, as from sleep: whatever came due meanwhile.
                    app.give_reminders();
                    let last = app.last_focus.get();
                    if !last.is_invalid() && IsWindow(Some(last)).as_bool() && IsWindowVisible(last).as_bool() {
                        controls::focus(last);
                    } else {
                        app.move_pane(1);
                    }
                    return LRESULT(0);
                }
            }
            WM_CLOSE => {
                // Hidden, not closed: the app stays resident, syncing.
                app.hide_window();
                return LRESULT(0);
            }
            WM_TIMER if wparam.0 == TIMER_ASK => app.ask_when_still(),
            WM_TIMER if wparam.0 == TIMER_SETTLE => app.settle_typing(),
            WM_TIMER if wparam.0 == TIMER_LAUNCH => app.launch_when_still(),
            WM_TIMER => app.on_timer(),
            WM_TURN => app.take_turn(taken(lparam)),
            WM_SAY => app.say(&said(lparam)),
            WM_EVENT => app.crux.send(taken::<lumenna_ffi::ShellEvent>(lparam)),
            WM_REMINDER => app.reminder_answered(taken(lparam)),
            WM_DEFERRED => {
                let action = app.deferred.borrow_mut().pop_front();
                if let Some(action) = action {
                    action(&app);
                }
            }
            WM_HOTKEY => match shortcuts::kind_of(wparam.0 as i32) {
                Some(Kind::Show) => app.show_window(),
                Some(Kind::QuickAdd) => app.quick_add(true),
                None => {}
            },
            WM_TRAY => app.on_tray(wparam, lparam),
            WM_SHOW_RUNNING => app.show_window(),
            WM_COPYDATA => match tray::launch_handed(lparam) {
                Some(id) => {
                    app.launch(id);
                    return LRESULT(1);
                }
                None => return DefWindowProcW(hwnd, message, wparam, lparam),
            },
            WM_DESTROY => PostQuitMessage(0),
            other if other == app.taskbar_created && other != 0 => tray::add_icon(hwnd, app.icon),
            _ => return DefWindowProcW(hwnd, message, wparam, lparam),
        }
        LRESULT(0)
    }
}

/// A pane: a container that hands everything its controls send on to the main window.
unsafe extern "system" fn pane_procedure(hwnd: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        match message {
            WM_COMMAND | WM_NOTIFY => {
                let main = GetAncestor(hwnd, GA_ROOT);
                SendMessageW(main, message, Some(wparam), Some(lparam))
            }
            // A multi-line field sends its parent WM_CLOSE on Escape, which would destroy the
            // pane.
            WM_CLOSE => LRESULT(0),
            WM_CTLCOLORSTATIC | WM_CTLCOLORBTN | WM_CTLCOLOREDIT | WM_CTLCOLORLISTBOX => {
                dark::colour(message, wparam, lparam).unwrap_or_else(|| DefWindowProcW(hwnd, message, wparam, lparam))
            }
            WM_ERASEBKGND => dark::erase(hwnd, wparam).unwrap_or_else(|| DefWindowProcW(hwnd, message, wparam, lparam)),
            _ => DefWindowProcW(hwnd, message, wparam, lparam),
        }
    }
}

/// The day's own action a Day-menu command means.
fn day_kind(command: u16) -> Option<ActionKind> {
    match command {
        menu::PLAN_DAY => Some(ActionKind::PlanDay),
        menu::REPLAN => Some(ActionKind::Replan),
        menu::WHAT_NOW => Some(ActionKind::WhatNow),
        menu::END_DAY => Some(ActionKind::EndDay),
        menu::CLEAR_DAY => Some(ActionKind::ClearDay),
        _ => None,
    }
}

/// The kinds of action a Task-menu command means: none for any other command.
fn task_kinds(command: u16) -> &'static [ActionKind] {
    match command {
        menu::MARK_DONE => &[ActionKind::MarkDone, ActionKind::MarkNotDone],
        menu::PUT_IN_BLOCK => &[ActionKind::PutInBlock],
        menu::MOVE_TO_PROJECT => &[ActionKind::MoveToProject],
        menu::MAKE_SUBTASK => &[ActionKind::MakeSubtaskOf],
        menu::MOVE_TO_TOP => &[ActionKind::MoveToTopLevel],
        menu::WAIT_FOR => &[ActionKind::WaitFor],
        menu::TRASH_TASK => &[ActionKind::Delete],
        menu::RESTORE_TASK => &[ActionKind::Restore],
        menu::ERASE_TASK => &[ActionKind::DeleteForGood],
        menu::ERASE_PERMANENTLY => &[ActionKind::ErasePermanently],
        _ => &[],
    }
}
