//! `SysTreeView32`, which every list in the app is.
//!
//! The tree view reports each item's level, its position among its siblings and how many
//! there are, whether it is expanded, and — with checkboxes — whether it is checked, from its
//! own structure, through MSAA and UI Automation alike. So the core's `depth`, `index`,
//! `count`, `expanded` and `checked` become the tree's structure rather than words.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use lumenna_records::Swatch;

use windows::Win32::Foundation::{HWND, LPARAM, POINT, RECT};
use windows::Win32::Graphics::Gdi::{ClientToScreen, InvalidateRect, ScreenToClient};
use windows::Win32::UI::Controls::{
    HTREEITEM, SetWindowTheme, TVE_COLLAPSE, TVE_EXPAND, TVGN_CARET, TVHITTESTINFO,
    TVI_LAST, TVM_HITTEST,
    TVI_ROOT, TVIF_PARAM, TVIF_STATE, TVIF_TEXT, TVINSERTSTRUCTW, TVINSERTSTRUCTW_0,
    TVIF_IMAGE, TVIF_SELECTEDIMAGE, TVIS_STATEIMAGEMASK, TVITEMEXW, TVM_DELETEITEM, TVM_ENSUREVISIBLE, TVM_EXPAND,
    TVM_GETITEMRECT, TVM_GETITEMW, TVM_GETNEXTITEM, TVM_INSERTITEMW, TVM_MAPHTREEITEMTOACCID,
    TVM_SELECTITEM, TVM_SETEXTENDEDSTYLE, TVM_SETITEMW, TVS_CHECKBOXES, TVS_DISABLEDRAGDROP,
    TVS_EX_DOUBLEBUFFER, TVS_HASBUTTONS, TVS_HASLINES, TVS_LINESATROOT, TVS_NOHSCROLL, TVS_SHOWSELALWAYS,
    WC_TREEVIEWW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GWL_STYLE, GetWindowLongPtrW, SetWindowLongPtrW, WM_SETREDRAW, WS_EX_CLIENTEDGE, WS_TABSTOP,
};
use windows::core::{PWSTR, w};

use super::swatch::Marks;
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
    /// Whether what is under it is shown, where the app folds the list; `None` for a tree
    /// whose rows all stay open.
    pub expanded: Option<bool>,
    /// Its colour, drawn as a mark before the text and never part of it (`swatch.rs`).
    pub swatch: Option<Swatch>,
}

/// A tree view, and the rows it was last given.
pub struct Tree {
    pub hwnd: HWND,
    items: RefCell<Vec<Item>>,
    handles: RefCell<Vec<HTREEITEM>>,
    busy: Cell<bool>,
    marks: Rc<Marks>,
}

impl Tree {
    /// Creates one, named `name` for screen readers.
    pub fn create(parent: HWND, id: u16, name: &str, checkboxes: bool) -> Self {
        // No sideways scrolling: a long row is cut off on screen, with the tree's own tooltip
        // showing it whole, and is read whole either way.
        let style =
            TVS_HASBUTTONS | TVS_HASLINES | TVS_LINESATROOT | TVS_SHOWSELALWAYS | TVS_DISABLEDRAGDROP | TVS_NOHSCROLL;
        let hwnd = controls::create(parent, WC_TREEVIEWW, "", style | WS_TABSTOP.0, WS_EX_CLIENTEDGE.0, id);
        Self::adopt(hwnd, name, checkboxes)
    }

    /// Takes on a tree view that a dialog template made, as [`Tree::create`] makes one.
    pub fn adopt(hwnd: HWND, name: &str, checkboxes: bool) -> Self {
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
            busy: Cell::new(false),
            marks: Marks::new(hwnd),
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
        self.marks.show(items.iter().any(|item| item.swatch.is_some()));
        if same {
            let changed: Vec<(usize, Item)> = {
                let old = self.items.borrow();
                items.iter().enumerate().filter(|(i, item)| old[*i] != **item).map(|(i, item)| (i, item.clone())).collect()
            };
            let handles = self.handles.borrow().clone();
            for (index, item) in changed {
                let mut text = wide(&item.text);
                let image = self.marks.image(item.swatch.as_ref());
                let mut tv = TVITEMEXW {
                    mask: TVIF_TEXT | TVIF_STATE | TVIF_IMAGE | TVIF_SELECTEDIMAGE,
                    hItem: handles[index],
                    pszText: PWSTR(text.as_mut_ptr()),
                    state: state_image(item.checked),
                    stateMask: TVIS_STATEIMAGEMASK.0,
                    iImage: image,
                    iSelectedImage: image,
                    ..Default::default()
                };
                controls::send(self.hwnd, TVM_SETITEMW, 0, &raw mut tv as isize);
                // Folded or unfolded by the app: a no-op where the tree did it itself.
                if let Some(expanded) = item.expanded {
                    let action = if expanded { TVE_EXPAND } else { TVE_COLLAPSE };
                    controls::send(self.hwnd, TVM_EXPAND, action.0 as usize, handles[index].0);
                }
            }
        } else {
            controls::send(self.hwnd, WM_SETREDRAW, 0, 0);
            controls::send(self.hwnd, TVM_DELETEITEM, 0, TVI_ROOT.0);
            let depths: Vec<u32> = items.iter().map(|item| item.depth).collect();
            let parents = outline::parents(&depths);
            let mut handles: Vec<HTREEITEM> = Vec::with_capacity(items.len());
            for (index, item) in items.iter().enumerate() {
                let mut text = wide(&item.text);
                let image = self.marks.image(item.swatch.as_ref());
                let insert = TVINSERTSTRUCTW {
                    hParent: parents[index].map_or(TVI_ROOT, |parent| handles[parent]),
                    hInsertAfter: TVI_LAST,
                    Anonymous: TVINSERTSTRUCTW_0 {
                        itemex: TVITEMEXW {
                            mask: TVIF_TEXT | TVIF_PARAM | TVIF_STATE | TVIF_IMAGE | TVIF_SELECTEDIMAGE,
                            pszText: PWSTR(text.as_mut_ptr()),
                            lParam: LPARAM(index as isize),
                            state: state_image(item.checked),
                            stateMask: TVIS_STATEIMAGEMASK.0,
                            iImage: image,
                            iSelectedImage: image,
                            ..Default::default()
                        },
                    },
                };
                let handle = controls::send(self.hwnd, TVM_INSERTITEMW, 0, &raw const insert as isize);
                handles.push(HTREEITEM(handle));
            }
            // As the app folds it, else expanded; only once its children are in, since an item
            // without any cannot open.
            for (index, has_children) in outline::has_children(&parents).into_iter().enumerate() {
                if has_children {
                    let action = if items[index].expanded.unwrap_or(true) { TVE_EXPAND } else { TVE_COLLAPSE };
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

    /// Whether a row's checkbox is checked now: the tree checks one itself, on Space or a
    /// click.
    pub fn is_checked(&self, index: usize) -> bool {
        let Some(handle) = self.handles.borrow().get(index).copied() else { return false };
        let mut tv = TVITEMEXW { mask: TVIF_STATE, hItem: handle, stateMask: TVIS_STATEIMAGEMASK.0, ..Default::default() };
        controls::send(self.hwnd, TVM_GETITEMW, 0, &raw mut tv as isize);
        tv.state & TVIS_STATEIMAGEMASK.0 == state_image(Some(true))
    }

    /// Describes a row: what a screen reader says after it, never as part of its name.
    pub fn describe(&self, index: usize, description: &str) {
        let Some(handle) = self.handles.borrow().get(index).copied() else { return };
        let child = controls::send(self.hwnd, TVM_MAPHTREEITEMTOACCID, handle.0 as usize, 0);
        a11y::set_item_description(self.hwnd, child as u32, description);
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
