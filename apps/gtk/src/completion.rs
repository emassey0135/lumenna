//! Offering what could be typed next in a quick-add or filter field.
//!
//! Down arrow — or Ctrl+Space, as in an IDE — asks the core what fits at the cursor and opens
//! the candidates as a menu beside the cursor. Tab is never taken: it is how a screen reader
//! user leaves the field. Nothing opens by itself after a `#` or `@`, since typing a name
//! straight through is common and a menu would interrupt it.
//!
//! A menu, as on Windows, rather than a combobox: GTK's own popover menu, each item read
//! with its position and count, its first item focused as it opens, and focus back in the
//! field after it. Each item leads with its name.

use gtk::prelude::*;
use gtk::{gdk, gio, glib};
use lumenna_ffi::ShellEvent;
use lumenna_records::Syntax;

/// What a field that completes adds to its description, so the way in can be found.
pub const HINT: &str = "Down arrow offers what could come next.";

/// Where an entry's completions come from.
#[derive(Clone, Copy)]
pub enum Source {
    /// The filter above a list: asked of the store at keystroke rate (`App::complete`).
    Filter,
    /// The app's quick-add line, whose offer is the app's and whose choice it splices in.
    QuickAdd,
}

/// Makes an entry offer completions.
pub fn attach(entry: &gtk::Entry, source: Source) {
    let keys = gtk::EventControllerKey::new();
    // Before the entry's own keys: Down would otherwise be the window's, to move focus.
    keys.set_propagation_phase(gtk::PropagationPhase::Capture);
    let weak = entry.downgrade();
    keys.connect_key_pressed(move |_, key, _, modifiers| {
        let Some(entry) = weak.upgrade() else { return glib::Propagation::Proceed };
        let control = modifiers.contains(gdk::ModifierType::CONTROL_MASK);
        let plain = !modifiers.intersects(
            gdk::ModifierType::CONTROL_MASK | gdk::ModifierType::ALT_MASK | gdk::ModifierType::SHIFT_MASK,
        );
        let asked = match key {
            gdk::Key::Down | gdk::Key::KP_Down => plain,
            gdk::Key::space => control && !modifiers.contains(gdk::ModifierType::ALT_MASK),
            _ => false,
        };
        if !asked {
            return glib::Propagation::Proceed;
        }
        offer(&entry, source);
        glib::Propagation::Stop
    });
    entry.add_controller(keys);
}

/// The byte offset in `text` of a position counted in characters, as GTK counts them.
fn byte_at(text: &str, chars: usize) -> usize {
    text.char_indices().nth(chars).map_or(text.len(), |(at, _)| at)
}

/// The character position of a byte offset, as the core sends spans: UTF-8 bytes.
fn char_at(text: &str, bytes: usize) -> i32 {
    i32::try_from(text[..bytes.min(text.len())].chars().count()).unwrap_or(i32::MAX)
}

/// Asks what fits at the cursor and offers it.
fn offer(entry: &gtk::Entry, source: Source) {
    let Some(app) = crate::window::app() else { return };
    let text = entry.text().to_string();
    let cursor = u32::try_from(byte_at(&text, usize::try_from(entry.position()).unwrap_or(0))).unwrap_or(u32::MAX);
    let found = match source {
        Source::Filter => match app.model.complete(text, cursor, Syntax::Filter) {
            Ok(found) => found,
            Err(_) => return,
        },
        // The line where the cursor is now, then what the app offers there.
        Source::QuickAdd => {
            app.model.send(ShellEvent::Typing(lumenna_app::Typing::Text { text, cursor }));
            match app.model.current().0.line {
                Some(line) => line.offer,
                None => return,
            }
        }
    };

    let menu = gio::Menu::new();
    let actions = gio::SimpleActionGroup::new();
    if found.candidates.is_empty() {
        // Said as a menu too, so the answer comes the same way either way. An item whose
        // action does not exist is shown greyed.
        menu.append(Some(&found.announcement.replace('_', "__")), Some("row.nothing"));
    }
    for (index, candidate) in found.candidates.iter().enumerate() {
        let item = gio::MenuItem::new(Some(&lumenna_app::speech::candidate(candidate).replace('_', "__")), None);
        item.set_action_and_target_value(Some("row.complete"), Some(&(index as u32).to_variant()));
        menu.append_item(&item);
    }
    let complete = gio::SimpleAction::new("complete", Some(glib::VariantTy::UINT32));
    let weak = entry.downgrade();
    let (start, end) = (found.start as usize, found.end as usize);
    let candidates: Vec<String> = found.candidates.iter().map(|c| c.text.clone()).collect();
    complete.connect_activate(move |_, parameter| {
        let (Some(entry), Some(index)) = (weak.upgrade(), parameter.and_then(|p| p.get::<u32>())) else { return };
        // The app's line: the app puts the choice in, and the line is drawn as it says.
        if let Source::QuickAdd = source {
            if let Some(app) = crate::window::app() {
                app.model.send(ShellEvent::Typing(lumenna_app::Typing::Choose(index)));
            }
            return;
        }
        let Some(inserted) = candidates.get(index as usize) else { return };
        let text = entry.text().to_string();
        let (from, to) = (char_at(&text, start), char_at(&text, end));
        // Replaced in place, so the entry's own undo takes it back and the field hears of the
        // change as if it had been typed.
        entry.delete_text(from, to);
        let mut position = from;
        entry.insert_text(inserted, &mut position);
        entry.set_position(position);
    });
    actions.add_action(&complete);

    // Beside the cursor, as far as GTK will say where it is.
    let x = entry
        .delegate()
        .and_downcast::<gtk::Text>()
        .map(|text| text.compute_cursor_extents(entry.position() as usize).0)
        .map_or(0.0, |strong| f64::from(strong.x()));
    let y = f64::from(entry.height());
    app.popup(&menu, entry.upcast_ref(), Some((x, y)), Some(&actions));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spans_in_bytes_and_cursors_in_characters_meet() {
        let text = "café #Wo";
        assert_eq!(byte_at(text, 4), 5, "é is two bytes");
        assert_eq!(char_at(text, 5), 4);
        assert_eq!(byte_at(text, 99), text.len());
    }
}
