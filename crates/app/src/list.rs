//! A listing as shown: folded, its level said where it changes, and which row is in hand.

use std::collections::BTreeSet;

use lumenna_records::{Action, ActionKind, Question, RowView, Rows};
pub use lumenna_records::{Asks, Offered};
use serde::{Deserialize, Serialize};

/// One row of a list.
#[boltffi::data]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Item {
    /// The row, `expanded` set when something is under it. In a view its `actions` are empty:
    /// they are `actions`, in short.
    pub row: RowView,
    /// Folded away under a collapsed row. A tree control that folds by itself takes every row
    /// and the expanded states; a flat list shows only the rows not hidden.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub hidden: bool,
    /// "level 2", where the level differs from the row shown before; depth said as
    /// indentation says nothing in speech, and said on every row it says too much.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub level: Option<String>,
    /// The row's actions, in order, as places in the list's `offered`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub actions: Vec<u32>,
}

/// The actions `items` offer, each once, and each item's as places among them.
pub fn offer(items: &mut [Item]) -> Vec<Offered> {
    let mut offered: Vec<Offered> = Vec::new();
    let mut places: std::collections::HashMap<Offered, u32> = std::collections::HashMap::new();
    for item in items {
        item.actions = item
            .row
            .actions
            .iter()
            .map(|action| {
                let short = Offered::of(action);
                *places.entry(short.clone()).or_insert_with(|| {
                    offered.push(short);
                    u32::try_from(offered.len() - 1).unwrap_or(u32::MAX)
                })
            })
            .collect();
    }
    offered
}

/// Every row of `rows`, with what `folded` names folded away and marked hidden.
#[must_use]
pub fn items(rows: &Rows, folded: &BTreeSet<String>) -> Vec<Item> {
    let mut items: Vec<Item> = Vec::with_capacity(rows.rows.len());
    let mut hidden_below: Option<u32> = None;
    let mut previous = 0;
    for (index, row) in rows.rows.iter().enumerate() {
        if hidden_below.is_some_and(|depth| row.depth <= depth) {
            hidden_below = None;
        }
        let hidden = hidden_below.is_some();
        let parent = rows.rows.get(index + 1).is_some_and(|next| next.depth > row.depth);
        let collapsed = parent && folded.contains(&row.id);
        if collapsed && !hidden {
            hidden_below = Some(row.depth);
        }
        let mut row = row.clone();
        if parent {
            row.expanded = Some(!collapsed);
            row.actions.push(fold_action(&row, collapsed));
        }
        let level = (!hidden && row.depth != previous).then(|| lumenna_records::words::level(row.depth));
        if !hidden {
            previous = row.depth;
        }
        items.push(Item { row, hidden, level, actions: Vec::new() });
    }
    items
}

/// Expand or Collapse, for a row with something under it.
fn fold_action(row: &RowView, collapsed: bool) -> Action {
    let subject = row.actions.first().map_or(lumenna_records::Subject::Task, |a| a.subject);
    let (kind, title) = if collapsed { (ActionKind::Expand, "Expand") } else { (ActionKind::Collapse, "Collapse") };
    let mut action = Action::new(kind, subject, title, row.id.clone());
    action.question = Question::Immediate;
    action
}

/// The row to hold after the list changed: the same one if it is still shown, else whichever
/// now holds its place among the rows shown, else nothing. Positions count shown rows only.
#[must_use]
pub fn settle(items: &[Item], focus: Option<&str>, position: usize) -> Option<(String, usize)> {
    let shown: Vec<&Item> = items.iter().filter(|item| !item.hidden).collect();
    if let Some(id) = focus
        && let Some(at) = shown.iter().position(|item| item.row.id == id)
    {
        return Some((id.to_owned(), at));
    }
    let at = position.min(shown.len().checked_sub(1)?);
    Some((shown[at].row.id.clone(), at))
}

/// Where `id` is among the rows shown.
#[must_use]
pub fn position(items: &[Item], id: &str) -> Option<usize> {
    items.iter().filter(|item| !item.hidden).position(|item| item.row.id == id)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(id: &str, depth: u32) -> RowView {
        RowView { id: id.to_owned(), depth, title: id.to_owned(), ..RowView::default() }
    }

    fn rows(list: &[(&str, u32)]) -> Rows {
        Rows { rows: list.iter().map(|(id, depth)| row(id, *depth)).collect(), ..Rows::default() }
    }

    #[test]
    fn a_folded_row_hides_everything_under_it_and_nothing_after() {
        let rows = rows(&[("a", 0), ("b", 1), ("c", 2), ("d", 0)]);
        let folded = BTreeSet::from(["a".to_owned()]);
        let shown: Vec<_> = items(&rows, &folded).into_iter().filter(|i| !i.hidden).map(|i| i.row.id).collect();
        assert_eq!(shown, ["a", "d"]);
    }

    #[test]
    fn the_level_is_said_only_where_it_changes_from_the_row_shown_before() {
        let rows = rows(&[("a", 0), ("b", 1), ("c", 1), ("d", 0)]);
        let levels: Vec<_> = items(&rows, &BTreeSet::new()).into_iter().map(|i| i.level).collect();
        assert_eq!(levels, [None, Some("level 2".to_owned()), None, Some("level 1".to_owned())]);
    }

    #[test]
    fn the_level_after_a_fold_is_said_against_the_row_shown_before() {
        let rows = rows(&[("a", 0), ("b", 1), ("c", 2), ("d", 1)]);
        let folded = BTreeSet::from(["b".to_owned()]);
        let items = items(&rows, &folded);
        assert!(items[2].hidden);
        assert_eq!(items[2].level, None);
        assert_eq!(items[3].level, None, "level 2, as b was");
    }

    #[test]
    fn a_row_with_something_under_it_offers_collapse_and_says_it_is_expanded() {
        let rows = rows(&[("a", 0), ("b", 1)]);
        let shown = items(&rows, &BTreeSet::new());
        assert_eq!(shown[0].row.expanded, Some(true));
        assert_eq!(shown[0].row.actions.last().map(|a| a.kind), Some(ActionKind::Collapse));
        assert_eq!(shown[1].row.expanded, None);
    }

    #[test]
    fn focus_stays_on_its_row_or_goes_to_whatever_holds_its_place() {
        let items = items(&rows(&[("a", 0), ("c", 0)]), &BTreeSet::new());
        assert_eq!(settle(&items, Some("c"), 0), Some(("c".to_owned(), 1)));
        assert_eq!(settle(&items, Some("b"), 1), Some(("c".to_owned(), 1)));
        assert_eq!(settle(&items, Some("z"), 5), Some(("c".to_owned(), 1)));
        assert_eq!(settle(&[], Some("a"), 0), None);
    }
}
