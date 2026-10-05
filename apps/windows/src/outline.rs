//! The flat, depth-first rows the core sends, as a tree.

/// Each row's parent, by position: the nearest row above it that is shallower.
///
/// A row whose parent was filtered out — a matching subtask under a task that does not match
/// — has no shallower row above it in its branch, and sits at the top rather than under
/// whatever happens to precede it. That is the same rule the Apple apps' outlines use.
pub fn parents(depths: &[u32]) -> Vec<Option<usize>> {
    let mut chain: Vec<usize> = Vec::new();
    depths
        .iter()
        .enumerate()
        .map(|(index, &depth)| {
            while chain.last().is_some_and(|&last| depths[last] >= depth) {
                chain.pop();
            }
            let parent = chain.last().copied();
            chain.push(index);
            parent
        })
        .collect()
}

/// Whether a row has rows beneath it.
pub fn has_children(parents: &[Option<usize>]) -> Vec<bool> {
    let mut children = vec![false; parents.len()];
    for parent in parents.iter().flatten() {
        children[*parent] = true;
    }
    children
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_row_goes_under_the_nearest_shallower_row_above_it() {
        assert_eq!(
            parents(&[0, 1, 2, 1, 0, 1]),
            vec![None, Some(0), Some(1), Some(0), None, Some(4)]
        );
    }

    #[test]
    fn a_row_whose_parent_was_filtered_out_sits_at_the_top() {
        // A level-two row straight after a top-level one goes under it; one with nothing
        // shallower above it in its branch has no parent at all.
        assert_eq!(parents(&[1, 2, 0, 2]), vec![None, Some(0), None, Some(2)]);
    }

    #[test]
    fn rows_know_whether_anything_is_beneath_them() {
        let parents = parents(&[0, 1, 1, 0]);
        assert_eq!(has_children(&parents), vec![true, false, false, false]);
    }
}
