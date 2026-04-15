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
pub struct ChartViewport {
    pub area: Rect,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SupportLane {
    pub area: Rect,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PanelContentMetrics {
    pub chart: ChartViewport,
    pub support: SupportLane,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChartClusterPacking {
    pub cluster_area: Rect,
    pub support_area: Rect,
    pub top_padding: u16,
    pub bottom_padding: u16,
    pub gap_before_support: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OverlayLayout {
    pub bounds: Rect,
    pub inner: Rect,
    pub title_area: Rect,
    pub content_area: Rect,
    pub visible_body_rows: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OverlayLayoutSpec {
    pub min_width: u16,
    pub max_width: u16,
    pub min_height: u16,
    pub max_height: u16,
    pub inset_x: u16,
    pub inset_y: u16,
    pub content_width_hint: u16,
    pub content_height_hint: u16,
}

impl OverlayLayoutSpec {
    #[must_use]
    pub const fn new(max_width: u16, max_height: u16) -> Self {
        Self {
            min_width: 40,
            max_width,
            min_height: 8,
            max_height,
            inset_x: 2,
            inset_y: 1,
            content_width_hint: max_width.saturating_sub(6),
            content_height_hint: max_height.saturating_sub(4),
        }
    }

    #[must_use]
    pub const fn with_min_size(mut self, min_width: u16, min_height: u16) -> Self {
        self.min_width = min_width;
        self.min_height = min_height;
        self
    }

    #[must_use]
    #[allow(dead_code)]
    pub const fn with_insets(mut self, inset_x: u16, inset_y: u16) -> Self {
        self.inset_x = inset_x;
        self.inset_y = inset_y;
        self
    }

    #[must_use]
    pub const fn with_content_hints(
        mut self,
        content_width_hint: u16,
        content_height_hint: u16,
    ) -> Self {
        self.content_width_hint = content_width_hint;
        self.content_height_hint = content_height_hint;
        self
    }
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
                badge_width: 6,
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
                badge_width: 8,
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
                badge_width: 8,
                focus_gutter_width: 2,
                footer_hint_limit: 2,
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DashboardChartMetrics {
    pub support_lane_height: u16,
    pub breakdown_header_height: u16,
    pub breakdown_band_height: u16,
    pub breakdown_label_min_width: u16,
    pub breakdown_label_max_width: u16,
    pub breakdown_delta_min_width: u16,
    pub breakdown_delta_max_width: u16,
    pub breakdown_signal_badge_width: u16,
    pub breakdown_bar_min_width: u16,
    pub weekly_header_height: u16,
    pub weekly_legend_height: u16,
    pub weekly_summary_height: u16,
    pub weekly_label_min_width: u16,
    pub weekly_label_max_width: u16,
    pub weekly_slot_min_width: u16,
    pub weekly_row_gap: u16,
}

impl DashboardChartMetrics {
    #[must_use]
    pub const fn for_viewport(viewport: ViewportClass) -> Self {
        match viewport {
            ViewportClass::Compact => Self {
                support_lane_height: 1,
                breakdown_header_height: 1,
                breakdown_band_height: 0,
                breakdown_label_min_width: 8,
                breakdown_label_max_width: 12,
                breakdown_delta_min_width: 7,
                breakdown_delta_max_width: 10,
                breakdown_signal_badge_width: 7,
                breakdown_bar_min_width: 8,
                weekly_header_height: 1,
                weekly_legend_height: 1,
                weekly_summary_height: 1,
                weekly_label_min_width: 4,
                weekly_label_max_width: 5,
                weekly_slot_min_width: 3,
                weekly_row_gap: 0,
            },
            ViewportClass::Medium => Self {
                support_lane_height: 1,
                breakdown_header_height: 1,
                breakdown_band_height: 0,
                breakdown_label_min_width: 9,
                breakdown_label_max_width: 13,
                breakdown_delta_min_width: 8,
                breakdown_delta_max_width: 10,
                breakdown_signal_badge_width: 7,
                breakdown_bar_min_width: 10,
                weekly_header_height: 1,
                weekly_legend_height: 1,
                weekly_summary_height: 1,
                weekly_label_min_width: 5,
                weekly_label_max_width: 6,
                weekly_slot_min_width: 3,
                weekly_row_gap: 0,
            },
            ViewportClass::Wide => Self {
                support_lane_height: 1,
                breakdown_header_height: 1,
                breakdown_band_height: 0,
                breakdown_label_min_width: 10,
                breakdown_label_max_width: 14,
                breakdown_delta_min_width: 8,
                breakdown_delta_max_width: 11,
                breakdown_signal_badge_width: 7,
                breakdown_bar_min_width: 12,
                weekly_header_height: 1,
                weekly_legend_height: 1,
                weekly_summary_height: 1,
                weekly_label_min_width: 5,
                weekly_label_max_width: 6,
                weekly_slot_min_width: 3,
                weekly_row_gap: 0,
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WeeklyHeatmapMode {
    Standard,
    #[cfg_attr(not(test), allow(dead_code))]
    DenseHistory,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BreakdownLayout {
    pub chart_body: Rect,
    pub support_lane: Rect,
    pub header_area: Rect,
    pub band_area: Rect,
    pub top_padding: u16,
    pub bottom_padding: u16,
    pub gap_before_support: u16,
    pub row_gap_count: u16,
    pub band_gap: u16,
    pub label_column_x: u16,
    pub label_column_width: u16,
    pub signal_column_x: u16,
    pub signal_column_width: u16,
    pub signal_badge_width: u16,
    pub bar_viewport_x: u16,
    pub bar_viewport_width: u16,
    pub delta_column_x: u16,
    pub delta_column_width: u16,
    pub row_height: u16,
    row_count: u16,
}

impl BreakdownLayout {
    #[must_use]
    pub fn for_panel(
        area: Rect,
        metrics: DashboardChartMetrics,
        row_count: usize,
        preferred_label_width: u16,
        preferred_delta_width: u16,
    ) -> Self {
        let row_count = u16::try_from(row_count).unwrap_or(u16::MAX).max(1);
        let minimum_cluster_height = metrics
            .breakdown_header_height
            .saturating_add(row_count)
            .saturating_add(metrics.breakdown_band_height);
        let packing = pack_chart_cluster_vertically(
            area,
            minimum_cluster_height.max(1),
            metrics.support_lane_height,
        );
        let mut cluster_slack = packing.top_padding.saturating_add(packing.bottom_padding);
        let mut row_height = 1;
        if cluster_slack >= row_count {
            row_height = 2;
            cluster_slack = cluster_slack.saturating_sub(row_count);
        }
        let band_gap = cluster_slack.min(1);
        cluster_slack = cluster_slack.saturating_sub(band_gap);
        let row_gap_count = cluster_slack.min(row_count.saturating_sub(1));
        cluster_slack = cluster_slack.saturating_sub(row_gap_count);
        let top_padding = cluster_slack / 2;
        let bottom_padding = cluster_slack.saturating_sub(top_padding);
        let cluster_top = packing.cluster_area.y.saturating_add(top_padding);
        let header_area = Rect::new(
            packing.cluster_area.x,
            cluster_top,
            packing.cluster_area.width,
            metrics
                .breakdown_header_height
                .min(packing.cluster_area.height.saturating_sub(top_padding)),
        );

        let available_width = packing.cluster_area.width;
        let reserved_signal = metrics
            .breakdown_signal_badge_width
            .saturating_add(metrics.breakdown_bar_min_width)
            .saturating_add(2);
        let label_column_width = label_column_width(
            preferred_label_width,
            metrics.breakdown_label_min_width,
            metrics.breakdown_label_max_width,
            available_width,
            reserved_signal.saturating_add(metrics.breakdown_delta_min_width),
        );
        let remaining_after_label = available_width.saturating_sub(label_column_width);
        let delta_column_width = preferred_delta_width
            .clamp(
                metrics.breakdown_delta_min_width,
                metrics.breakdown_delta_max_width,
            )
            .min(
                remaining_after_label
                    .saturating_sub(reserved_signal)
                    .max(metrics.breakdown_delta_min_width),
            );
        let signal_column_width = available_width
            .saturating_sub(label_column_width)
            .saturating_sub(delta_column_width)
            .saturating_sub(2)
            .max(
                metrics
                    .breakdown_signal_badge_width
                    .saturating_add(metrics.breakdown_bar_min_width),
            );
        let label_column_x = packing.cluster_area.x;
        let signal_column_x = label_column_x
            .saturating_add(label_column_width)
            .saturating_add(1);
        let delta_column_x = signal_column_x
            .saturating_add(signal_column_width)
            .saturating_add(1);
        let signal_badge_width = metrics
            .breakdown_signal_badge_width
            .min(signal_column_width);
        let bar_viewport_x = signal_column_x
            .saturating_add(signal_badge_width)
            .saturating_add(1);
        let bar_viewport_width = signal_column_width
            .saturating_sub(signal_badge_width)
            .saturating_sub(1);
        let band_y = header_area
            .y
            .saturating_add(metrics.breakdown_header_height)
            .saturating_add(row_count.saturating_mul(row_height))
            .saturating_add(row_gap_count)
            .saturating_add(band_gap);
        let band_area = Rect::new(
            packing.cluster_area.x,
            band_y.min(
                packing
                    .cluster_area
                    .y
                    .saturating_add(packing.cluster_area.height.saturating_sub(1)),
            ),
            packing.cluster_area.width,
            metrics.breakdown_band_height.min(
                packing.cluster_area.height.saturating_sub(
                    header_area
                        .y
                        .saturating_sub(packing.cluster_area.y)
                        .saturating_add(metrics.breakdown_header_height)
                        .saturating_add(row_count)
                        .saturating_add(row_gap_count)
                        .saturating_add(band_gap),
                ),
            ),
        );

        Self {
            chart_body: packing.cluster_area,
            support_lane: packing.support_area,
            header_area,
            band_area,
            top_padding,
            bottom_padding,
            gap_before_support: packing.gap_before_support,
            row_gap_count,
            band_gap,
            label_column_x,
            label_column_width,
            signal_column_x,
            signal_column_width,
            signal_badge_width,
            bar_viewport_x,
            bar_viewport_width,
            delta_column_x,
            delta_column_width,
            row_height,
            row_count,
        }
    }

    #[must_use]
    pub fn row_area(self, index: usize) -> Rect {
        let clamped_index = index.min(usize::from(self.row_count.saturating_sub(1)));
        let index = u16::try_from(clamped_index).unwrap_or(u16::MAX);
        let gap_before = index.min(self.row_gap_count);
        Rect::new(
            self.chart_body.x,
            self.header_area
                .y
                .saturating_add(self.header_area.height)
                .saturating_add(index.saturating_mul(self.row_height))
                .saturating_add(gap_before),
            self.chart_body.width,
            self.row_height,
        )
    }

    #[must_use]
    pub fn row_line_area(self, index: usize) -> Rect {
        let row = self.row_area(index);
        Rect::new(row.x, row.y.saturating_add(row.height / 2), row.width, 1)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WeeklyTrendsLayout {
    pub chart_body: Rect,
    pub support_lane: Rect,
    pub header_area: Rect,
    pub header_grid_area: Rect,
    pub grid_viewport: Rect,
    pub legend_area: Rect,
    pub summary_area: Rect,
    pub label_column_width: u16,
    pub slot_width: u16,
    pub cell_width: u16,
    pub cell_gap: u16,
    pub group_gap_count: u16,
    pub subrow_gap: u16,
    pub top_padding: u16,
    pub bottom_padding: u16,
    pub gap_before_support: u16,
    group_count: u16,
}

impl WeeklyTrendsLayout {
    const SUBROWS_PER_GROUP: u16 = 2;

    #[must_use]
    pub fn for_panel(
        area: Rect,
        metrics: DashboardChartMetrics,
        mode: WeeklyHeatmapMode,
        day_count: usize,
        row_count: usize,
    ) -> Self {
        let column_count = u16::try_from(day_count).unwrap_or(u16::MAX).max(1);
        let group_count = u16::try_from(row_count).unwrap_or(u16::MAX).max(1);
        let min_grid_height = group_count.saturating_mul(Self::SUBROWS_PER_GROUP);
        let minimum_cluster_height = metrics
            .weekly_header_height
            .saturating_add(min_grid_height)
            .saturating_add(metrics.weekly_legend_height);
        let packing = pack_chart_cluster_vertically(
            area,
            minimum_cluster_height.max(1),
            metrics
                .weekly_summary_height
                .min(metrics.support_lane_height.max(1)),
        );
        let mut cluster_slack = packing.top_padding.saturating_add(packing.bottom_padding);
        let group_gap_count = cluster_slack.min(group_count.saturating_sub(1));
        cluster_slack = cluster_slack.saturating_sub(group_gap_count);
        let subrow_gap = u16::from(cluster_slack >= group_count);
        if subrow_gap > 0 {
            cluster_slack = cluster_slack.saturating_sub(group_count);
        }
        let top_padding = cluster_slack / 2;
        let bottom_padding = cluster_slack.saturating_sub(top_padding);

        let cluster_width = packing.cluster_area.width;
        let label_column_width = match mode {
            WeeklyHeatmapMode::DenseHistory => metrics.weekly_label_min_width,
            WeeklyHeatmapMode::Standard => metrics.weekly_label_max_width,
        }
        .min(
            cluster_width
                .saturating_sub(column_count.saturating_mul(metrics.weekly_slot_min_width)),
        )
        .max(metrics.weekly_label_min_width);
        let usable_width = cluster_width
            .saturating_sub(label_column_width)
            .max(column_count);
        let slot_width = usable_width
            .checked_div(column_count)
            .unwrap_or(metrics.weekly_slot_min_width)
            .max(metrics.weekly_slot_min_width);
        let cell_gap: u16 = 1;
        let cell_width = slot_width.saturating_sub(cell_gap.saturating_mul(2)).max(1);
        let cluster_top = packing.cluster_area.y.saturating_add(top_padding);

        let header_area = Rect::new(
            packing.cluster_area.x,
            cluster_top,
            cluster_width,
            metrics.weekly_header_height.min(
                packing
                    .cluster_area
                    .height
                    .saturating_sub(top_padding)
                    .max(1),
            ),
        );
        let grid_height = min_grid_height
            .saturating_add(group_gap_count)
            .saturating_add(group_count.saturating_mul(subrow_gap));
        let grid_viewport = Rect::new(
            packing.cluster_area.x.saturating_add(label_column_width),
            header_area.y.saturating_add(header_area.height),
            slot_width.saturating_mul(column_count),
            grid_height,
        );
        let legend_area = Rect::new(
            grid_viewport.x,
            grid_viewport.y.saturating_add(grid_viewport.height),
            grid_viewport.width,
            metrics.weekly_legend_height.min(
                packing.cluster_area.height.saturating_sub(
                    grid_viewport
                        .y
                        .saturating_sub(packing.cluster_area.y)
                        .saturating_add(grid_viewport.height),
                ),
            ),
        );

        Self {
            chart_body: packing.cluster_area,
            support_lane: packing.support_area,
            header_area,
            header_grid_area: Rect::new(
                grid_viewport.x,
                header_area.y,
                grid_viewport.width,
                header_area.height,
            ),
            grid_viewport,
            legend_area,
            summary_area: Rect::new(
                grid_viewport.x,
                packing.support_area.y,
                packing
                    .support_area
                    .width
                    .saturating_sub(label_column_width),
                packing.support_area.height,
            ),
            label_column_width,
            slot_width,
            cell_width,
            cell_gap,
            group_gap_count,
            subrow_gap,
            top_padding,
            bottom_padding,
            gap_before_support: packing.gap_before_support,
            group_count,
        }
    }

    #[must_use]
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn column_origin(self, column_index: usize) -> u16 {
        let clamped_index = column_index.min(usize::from(u16::MAX));
        let column_index = u16::try_from(clamped_index).unwrap_or(u16::MAX);
        self.grid_viewport
            .x
            .saturating_add(self.slot_width.saturating_mul(column_index))
    }

    #[must_use]
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn selected_bracket_origin(self, column_index: usize) -> u16 {
        self.column_origin(column_index)
    }

    #[must_use]
    pub fn group_area(self, group_index: usize) -> Rect {
        let clamped_index = group_index.min(usize::from(self.group_count.saturating_sub(1)));
        let group_index = u16::try_from(clamped_index).unwrap_or(u16::MAX);
        let group_height = Self::SUBROWS_PER_GROUP.saturating_add(self.subrow_gap);
        Rect::new(
            self.chart_body.x,
            self.grid_viewport
                .y
                .saturating_add(group_index.saturating_mul(group_height))
                .saturating_add(group_index.min(self.group_gap_count)),
            self.chart_body.width,
            group_height,
        )
    }

    #[must_use]
    pub fn group_label_area(self, group_index: usize) -> Rect {
        let group = self.group_area(group_index);
        Rect::new(
            self.chart_body.x,
            group.y,
            self.label_column_width,
            group.height,
        )
    }

    #[must_use]
    pub fn group_label_line_area(self, group_index: usize) -> Rect {
        let group = self.group_label_area(group_index);
        Rect::new(
            group.x,
            group.y.saturating_add(group.height / 2),
            group.width,
            1,
        )
    }

    #[must_use]
    pub fn subrow_area(self, group_index: usize, subrow_index: usize) -> Rect {
        let group = self.group_area(group_index);
        let subrow_index = u16::try_from(subrow_index)
            .unwrap_or(u16::MAX)
            .min(Self::SUBROWS_PER_GROUP.saturating_sub(1));
        Rect::new(
            self.grid_viewport.x,
            group
                .y
                .saturating_add(subrow_index)
                .saturating_add(subrow_index.min(1).saturating_mul(self.subrow_gap)),
            self.grid_viewport.width,
            1,
        )
    }
}

#[must_use]
pub const fn pack_chart_cluster_vertically(
    area: Rect,
    cluster_min_height: u16,
    support_lane_height: u16,
) -> ChartClusterPacking {
    if area.width == 0 || area.height == 0 {
        return ChartClusterPacking {
            cluster_area: area,
            support_area: Rect::new(area.x, area.y, 0, 0),
            top_padding: 0,
            bottom_padding: 0,
            gap_before_support: 0,
        };
    }

    let support_height = if support_lane_height > 0 && area.height > cluster_min_height {
        let support_budget = area.height.saturating_sub(cluster_min_height);
        if support_lane_height > support_budget {
            support_budget
        } else {
            support_lane_height
        }
    } else {
        0
    };
    let remaining_height = area
        .height
        .saturating_sub(cluster_min_height)
        .saturating_sub(support_height);
    let gap_before_support = (support_height > 0 && remaining_height > 0) as u16;
    let cluster_height = area
        .height
        .saturating_sub(support_height)
        .saturating_sub(gap_before_support);
    let cluster_slack = cluster_height.saturating_sub(cluster_min_height);
    let top_padding = cluster_slack / 2;
    let bottom_padding = cluster_slack.saturating_sub(top_padding);

    ChartClusterPacking {
        cluster_area: Rect::new(area.x, area.y, area.width, cluster_height),
        support_area: Rect::new(
            area.x,
            area.y
                .saturating_add(cluster_height)
                .saturating_add(gap_before_support),
            area.width,
            support_height,
        ),
        top_padding,
        bottom_padding,
        gap_before_support,
    }
}

#[must_use]
pub const fn panel_content_metrics(area: Rect, support_lane_height: u16) -> PanelContentMetrics {
    if support_lane_height == 0 || area.height <= support_lane_height {
        return PanelContentMetrics {
            chart: ChartViewport { area },
            support: SupportLane {
                area: Rect::new(area.x, area.y.saturating_add(area.height), area.width, 0),
            },
        };
    }

    let chart_body_height = area.height.saturating_sub(support_lane_height);
    PanelContentMetrics {
        chart: ChartViewport {
            area: Rect::new(area.x, area.y, area.width, chart_body_height),
        },
        support: SupportLane {
            area: Rect::new(
                area.x,
                area.y.saturating_add(chart_body_height),
                area.width,
                support_lane_height,
            ),
        },
    }
}

#[must_use]
pub fn label_column_width(
    preferred: u16,
    minimum: u16,
    maximum: u16,
    available_width: u16,
    reserved_after: u16,
) -> u16 {
    preferred
        .clamp(minimum, maximum)
        .min(available_width.saturating_sub(reserved_after).max(minimum))
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

impl BreakdownLayout {
    #[must_use]
    pub const fn label_cell(self, row: Rect) -> Rect {
        Rect::new(
            self.label_column_x,
            row.y,
            self.label_column_width,
            row.height,
        )
    }

    #[must_use]
    pub const fn signal_cell(self, row: Rect) -> Rect {
        Rect::new(
            self.signal_column_x,
            row.y,
            self.signal_column_width,
            row.height,
        )
    }

    #[must_use]
    pub const fn signal_badge_cell(self, row: Rect) -> Rect {
        Rect::new(
            self.signal_column_x,
            row.y,
            self.signal_badge_width,
            row.height,
        )
    }

    #[must_use]
    pub const fn signal_track_cell(self, row: Rect) -> Rect {
        Rect::new(
            self.bar_viewport_x,
            row.y,
            self.bar_viewport_width,
            row.height,
        )
    }

    #[must_use]
    pub const fn delta_cell(self, row: Rect) -> Rect {
        Rect::new(
            self.delta_column_x,
            row.y,
            self.delta_column_width,
            row.height,
        )
    }
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

#[must_use]
pub fn content_fit_overlay_layout(
    area: Rect,
    metrics: DashboardMetrics,
    spec: OverlayLayoutSpec,
) -> OverlayLayout {
    let available_width = area
        .width
        .saturating_sub(spec.inset_x.saturating_mul(2))
        .max(1);
    let available_height = area
        .height
        .saturating_sub(spec.inset_y.saturating_mul(2))
        .max(1);
    let horizontal_shell = 2u16.saturating_add(metrics.panel_pad_x.saturating_mul(2));
    let vertical_shell = 2u16.saturating_add(metrics.content_top_inset);

    let preferred_width = spec.content_width_hint.saturating_add(horizontal_shell);
    let preferred_height = spec.content_height_hint.saturating_add(vertical_shell);
    let popup_width = preferred_width
        .clamp(spec.min_width, spec.max_width)
        .min(available_width)
        .max(1);
    let popup_height = preferred_height
        .clamp(spec.min_height, spec.max_height)
        .min(available_height)
        .max(1);
    let bounds = Rect::new(
        area.x + area.width.saturating_sub(popup_width) / 2,
        area.y + area.height.saturating_sub(popup_height) / 2,
        popup_width,
        popup_height,
    );
    let inner = inset(bounds, 1, 1);
    let title_height = metrics.title_row_height.min(inner.height);
    let title_area = Rect::new(inner.x, inner.y, inner.width, title_height);
    let body_top = if inner.height > metrics.content_top_inset {
        inner.y.saturating_add(metrics.content_top_inset)
    } else {
        inner.y.saturating_add(title_height)
    };
    let body_height = inner
        .height
        .saturating_sub(body_top.saturating_sub(inner.y))
        .max(1);
    let content_area = inset(
        Rect::new(inner.x, body_top, inner.width, body_height),
        metrics.panel_pad_x,
        metrics.panel_pad_y,
    );

    OverlayLayout {
        bounds,
        inner,
        title_area,
        content_area,
        visible_body_rows: content_area.height,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        BreakdownLayout, DEFAULT_NON_INTERACTIVE_VIEWPORT, DashboardChartMetrics, DashboardMetrics,
        ModalLayoutSpec, OverlayLayoutSpec, ViewportClass, WeeklyHeatmapMode, WeeklyTrendsLayout,
        centered_modal_layout, content_fit_overlay_layout, pack_chart_cluster_vertically,
        panel_content_metrics,
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
        assert_eq!(compact.badge_width, 6);
        assert_eq!(medium.badge_width, 8);
        assert_eq!(wide.badge_width, 8);
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

    #[test]
    fn overlay_layout_centers_and_preserves_visible_body_rows() {
        let layout = content_fit_overlay_layout(
            Rect::new(0, 0, 120, 36),
            DashboardMetrics::for_viewport(ViewportClass::Medium),
            OverlayLayoutSpec::new(78, 20)
                .with_min_size(56, 12)
                .with_content_hints(68, 14),
        );

        assert_eq!(layout.bounds.width, 72);
        assert_eq!(layout.bounds.height, 18);
        assert_eq!(layout.bounds.x, 24);
        assert_eq!(layout.bounds.y, 9);
        assert_eq!(layout.content_area.height, 14);
        assert_eq!(layout.visible_body_rows, 14);
    }

    #[test]
    fn breakdown_layout_keeps_rows_columns_and_support_lane_separate() {
        let layout = BreakdownLayout::for_panel(
            Rect::new(0, 0, 64, 8),
            DashboardChartMetrics::for_viewport(ViewportClass::Wide),
            4,
            14,
            10,
        );

        let first = layout.row_line_area(0);
        let last = layout.row_line_area(3);

        assert_eq!(layout.label_cell(first).x, layout.label_column_x);
        assert_eq!(layout.label_cell(last).x, layout.label_column_x);
        assert_eq!(layout.delta_cell(first).x, layout.delta_column_x);
        assert_eq!(layout.delta_cell(last).x, layout.delta_column_x);
        assert_eq!(layout.signal_track_cell(first).x, layout.bar_viewport_x);
        assert!(layout.band_area.y >= last.y.saturating_add(last.height));
        assert_eq!(
            layout.support_lane.y,
            layout
                .chart_body
                .y
                .saturating_add(layout.chart_body.height)
                .saturating_add(layout.gap_before_support)
        );
        assert_eq!(layout.support_lane.height, 1);
    }

    #[test]
    fn chart_cluster_packing_keeps_support_lane_close_to_cluster() {
        let packing = pack_chart_cluster_vertically(Rect::new(0, 0, 48, 10), 7, 1);

        assert_eq!(packing.cluster_area, Rect::new(0, 0, 48, 8));
        assert_eq!(packing.support_area, Rect::new(0, 9, 48, 1));
        assert_eq!(packing.gap_before_support, 1);
        assert_eq!(packing.top_padding + packing.bottom_padding, 1);
    }

    #[test]
    fn panel_content_metrics_keeps_support_lane_out_of_chart_body() {
        let metrics = panel_content_metrics(Rect::new(0, 0, 24, 6), 1);

        assert_eq!(metrics.chart.area, Rect::new(0, 0, 24, 5));
        assert_eq!(metrics.support.area, Rect::new(0, 5, 24, 1));
    }

    #[test]
    fn weekly_trends_layout_snaps_headers_selection_and_legend_to_grid() {
        let layout = WeeklyTrendsLayout::for_panel(
            Rect::new(0, 0, 52, 8),
            DashboardChartMetrics::for_viewport(ViewportClass::Wide),
            WeeklyHeatmapMode::Standard,
            7,
            3,
        );

        assert_eq!(layout.header_grid_area.x, layout.grid_viewport.x);
        assert_eq!(layout.legend_area.x, layout.grid_viewport.x);
        assert_eq!(layout.summary_area.x, layout.grid_viewport.x);
        assert_eq!(layout.column_origin(0), layout.grid_viewport.x);
        assert_eq!(
            layout.column_origin(4) - layout.column_origin(3),
            layout.slot_width
        );
        assert_eq!(layout.selected_bracket_origin(6), layout.column_origin(6));
        assert_eq!(layout.group_label_area(0).x, layout.chart_body.x);
        assert_eq!(layout.group_label_area(2).x, layout.chart_body.x);
        assert_eq!(
            layout.group_label_line_area(1).y,
            layout
                .group_area(1)
                .y
                .saturating_add(layout.group_area(1).height / 2)
        );
    }
}
