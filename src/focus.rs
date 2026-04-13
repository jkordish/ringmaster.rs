use crate::navigation::NavMove;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HelpOverlayAnchor {
    BindingList,
}

impl HelpOverlayAnchor {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::BindingList => "bindings",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrendsMatrixSubfocus {
    SortTabs,
    Rows,
}

impl TrendsMatrixSubfocus {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::SortTabs => "trend sort",
            Self::Rows => "trend rows",
        }
    }

    #[must_use]
    pub const fn is_sort_tabs(self) -> bool {
        matches!(self, Self::SortTabs)
    }

    #[must_use]
    pub const fn is_rows(self) -> bool {
        matches!(self, Self::Rows)
    }
}

#[must_use]
pub const fn move_roving_index(current: usize, len: usize, movement: NavMove) -> usize {
    if len == 0 {
        return 0;
    }

    let last = len - 1;
    match movement {
        NavMove::Previous => current.saturating_sub(1),
        NavMove::Next => {
            if current < last {
                current + 1
            } else {
                last
            }
        }
        NavMove::First | NavMove::PageBackward => 0,
        NavMove::Last | NavMove::PageForward => last,
    }
}

#[must_use]
pub const fn clamp_roving_index(index: usize, len: usize) -> usize {
    if len == 0 {
        0
    } else if index >= len {
        len - 1
    } else {
        index
    }
}

#[cfg(test)]
mod tests {
    use super::{HelpOverlayAnchor, TrendsMatrixSubfocus, clamp_roving_index, move_roving_index};
    use crate::navigation::NavMove;

    #[test]
    fn roving_index_moves_to_expected_edges() {
        assert_eq!(move_roving_index(1, 4, NavMove::Previous), 0);
        assert_eq!(move_roving_index(1, 4, NavMove::Next), 2);
        assert_eq!(move_roving_index(1, 4, NavMove::First), 0);
        assert_eq!(move_roving_index(1, 4, NavMove::Last), 3);
    }

    #[test]
    fn clamped_roving_index_never_exceeds_bounds() {
        assert_eq!(clamp_roving_index(5, 0), 0);
        assert_eq!(clamp_roving_index(5, 3), 2);
    }

    #[test]
    fn focus_labels_are_stable() {
        assert_eq!(HelpOverlayAnchor::BindingList.label(), "bindings");
        assert_eq!(TrendsMatrixSubfocus::SortTabs.label(), "trend sort");
        assert_eq!(TrendsMatrixSubfocus::Rows.label(), "trend rows");
    }
}
