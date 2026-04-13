use ratatui::layout::Rect;

pub const SEARCH_MODAL_MAX_WIDTH: u16 = 64;
pub const SEARCH_MODAL_MAX_HEIGHT: u16 = 9;
pub const HELP_MODAL_MAX_WIDTH: u16 = 78;
pub const HELP_MODAL_MAX_HEIGHT: u16 = 20;
pub const HELP_MODAL_VISIBLE_BODY_ROWS: u16 = 14;

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

    #[must_use]
    pub const fn named_dimensions(self) -> (u16, u16) {
        match self {
            Self::Compact => (90, 28),
            Self::Medium => (120, 36),
            Self::Wide => (160, 44),
        }
    }
}

pub const DEFAULT_NON_INTERACTIVE_VIEWPORT: ViewportClass = ViewportClass::Medium;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UiContext {
    pub area: Rect,
    pub viewport: ViewportClass,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModalLayout {
    pub bounds: Rect,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModalLayoutSpec {
    pub max_width: u16,
    pub max_height: u16,
    pub inset_x: u16,
    pub inset_y: u16,
}

impl ModalLayoutSpec {
    #[must_use]
    pub const fn new(max_width: u16, max_height: u16) -> Self {
        Self {
            max_width,
            max_height,
            inset_x: 2,
            inset_y: 1,
        }
    }

    #[must_use]
    #[cfg_attr(not(test), allow(dead_code))]
    pub const fn with_insets(mut self, inset_x: u16, inset_y: u16) -> Self {
        self.inset_x = inset_x;
        self.inset_y = inset_y;
        self
    }

    #[must_use]
    pub fn from_percent(area: Rect, width_pct: u16, height_pct: u16) -> Self {
        let width = area.width.saturating_mul(width_pct).saturating_div(100);
        let height = area.height.saturating_mul(height_pct).saturating_div(100);
        Self::new(width.max(1), height.max(1))
    }
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
                badge_width: 7,
                focus_gutter_width: 2,
                footer_hint_limit: 1,
            },
            ViewportClass::Medium => Self {
                outer_margin: 0,
                panel_gap_x: 1,
                panel_gap_y: 1,
                panel_pad_x: 1,
                panel_pad_y: 0,
                title_pad_x: 1,
                title_row_height: 1,
                title_separator_gap: 1,
                content_top_inset: 2,
                major_inset_x: 3,
                badge_width: 7,
                focus_gutter_width: 2,
                footer_hint_limit: 2,
            },
            ViewportClass::Wide => Self {
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
                badge_width: 7,
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

#[must_use]
pub fn centered_modal_layout(area: Rect, spec: ModalLayoutSpec) -> ModalLayout {
    let available_width = area
        .width
        .saturating_sub(spec.inset_x.saturating_mul(2))
        .max(1);
    let available_height = area
        .height
        .saturating_sub(spec.inset_y.saturating_mul(2))
        .max(1);
    let popup_width = spec.max_width.min(available_width).max(1);
    let popup_height = spec.max_height.min(available_height).max(1);
    let x = area.x + area.width.saturating_sub(popup_width) / 2;
    let y = area.y + area.height.saturating_sub(popup_height) / 2;
    ModalLayout {
        bounds: Rect::new(x, y, popup_width, popup_height),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        DEFAULT_NON_INTERACTIVE_VIEWPORT, DashboardMetrics, ModalLayoutSpec, ViewportClass,
        centered_modal_layout,
    };
    use ratatui::layout::Rect;

    #[test]
    fn viewport_breakpoints_are_stable() {
        assert_eq!(ViewportClass::from_width(90), ViewportClass::Compact);
        assert_eq!(ViewportClass::from_width(120), ViewportClass::Medium);
        assert_eq!(ViewportClass::from_width(160), ViewportClass::Wide);
        assert_eq!(ViewportClass::Compact.named_dimensions(), (90, 28));
        assert_eq!(ViewportClass::Medium.named_dimensions(), (120, 36));
        assert_eq!(ViewportClass::Wide.named_dimensions(), (160, 44));
        assert_eq!(DEFAULT_NON_INTERACTIVE_VIEWPORT, ViewportClass::Medium);
    }

    #[test]
    fn dashboard_metrics_keep_a_single_spacing_vocabulary() {
        let compact = DashboardMetrics::for_viewport(ViewportClass::Compact);
        let medium = DashboardMetrics::for_viewport(ViewportClass::Medium);
        let wide = DashboardMetrics::for_viewport(ViewportClass::Wide);

        assert_eq!(compact.panel_gap_x, 1);
        assert_eq!(wide.panel_gap_x, 1);
        assert_eq!(compact.title_pad_x, 1);
        assert_eq!(medium.panel_pad_x, 1);
        assert_eq!(medium.major_inset_x, 3);
        assert_eq!(wide.panel_pad_x, 2);
        assert_eq!(wide.major_inset_x, 4);
        assert_eq!(wide.badge_width, 7);
        assert_eq!(wide.focus_gutter_width, 2);
        assert_eq!(wide.footer_hint_limit, 2);
    }

    #[test]
    fn modal_layout_stays_centered_inside_requested_insets() {
        let layout = centered_modal_layout(
            Rect::new(0, 0, 120, 40),
            ModalLayoutSpec::new(72, 18).with_insets(4, 2),
        );

        assert_eq!(layout.bounds.width, 72);
        assert_eq!(layout.bounds.height, 18);
        assert_eq!(layout.bounds.x, 24);
        assert_eq!(layout.bounds.y, 11);
    }

    #[test]
    fn modal_layout_caps_to_the_available_viewport() {
        let layout = centered_modal_layout(
            Rect::new(0, 0, 20, 8),
            ModalLayoutSpec::new(72, 18).with_insets(3, 2),
        );

        assert_eq!(layout.bounds.width, 14);
        assert_eq!(layout.bounds.height, 4);
        assert_eq!(layout.bounds.x, 3);
        assert_eq!(layout.bounds.y, 2);
    }
}
