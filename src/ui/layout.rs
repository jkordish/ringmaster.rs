use ratatui::layout::Rect;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewportClass {
    Compact,
    Medium,
    Wide,
}

impl ViewportClass {
    #[must_use]
    pub const fn from_width(width: u16) -> Self {
        if width < 100 {
            Self::Compact
        } else if width < 140 {
            Self::Medium
        } else {
            Self::Wide
        }
    }

    #[must_use]
    pub const fn is_compact(self) -> bool {
        matches!(self, Self::Compact)
    }

    #[must_use]
    pub const fn is_wide(self) -> bool {
        matches!(self, Self::Wide)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UiContext {
    pub area: Rect,
    pub viewport: ViewportClass,
}

impl UiContext {
    #[must_use]
    pub const fn new(area: Rect) -> Self {
        Self {
            area,
            viewport: ViewportClass::from_width(area.width),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DashboardMetrics {
    pub outer_margin: u16,
    pub panel_gap_x: u16,
    pub panel_gap_y: u16,
    pub panel_pad_x: u16,
    pub panel_pad_y: u16,
    pub title_pad_x: u16,
    pub title_row_height: u16,
    pub title_separator_gap: u16,
    pub content_top_inset: u16,
    pub major_inset_x: u16,
    pub badge_width: usize,
    pub focus_gutter_width: usize,
    pub footer_hint_limit: usize,
}

impl DashboardMetrics {
    #[must_use]
    pub const fn for_viewport(viewport: ViewportClass) -> Self {
        match viewport {
            ViewportClass::Compact => Self {
                outer_margin: 0,
                panel_gap_x: 1,
                panel_gap_y: 1,
                panel_pad_x: 1,
                panel_pad_y: 0,
                title_pad_x: 1,
                title_row_height: 1,
                title_separator_gap: 0,
                content_top_inset: 1,
                major_inset_x: 2,
                badge_width: 8,
                focus_gutter_width: 2,
                footer_hint_limit: 1,
            },
            ViewportClass::Medium | ViewportClass::Wide => Self {
                outer_margin: 0,
                panel_gap_x: 1,
                panel_gap_y: 1,
                panel_pad_x: 2,
                panel_pad_y: 0,
                title_pad_x: 2,
                title_row_height: 1,
                title_separator_gap: 1,
                content_top_inset: 2,
                major_inset_x: 4,
                badge_width: 8,
                focus_gutter_width: 2,
                footer_hint_limit: 2,
            },
        }
    }
}

#[must_use]
pub const fn inset(area: Rect, horizontal: u16, vertical: u16) -> Rect {
    let width = area.width.saturating_sub(horizontal.saturating_mul(2));
    let height = area.height.saturating_sub(vertical.saturating_mul(2));
    Rect::new(
        area.x.saturating_add(horizontal),
        area.y.saturating_add(vertical),
        width,
        height,
    )
}

#[cfg(test)]
mod tests {
    use super::{DashboardMetrics, ViewportClass};

    #[test]
    fn viewport_breakpoints_are_stable() {
        assert_eq!(ViewportClass::from_width(90), ViewportClass::Compact);
        assert_eq!(ViewportClass::from_width(120), ViewportClass::Medium);
        assert_eq!(ViewportClass::from_width(160), ViewportClass::Wide);
    }

    #[test]
    fn dashboard_metrics_keep_a_single_spacing_vocabulary() {
        let compact = DashboardMetrics::for_viewport(ViewportClass::Compact);
        let wide = DashboardMetrics::for_viewport(ViewportClass::Wide);

        assert_eq!(compact.panel_gap_x, 1);
        assert_eq!(wide.panel_gap_x, 1);
        assert_eq!(compact.title_pad_x, 1);
        assert_eq!(wide.panel_pad_x, 2);
        assert_eq!(wide.major_inset_x, 4);
        assert_eq!(wide.badge_width, 8);
        assert_eq!(wide.focus_gutter_width, 2);
        assert_eq!(wide.footer_hint_limit, 2);
    }
}
