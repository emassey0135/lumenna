//! The tree every list in the app is: a `GtkListView` over a `GtkTreeListModel`, annotated so
//! a screen reader is told it is a tree (§16.3).
//!
//! Left alone, GTK 4 reports a list view as a flat list: each row's position is counted over
//! every visible row ("3 of 60"), and the level is on the expander button inside the row —
//! one too deep for a leaf, two for a leaf under a leaf's sibling. Orca says levels only
//! inside a tree, and reads what has focus. So the view is made with the `tree` role, the
//! focusable thing in each row is a `TreeExpander` made with the `tree item` role, and each
//! is given its level, its position among its siblings and their number. Expansion is the
//! expander's own state, which GTK keeps right. The `GtkTreeView` that GTK 4 deprecated was no
//! alternative: it reports no rows at all.
//!
//! What the tree reports is left out of each row's text (`speech`): saying it in words as
//! well would say it twice. The checked state is not reported — Orca does not read it on a
//! tree item — so a done task says "completed" in its text.
//!
//! GTK binds no keys to expanding and collapsing a row in a list view, so the tree does what
//! every other tree does: Right expands, or goes to the first child; Left collapses, or goes
//! to the parent.

use std::cell::{Cell, RefCell};
use std::collections::HashSet;
use std::rc::{Rc, Weak};

use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::{gdk, gio, glib};
use lumenna_desktop::outline;

/// One row as the tree shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Item {
    /// What identifies it across a reload.
    pub key: String,
    /// The line as read.
    pub text: String,
    /// How deep the core put it; the tree nests it under the nearest shallower row above.
    pub depth: u32,
}

mod imp {
    use std::cell::{Cell, RefCell};

    use gtk::glib;
    use gtk::glib::Properties;
    use gtk::prelude::*;
    use gtk::subclass::prelude::*;

    /// One row's object in the model.
    #[derive(Default, Properties)]
    #[properties(wrapper_type = super::Row)]
    pub struct Row {
        /// The line as read, which the row's label shows.
        #[property(get, set)]
        pub text: RefCell<String>,
        /// Where it is in the flat list the tree was given.
        pub index: Cell<usize>,
        /// Its position among its siblings, from one, and how many siblings there are.
        pub position: Cell<u32>,
        pub siblings: Cell<u32>,
        /// What is under it, if anything.
        pub children: RefCell<Option<gtk::gio::ListStore>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Row {
        const NAME: &'static str = "LumennaRow";
        type Type = super::Row;
    }

    #[glib::derived_properties]
    impl ObjectImpl for Row {}
}

glib::wrapper! {
    /// One row's object in the model.
    pub struct Row(ObjectSubclass<imp::Row>);
}

impl Row {
    fn new(text: &str, index: usize) -> Self {
        let row: Self = glib::Object::builder().property("text", text).build();
        row.imp().index.set(index);
        row
    }

    fn index(&self) -> usize {
        self.imp().index.get()
    }
}

thread_local! {
    /// How many moves of focus are waiting for their row's widget.
    static PENDING: Cell<u32> = const { Cell::new(0) };
    /// What runs once they have all landed.
    static SETTLED: RefCell<Vec<Box<dyn FnOnce()>>> = RefCell::new(Vec::new());
}

/// Runs `action` once focus has landed wherever it is on its way to — at once, if it is not
/// on its way anywhere. `App::say` waits this way, so an announcement follows the row focus
/// lands on rather than being cut off by it: Orca reads an announcement as a message, and a
/// focus change after it would interrupt it.
pub fn when_settled(action: impl FnOnce() + 'static) {
    if PENDING.with(Cell::get) == 0 {
        action();
    } else {
        SETTLED.with(|settled| settled.borrow_mut().push(Box::new(action)));
    }
}

fn settle() {
    let left = PENDING.with(|pending| {
        pending.set(pending.get().saturating_sub(1));
        pending.get()
    });
    if left == 0 {
        for action in SETTLED.with(|settled| settled.take()) {
            action();
        }
    }
}

/// Tries to put focus and the selection on a visible position, and says whether focus is
/// there now.
fn land(view: &gtk::ListView, position: u32) -> bool {
    view.scroll_to(position, gtk::ListScrollFlags::FOCUS | gtk::ListScrollFlags::SELECT, None);
    let focus_within = |view: &gtk::ListView| {
        view.root().and_then(|root| root.focus()).filter(|focus| focus.is_ancestor(view))
    };
    if focus_within(view).is_none() {
        // Scrolling moves focus within the list, but not into it from elsewhere; the list
        // takes it onto the item just scrolled to.
        view.grab_focus();
    }
    let Some(focus) = focus_within(view) else { return false };
    let expander = match focus.clone().downcast::<gtk::TreeExpander>() {
        Ok(expander) => Some(expander),
        Err(_) => focus.ancestor(gtk::TreeExpander::static_type()).and_downcast::<gtk::TreeExpander>(),
    };
    let wanted = view
        .model()
        .and_then(|selection| selection.item(position))
        .and_downcast::<gtk::TreeListRow>();
    expander.and_then(|expander| expander.list_row()).is_some_and(|row| Some(row) == wanted)
}

/// Which key went to a row, for the view that owns the tree.
pub type KeyHandler = dyn Fn(gdk::Key, gdk::ModifierType, usize) -> glib::Propagation;

/// Where a row's context menu was asked for: its flat index, and the point of a right-click.
pub type MenuHandler = dyn Fn(usize, Option<(f64, f64)>);

/// Where a view's handler for something the tree reports is kept.
type Callback<F> = RefCell<Option<Box<F>>>;

/// A tree, and the rows it was last given.
pub struct Tree {
    pub view: gtk::ListView,
    /// The view inside its scrolled window, which is what goes in a layout.
    pub widget: gtk::ScrolledWindow,
    root: gio::ListStore,
    model: gtk::TreeListModel,
    selection: gtk::SingleSelection,
    items: RefCell<Vec<Item>>,
    rows: RefCell<Vec<Row>>,
    collapsed: RefCell<HashSet<String>>,
    busy: Cell<bool>,
    /// Whether focus was on the tree when it was last rebuilt, until the selection is put back.
    had_focus: Cell<bool>,
    /// The visible position focus is on its way to, while its row has no widget yet.
    target: Rc<Cell<Option<u32>>>,
    on_selected: Callback<dyn Fn(Option<usize>)>,
    on_activate: Callback<dyn Fn(usize)>,
    on_key: Callback<KeyHandler>,
    on_menu: Callback<MenuHandler>,
}

impl Tree {
    /// Creates one, named `name` for screen readers.
    pub fn new(name: &str) -> Rc<Self> {
        let root = gio::ListStore::new::<Row>();
        let model = gtk::TreeListModel::new(root.clone(), false, false, |item| {
            let row = item.downcast_ref::<Row>()?;
            let children = row.imp().children.borrow().clone()?;
            Some(children.upcast())
        });
        let selection = gtk::SingleSelection::builder().model(&model).autoselect(true).can_unselect(false).build();
        let factory = gtk::SignalListItemFactory::new();
        factory.connect_setup(|_, item| {
            let Some(item) = item.downcast_ref::<gtk::ListItem>() else { return };
            // The row's own widget would be a list item, positioned among every visible row;
            // the tree item inside it takes focus instead.
            item.set_focusable(false);
            let label = gtk::Label::builder().xalign(0.0).ellipsize(gtk::pango::EllipsizeMode::End).build();
            let expander = gtk::TreeExpander::builder()
                .accessible_role(gtk::AccessibleRole::TreeItem)
                .focusable(true)
                .child(&label)
                .build();
            item.set_child(Some(&expander));
            item.property_expression("item")
                .chain_property::<gtk::TreeListRow>("item")
                .chain_property::<Row>("text")
                .bind(&label, "label", gtk::Widget::NONE);
        });
        factory.connect_bind(|_, item| {
            let Some(item) = item.downcast_ref::<gtk::ListItem>() else { return };
            let Some(list_row) = item.item().and_downcast::<gtk::TreeListRow>() else { return };
            let Some(expander) = item.child().and_downcast::<gtk::TreeExpander>() else { return };
            expander.set_list_row(Some(&list_row));
            let Some(row) = list_row.item().and_downcast::<Row>() else { return };
            let level = i32::try_from(list_row.depth()).unwrap_or(i32::MAX - 1) + 1;
            expander.update_property(&[gtk::accessible::Property::Level(level)]);
            expander.update_relation(&[
                gtk::accessible::Relation::PosInSet(row.imp().position.get() as i32),
                gtk::accessible::Relation::SetSize(row.imp().siblings.get() as i32),
            ]);
        });
        factory.connect_unbind(|_, item| {
            if let Some(expander) = item.downcast_ref::<gtk::ListItem>().and_then(|i| i.child()).and_downcast::<gtk::TreeExpander>() {
                expander.set_list_row(None);
            }
        });
        let view = gtk::ListView::builder()
            .accessible_role(gtk::AccessibleRole::Tree)
            .model(&selection)
            .factory(&factory)
            .build();
        view.update_property(&[gtk::accessible::Property::Label(name)]);
        let widget = gtk::ScrolledWindow::builder()
            .child(&view)
            .hscrollbar_policy(gtk::PolicyType::Never)
            .vexpand(true)
            .hexpand(true)
            .has_frame(true)
            .build();
        let tree = Rc::new(Self {
            view,
            widget,
            root,
            model,
            selection,
            items: RefCell::new(Vec::new()),
            rows: RefCell::new(Vec::new()),
            collapsed: RefCell::new(HashSet::new()),
            busy: Cell::new(false),
            had_focus: Cell::new(false),
            target: Rc::new(Cell::new(None)),
            on_selected: RefCell::new(None),
            on_activate: RefCell::new(None),
            on_key: RefCell::new(None),
            on_menu: RefCell::new(None),
        });
        tree.connect_signals();
        tree
    }

    fn connect_signals(self: &Rc<Self>) {
        let weak = Rc::downgrade(self);
        self.selection.connect_selected_item_notify(move |_| {
            let Some(tree) = weak.upgrade() else { return };
            if tree.busy.get() {
                return;
            }
            let selected = tree.selected();
            if let Some(callback) = tree.on_selected.borrow().as_ref() {
                callback(selected);
            }
        });

        let weak = Rc::downgrade(self);
        self.view.connect_activate(move |_, position| {
            let Some(tree) = weak.upgrade() else { return };
            if let Some(index) = tree.index_at(position)
                && let Some(callback) = tree.on_activate.borrow().as_ref() {
                    callback(index);
                }
        });

        // In the capture phase, so the tree sees a key before the row it is going to.
        let keys = gtk::EventControllerKey::new();
        keys.set_propagation_phase(gtk::PropagationPhase::Capture);
        let weak: Weak<Self> = Rc::downgrade(self);
        keys.connect_key_pressed(move |_, key, _, modifiers| {
            let Some(tree) = weak.upgrade() else { return glib::Propagation::Proceed };
            tree.key_pressed(key, modifiers)
        });
        self.view.add_controller(keys);

        let click = gtk::GestureClick::builder().button(gdk::BUTTON_SECONDARY).build();
        let weak = Rc::downgrade(self);
        click.connect_pressed(move |gesture, _, x, y| {
            let Some(tree) = weak.upgrade() else { return };
            let Some(position) = tree.position_at(x, y) else { return };
            gesture.set_state(gtk::EventSequenceState::Claimed);
            tree.view.scroll_to(position, gtk::ListScrollFlags::FOCUS | gtk::ListScrollFlags::SELECT, None);
            if let (Some(index), Some(callback)) = (tree.index_at(position), tree.on_menu.borrow().as_ref()) {
                callback(index, Some((x, y)));
            }
        });
        self.view.add_controller(click);
    }

    fn key_pressed(&self, key: gdk::Key, modifiers: gdk::ModifierType) -> glib::Propagation {
        let Some(list_row) = self.selection.selected_item().and_downcast::<gtk::TreeListRow>() else {
            return glib::Propagation::Proceed;
        };
        let position = list_row.position();
        let plain = !modifiers.intersects(
            gdk::ModifierType::CONTROL_MASK | gdk::ModifierType::ALT_MASK | gdk::ModifierType::SHIFT_MASK,
        );
        match key {
            gdk::Key::Right | gdk::Key::KP_Right if plain => {
                if list_row.is_expandable() && !list_row.is_expanded() {
                    list_row.set_expanded(true);
                } else if list_row.is_expanded() {
                    self.move_to(position + 1);
                }
                return glib::Propagation::Stop;
            }
            gdk::Key::Left | gdk::Key::KP_Left if plain => {
                if list_row.is_expanded() {
                    list_row.set_expanded(false);
                } else if let Some(parent) = list_row.parent() {
                    self.move_to(parent.position());
                }
                return glib::Propagation::Stop;
            }
            gdk::Key::Menu => return self.menu(position),
            gdk::Key::F10 if modifiers.contains(gdk::ModifierType::SHIFT_MASK) => return self.menu(position),
            _ => {}
        }
        let Some(index) = self.index_at(position) else { return glib::Propagation::Proceed };
        match self.on_key.borrow().as_ref() {
            Some(callback) => callback(key, modifiers, index),
            None => glib::Propagation::Proceed,
        }
    }

    fn menu(&self, position: u32) -> glib::Propagation {
        if let (Some(index), Some(callback)) = (self.index_at(position), self.on_menu.borrow().as_ref()) {
            callback(index, None);
        }
        glib::Propagation::Stop
    }

    /// Moves focus and the selection to a visible position.
    ///
    /// A row has no widget until GTK lays the list out — after a rebuild, or in a list just
    /// made — and until then focus cannot go to it: it stays on whatever row holds the old
    /// one's place, or outside the list. So when the first try does not land, it is tried
    /// again each frame until it does. Announcements wait for it (`when_settled`).
    fn move_to(&self, position: u32) {
        let chasing = self.target.get().is_some();
        self.target.set(Some(position));
        if chasing {
            // A move already waiting for its row goes to this one instead.
            return;
        }
        if land(&self.view, position) {
            self.target.set(None);
            return;
        }
        PENDING.with(|pending| pending.set(pending.get() + 1));
        let frames = Cell::new(0);
        let target = Rc::clone(&self.target);
        self.view.add_tick_callback(move |view, _| {
            frames.set(frames.get() + 1);
            let landed = target.get().is_none_or(|position| land(view, position));
            if landed || frames.get() >= 30 {
                target.set(None);
                settle();
                return glib::ControlFlow::Break;
            }
            glib::ControlFlow::Continue
        });
    }

    /// The visible row under a point in the view's coordinates.
    fn position_at(&self, x: f64, y: f64) -> Option<u32> {
        let picked = self.view.pick(x, y, gtk::PickFlags::DEFAULT)?;
        let expander = if let Ok(expander) = picked.clone().downcast::<gtk::TreeExpander>() {
            expander
        } else {
            picked.ancestor(gtk::TreeExpander::static_type()).and_downcast::<gtk::TreeExpander>()?
        };
        expander.list_row().map(|row| row.position())
    }

    /// The flat index of the row at a visible position.
    fn index_at(&self, position: u32) -> Option<usize> {
        let list_row = self.model.item(position).and_downcast::<gtk::TreeListRow>()?;
        list_row.item().and_downcast::<Row>().map(|row| row.index())
    }

    /// The visible position of a flat index, or of its nearest visible ancestor when it is
    /// inside something collapsed.
    fn position_of(&self, index: usize) -> Option<u32> {
        let items = self.items.borrow();
        let depths: Vec<u32> = items.iter().map(|item| item.depth).collect();
        let parents = outline::parents(&depths);
        let mut target = Some(index);
        while let Some(wanted) = target {
            for position in 0..self.model.n_items() {
                if self.index_at(position) == Some(wanted) {
                    return Some(position);
                }
            }
            target = parents.get(wanted).copied().flatten();
        }
        None
    }

    /// Calls `callback` with the flat index of the row selected, as the selection moves.
    pub fn connect_selected(&self, callback: impl Fn(Option<usize>) + 'static) {
        *self.on_selected.borrow_mut() = Some(Box::new(callback));
    }

    /// Calls `callback` when a row is activated: Enter, or a double click.
    pub fn connect_activate(&self, callback: impl Fn(usize) + 'static) {
        *self.on_activate.borrow_mut() = Some(Box::new(callback));
    }

    /// Offers every key on a row, other than the tree's own, to `callback` first.
    pub fn connect_key(&self, callback: impl Fn(gdk::Key, gdk::ModifierType, usize) -> glib::Propagation + 'static) {
        *self.on_key.borrow_mut() = Some(Box::new(callback));
    }

    /// Calls `callback` when a row's context menu is asked for: Shift+F10 or the Menu key,
    /// with no point, or a right-click, at a point in the view.
    pub fn connect_menu(&self, callback: impl Fn(usize, Option<(f64, f64)>) + 'static) {
        *self.on_menu.borrow_mut() = Some(Box::new(callback));
    }

    /// Shows `items`. When they are the same rows as before — the same keys at the same
    /// depths, in the same order — each row's text is updated where it stands, so the row
    /// in focus is not read again. Otherwise the tree is rebuilt, keeping what was collapsed,
    /// and the caller puts the selection back. Returns whether it was rebuilt.
    pub fn set(&self, items: Vec<Item>) -> bool {
        let same = {
            let old = self.items.borrow();
            old.len() == items.len() && old.iter().zip(&items).all(|(a, b)| a.key == b.key && a.depth == b.depth)
        };
        if same {
            for (row, item) in self.rows.borrow().iter().zip(&items) {
                if row.text() != item.text {
                    row.set_text(item.text.clone());
                }
            }
            *self.items.borrow_mut() = items;
            return false;
        }
        self.remember_expansion();
        self.had_focus.set(self.has_focus());
        self.busy.set(true);
        let depths: Vec<u32> = items.iter().map(|item| item.depth).collect();
        let parents = outline::parents(&depths);
        let rows: Vec<Row> = items.iter().enumerate().map(|(index, item)| Row::new(&item.text, index)).collect();
        let mut tops = Vec::new();
        let mut children: Vec<Vec<Row>> = vec![Vec::new(); rows.len()];
        for (index, parent) in parents.iter().enumerate() {
            match parent {
                Some(parent) => children[*parent].push(rows[index].clone()),
                None => tops.push(rows[index].clone()),
            }
        }
        let number = |siblings: &[Row]| {
            let count = u32::try_from(siblings.len()).unwrap_or(u32::MAX);
            for (position, row) in siblings.iter().enumerate() {
                row.imp().position.set(u32::try_from(position).unwrap_or(u32::MAX - 1) + 1);
                row.imp().siblings.set(count);
            }
        };
        number(&tops);
        for (index, under) in children.iter().enumerate() {
            if under.is_empty() {
                continue;
            }
            number(under);
            let store = gio::ListStore::new::<Row>();
            store.extend_from_slice(under);
            *rows[index].imp().children.borrow_mut() = Some(store);
        }
        *self.items.borrow_mut() = items;
        *self.rows.borrow_mut() = rows;
        self.root.splice(0, self.root.n_items(), &tops);
        self.expand();
        self.busy.set(false);
        true
    }

    /// Notes which visible rows are collapsed and which expanded, before a rebuild. Rows out
    /// of sight keep what was noted last.
    fn remember_expansion(&self) {
        let items = self.items.borrow();
        let mut collapsed = self.collapsed.borrow_mut();
        for position in 0..self.model.n_items() {
            let Some(list_row) = self.model.item(position).and_downcast::<gtk::TreeListRow>() else { continue };
            if !list_row.is_expandable() {
                continue;
            }
            let Some(item) = list_row.item().and_downcast::<Row>().and_then(|row| items.get(row.index()).cloned()) else {
                continue;
            };
            if list_row.is_expanded() {
                collapsed.remove(&item.key);
            } else {
                collapsed.insert(item.key);
            }
        }
    }

    /// Expands every row but those collapsed, top to bottom, so children are reached too.
    fn expand(&self) {
        let items = self.items.borrow();
        let collapsed = self.collapsed.borrow();
        let mut position = 0;
        while position < self.model.n_items() {
            if let Some(list_row) = self.model.item(position).and_downcast::<gtk::TreeListRow>() {
                let key = list_row.item().and_downcast::<Row>().and_then(|row| items.get(row.index()).map(|i| &i.key));
                if list_row.is_expandable() && key.is_some_and(|key| !collapsed.contains(key)) {
                    list_row.set_expanded(true);
                }
            }
            position += 1;
        }
    }

    /// The flat index of the row selected.
    pub fn selected(&self) -> Option<usize> {
        let list_row = self.selection.selected_item().and_downcast::<gtk::TreeListRow>()?;
        list_row.item().and_downcast::<Row>().map(|row| row.index())
    }

    /// The key of the row at a flat index.
    pub fn key(&self, index: usize) -> Option<String> {
        self.items.borrow().get(index).map(|item| item.key.clone())
    }

    /// How many rows the tree holds, shown or not.
    pub fn len(&self) -> usize {
        self.items.borrow().len()
    }

    /// Whether focus is on the tree.
    pub fn has_focus(&self) -> bool {
        self.view.root().and_then(|root| root.focus()).is_some_and(|focus| focus.is_ancestor(&self.view))
    }

    /// Selects the row at a flat index, moving focus to it if focus is on the tree, and
    /// tells whoever is listening.
    pub fn select(&self, index: usize) {
        self.select_quietly(index);
        if let Some(callback) = self.on_selected.borrow().as_ref() {
            callback(self.selected());
        }
    }

    /// Selects the row at a flat index, moving focus to it if focus is on the tree, without
    /// telling anyone: for putting back a selection that never meant to change.
    ///
    /// The selection moves at once; focus follows once the row has a widget (`move_to`).
    pub fn select_quietly(&self, index: usize) {
        let Some(position) = self.position_of(index) else { return };
        self.busy.set(true);
        self.view.scroll_to(position, gtk::ListScrollFlags::SELECT, None);
        // Replacing the rows takes focus out of the list for a moment, so a tree that had it
        // before a rebuild still counts as having it.
        if self.has_focus() || self.had_focus.take() || self.target.get().is_some() {
            self.move_to(position);
        }
        self.busy.set(false);
    }

    /// The flat index of the row with `key`.
    pub fn index_of(&self, key: &str) -> Option<usize> {
        self.items.borrow().iter().position(|item| item.key == key)
    }

    /// Selects the row with `key` if there still is one, else the row now at `near` — or the
    /// last, if the list got shorter (§13: focus goes somewhere sensible, never nowhere).
    pub fn select_key_or_near(&self, key: Option<&str>, near: Option<usize>) {
        let count = self.len();
        if count == 0 {
            return;
        }
        let found = key.and_then(|key| self.items.borrow().iter().position(|item| item.key == key));
        if let Some(index) = found.or_else(|| near.map(|near| near.min(count - 1))) {
            self.select(index);
        }
    }

    /// Moves focus to the tree: to the row selected, or the first.
    pub fn focus(&self) {
        match self.selection.selected_item().and_downcast::<gtk::TreeListRow>() {
            Some(list_row) => self.move_to(list_row.position()),
            None if self.model.n_items() > 0 => self.move_to(0),
            None => {
                self.view.grab_focus();
            }
        }
    }
}
