use ratatui::{layout::Rect, prelude::Constraint};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewportClass {
    Compact,
    Medium,
    Wide,
}

impl ViewportClass {
    pub fn from_width(width: u16) -> Self {
        if width < 100 {
            Self::Compact
        } else if width < 140 {
            Self::Medium
        } else {
            Self::Wide
        }
    }

    pub fn is_compact(self) -> bool {
        matches!(self, Self::Compact)
    }

    pub fn is_wide(self) -> bool {
        matches!(self, Self::Wide)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UiContext {
    pub area: Rect,
    pub viewport: ViewportClass,
}

impl UiContext {
    pub fn new(area: Rect) -> Self {
        Self {
            area,
            viewport: ViewportClass::from_width(area.width),
        }
    }
}

pub fn equal_columns(count: usize) -> Vec<Constraint> {
    if count == 0 {
        return Vec::new();
    }

    (0..count)
        .map(|_| Constraint::Percentage((100 / count.max(1)) as u16))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::ViewportClass;

    #[test]
    fn viewport_breakpoints_are_stable() {
        assert_eq!(ViewportClass::from_width(90), ViewportClass::Compact);
        assert_eq!(ViewportClass::from_width(120), ViewportClass::Medium);
        assert_eq!(ViewportClass::from_width(160), ViewportClass::Wide);
    }
}
