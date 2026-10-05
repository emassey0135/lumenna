//! `SysTreeView32`, which every list in the app is.
//!
//! The tree view reports each item's level, its position among its siblings and how many
//! there are, whether it is expanded, and — with checkboxes — whether it is checked, from its
//! own structure, through MSAA and UI Automation alike. So the core's `depth`, `index`,
//! `count`, `expanded` and `checked` become the tree's structure rather than words.

use std::cell::{Cell, RefCell};
use std::collections::HashSet;

use windows::Win32::Foundation::{HWND, LPARAM, POINT, RECT};
use windows::Win32::Graphics::Gdi::{ClientToScreen, InvalidateRect, ScreenToClient};
use windows::Win32::UI::Controls::{
    HTREEITEM, NMTREEVIEWW, SetWindowTheme, TVE_COLLAPSE, TVE_EXPAND, TVGN_CARET, TVHITTESTINFO,
    TVI_LAST, TVM_HITTEST,
    TVI_ROOT, TVIF_PARAM, TVIF_STATE, TVIF_TEXT, TVINSERTSTRUCTW, TVINSERTSTRUCTW_0,
    TVIS_STATEIMAGEMASK, TVITEMEXW, TVM_DELETEITEM, TVM_ENSUREVISIBLE, TVM_EXPAND,
    TVM_GETITEMRECT, TVM_GETITEMW, TVM_GETNEXTITEM, TVM_INSERTITEMW,
    TVM_SELECTITEM, TVM_SETEXTENDEDSTYLE, TVM_SETITEMW, TVS_CHECKBOXES, TVS_DISABLEDRAGDROP,
    TVS_EX_DOUBLEBUFFER, TVS_HASBUTTONS, TVS_HASLINES, TVS_LINESATROOT, TVS_NOHSCROLL, TVS_SHOWSELALWAYS,
    WC_TREEVIEWW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GWL_STYLE, GetWindowLongPtrW, SetWindowLongPtrW, WM_SETREDRAW, WS_EX_CLIENTEDGE, WS_TABSTOP,
};
use windows::core::{PWSTR, w};

use super::{a11y, controls};
use crate::outline;

/// One row as the tree shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Item {
    /// What identifies it across a reload.
    pub key: String,
    /// The line as read.
    pub text: String,
    pub depth: u32,
    /// Whether its checkbox is checked; `None` for a row with no checkbox.
    pub checked: Option<bool>,
}

/// A tree view, and the rows it was last given.
pub struct Tree {
    pub hwnd: HWND,
    items: RefCell<Vec<Item>>,
    handles: RefCell<Vec<HTREEITEM>>,
    collapsed: RefCell<HashSet<String>>,
    busy: Cell<bool>,
}

impl Tree {
    /// Creates one, named `name` for screen readers.
    pub fn create(parent: HWND, id: u16, name: &str, checkboxes: bool) -> Self {
        // No sideways scrolling: a long row is cut off on screen, with the tree's own tooltip
        // showing it whole, and is read whole either way.
        let style =
            TVS_HASBUTTONS | TVS_HASLINES | TVS_LINESATROOT | TVS_SHOWSELALWAYS | TVS_DISABLEDRAGDROP | TVS_NOHSCROLL;
        let hwnd = controls::create(parent, WC_TREEVIEWW, "", style | WS_TABSTOP.0, WS_EX_CLIENTEDGE.0, id);
        unsafe {
            let _ = SetWindowTheme(hwnd, w!("Explorer"), None);
            if checkboxes {
                // The documented order: set after the control exists and before it has any
                // items, or the state images are not made.
                let style = GetWindowLongPtrW(hwnd, GWL_STYLE);
                SetWindowLongPtrW(hwnd, GWL_STYLE, style | TVS_CHECKBOXES as isize);
            }
        }
        controls::send(hwnd, TVM_SETEXTENDEDSTYLE, TVS_EX_DOUBLEBUFFER as usize, TVS_EX_DOUBLEBUFFER as isize);
        a11y::set_name(hwnd, name);
        Self {
            hwnd,
            items: RefCell::new(Vec::new()),
            handles: RefCell::new(Vec::new()),
            collapsed: RefCell::new(HashSet::new()),
            busy: Cell::new(false),
        }
    }

    /// Whether the tree is being refilled. Notifications it sends meanwhile are about the
    /// refill, not about anything the person did, and are ignored.
    pub fn busy(&self) -> bool {
        self.busy.get()
    }

    /// Shows these rows.
    ///
    /// The same rows in the same order are updated in place: only lines that changed are
    /// touched, the selection stays, and a screen reader hears nothing unless the focused
    /// line itself changed. Anything else rebuilds, and the caller then chooses the selection.
    pub fn set(&self, items: Vec<Item>) -> bool {
        let same = {
            let old = self.items.borrow();
            old.len() == items.len() && old.iter().zip(&items).all(|(a, b)| a.key == b.key && a.depth == b.depth)
        };
        self.busy.set(true);
        if same {
            let changed: Vec<(usize, Item)> = {
                let old = self.items.borrow();
                items.iter().enumerate().filter(|(i, item)| old[*i] != **item).map(|(i, item)| (i, item.clone())).collect()
            };
            let handles = self.handles.borrow().clone();
            for (index, item) in changed {
                let mut text = wide(&item.text);
                let mut tv = TVITEMEXW {
                    mask: TVIF_TEXT | TVIF_STATE,
                    hItem: handles[index],
                    pszText: PWSTR(text.as_mut_ptr()),
                    state: state_image(item.checked),
                    stateMask: TVIS_STATEIMAGEMASK.0,
                    ..Default::default()
                };
                controls::send(self.hwnd, TVM_SETITEMW, 0, &raw mut tv as isize);
            }
        } else {
            controls::send(self.hwnd, WM_SETREDRAW, 0, 0);
            controls::send(self.hwnd, TVM_DELETEITEM, 0, TVI_ROOT.0);
            let depths: Vec<u32> = items.iter().map(|item| item.depth).collect();
            let parents = outline::parents(&depths);
            let mut handles: Vec<HTREEITEM> = Vec::with_capacity(items.len());
            for (index, item) in items.iter().enumerate() {
                let mut text = wide(&item.text);
                let insert = TVINSERTSTRUCTW {
                    hParent: parents[index].map_or(TVI_ROOT, |parent| handles[parent]),
                    hInsertAfter: TVI_LAST,
                    Anonymous: TVINSERTSTRUCTW_0 {
                        itemex: TVITEMEXW {
                            mask: TVIF_TEXT | TVIF_PARAM | TVIF_STATE,
                            pszText: PWSTR(text.as_mut_ptr()),
                            lParam: LPARAM(index as isize),
                            state: state_image(item.checked),
                            stateMask: TVIS_STATEIMAGEMASK.0,
                            ..Default::default()
                        },
                    },
                };
                let handle = controls::send(self.hwnd, TVM_INSERTITEMW, 0, &raw const insert as isize);
                handles.push(HTREEITEM(handle));
            }
            // Expanded unless the person collapsed it, which is remembered by key across
            // reloads; only once its children are in, since an item without any cannot open.
            let collapsed = self.collapsed.borrow().clone();
            for (index, has_children) in outline::has_children(&parents).into_iter().enumerate() {
                if has_children {
                    let action = if collapsed.contains(&items[index].key) { TVE_COLLAPSE } else { TVE_EXPAND };
                    controls::send(self.hwnd, TVM_EXPAND, action.0 as usize, handles[index].0);
                }
            }
            *self.handles.borrow_mut() = handles;
            controls::send(self.hwnd, WM_SETREDRAW, 1, 0);
            unsafe {
                let _ = InvalidateRect(Some(self.hwnd), None, true);
            }
        }
        *self.items.borrow_mut() = items;
        self.busy.set(false);
        !same
    }

    pub fn len(&self) -> usize {
        self.items.borrow().len()
    }

    pub fn key(&self, index: usize) -> Option<String> {
        self.items.borrow().get(index).map(|item| item.key.clone())
    }

    pub fn index_of(&self, key: &str) -> Option<usize> {
        self.items.borrow().iter().position(|item| item.key == key)
    }

    /// The selected row, by position.
    pub fn selected(&self) -> Option<usize> {
        let handle = controls::send(self.hwnd, TVM_GETNEXTITEM, TVGN_CARET as usize, 0);
        (handle != 0).then(|| self.index_of_handle(HTREEITEM(handle))).flatten()
    }

    /// The row an item handle is, from what it was inserted with.
    pub fn index_of_handle(&self, handle: HTREEITEM) -> Option<usize> {
        let mut tv = TVITEMEXW { mask: TVIF_PARAM, hItem: handle, ..Default::default() };
        let found = controls::send(self.hwnd, TVM_GETITEMW, 0, &raw mut tv as isize);
        (found != 0).then_some(tv.lParam.0 as usize).filter(|index| *index < self.len())
    }

    /// Selects a row and brings it into view. The selection is the screen reader's focus
    /// while the tree has focus, so this is where focus lands after a change.
    pub fn select(&self, index: usize) {
        let handle = self.handles.borrow().get(index).copied();
        if let Some(handle) = handle {
            controls::send(self.hwnd, TVM_SELECTITEM, TVGN_CARET as usize, handle.0);
            controls::send(self.hwnd, TVM_ENSUREVISIBLE, 0, handle.0);
        }
    }

    /// Selects a row without the notification counting as the person's doing: for putting
    /// the selection back where it was after a rebuild.
    pub fn select_quietly(&self, index: usize) {
        self.busy.set(true);
        self.select(index);
        self.busy.set(false);
    }

    /// Selects the row with this key, or else the one now at `near` — the same row if it is
    /// still listed, else whatever holds its place.
    pub fn select_key_or_near(&self, key: Option<&str>, near: Option<usize>) {
        let target = key.and_then(|key| self.index_of(key)).or_else(|| {
            let count = self.len();
            near.filter(|_| count > 0).map(|index| index.min(count - 1))
        });
        if let Some(index) = target {
            self.select(index);
        }
    }

    /// Remembers what the person collapsed, so a reload keeps it so.
    pub fn expansion_changed(&self, notice: &NMTREEVIEWW) {
        if let Some(index) = self.index_of_handle(notice.itemNew.hItem) {
            let Some(key) = self.key(index) else { return };
            let expanded = notice.itemNew.state.0 & windows::Win32::UI::Controls::TVIS_EXPANDED.0 != 0;
            let mut collapsed = self.collapsed.borrow_mut();
            if expanded {
                collapsed.remove(&key);
            } else {
                collapsed.insert(key);
            }
        }
    }

    /// The row under a point on screen, for a context menu opened with the mouse — which, in
    /// a tree view, does not select what was clicked.
    pub fn index_at(&self, screen: POINT) -> Option<usize> {
        let mut hit = TVHITTESTINFO { pt: screen, ..Default::default() };
        unsafe {
            let _ = ScreenToClient(self.hwnd, &mut hit.pt);
        }
        let handle = controls::send(self.hwnd, TVM_HITTEST, 0, &raw mut hit as isize);
        (handle != 0).then(|| self.index_of_handle(HTREEITEM(handle))).flatten()
    }

    /// Where on screen to open a row's context menu when it was asked for from the keyboard.
    pub fn menu_point(&self, index: usize) -> POINT {
        let handle = self.handles.borrow().get(index).copied();
        let mut rect = RECT::default();
        if let Some(handle) = handle {
            // The message takes the item in the rectangle it fills in.
            unsafe { std::ptr::write((&raw mut rect).cast::<HTREEITEM>(), handle) };
            controls::send(self.hwnd, TVM_GETITEMRECT, 1, &raw mut rect as isize);
        }
        let mut point = POINT { x: rect.left, y: rect.bottom };
        unsafe {
            let _ = ClientToScreen(self.hwnd, &mut point);
        }
        point
    }
}

/// A checkbox's state image: 1 unchecked, 2 checked, none for a row without one.
fn state_image(checked: Option<bool>) -> u32 {
    let image = match checked {
        None => 0,
        Some(false) => 1,
        Some(true) => 2,
    };
    image << 12
}

pub fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}
