use crate::navigation::NavMove;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusInteraction {
    Navigate(&'static str),
    Expand(&'static str),
    Toggle(&'static str),
    OpenOverlay(&'static str),
    Activate(&'static str),
    None,
}

impl FocusInteraction {
    #[must_use]
    pub const fn is_actionable(self) -> bool {
        !matches!(self, Self::None)
    }

    #[must_use]
    pub const fn kind_label(self) -> &'static str {
        match self {
            Self::Navigate(_) => "navigate",
            Self::Expand(_) => "expand",
            Self::Toggle(_) => "toggle",
            Self::OpenOverlay(_) => "open overlay",
            Self::Activate(_) => "activate",
            Self::None => "inspect only",
        }
    }

    #[must_use]
    pub const fn target_label(self) -> Option<&'static str> {
        match self {
            Self::Navigate(target)
            | Self::Expand(target)
            | Self::Toggle(target)
            | Self::OpenOverlay(target)
            | Self::Activate(target) => Some(target),
            Self::None => None,
        }
    }
}

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
pub enum SearchOverlayAnchor {
    QueryField,
}

impl SearchOverlayAnchor {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::QueryField => "query",
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
    use super::{
        FocusInteraction, HelpOverlayAnchor, SearchOverlayAnchor, TrendsMatrixSubfocus,
        clamp_roving_index, move_roving_index,
    };
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
        assert_eq!(SearchOverlayAnchor::QueryField.label(), "query");
        assert_eq!(TrendsMatrixSubfocus::SortTabs.label(), "trend sort");
        assert_eq!(TrendsMatrixSubfocus::Rows.label(), "trend rows");
    }

    #[test]
    fn focus_interaction_reports_actionability_truthfully() {
        let navigate = FocusInteraction::Navigate("timeline detail");
        let inspect_only = FocusInteraction::None;

        assert!(navigate.is_actionable());
        assert_eq!(navigate.kind_label(), "navigate");
        assert_eq!(navigate.target_label(), Some("timeline detail"));

        assert!(!inspect_only.is_actionable());
        assert_eq!(inspect_only.kind_label(), "inspect only");
        assert_eq!(inspect_only.target_label(), None);
    }
}
