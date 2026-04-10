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

    let base = (100 / count) as u16;
    let remainder = (100 % count) as u16;

    (0..count)
        .map(|index| Constraint::Percentage(base + u16::from(index < remainder as usize)))
        .collect()
}

#[cfg(test)]
mod tests {
    use ratatui::prelude::Constraint;

    use super::{ViewportClass, equal_columns};

    #[test]
    fn viewport_breakpoints_are_stable() {
        assert_eq!(ViewportClass::from_width(90), ViewportClass::Compact);
        assert_eq!(ViewportClass::from_width(120), ViewportClass::Medium);
        assert_eq!(ViewportClass::from_width(160), ViewportClass::Wide);
    }

    #[test]
    fn equal_columns_distributes_remainder() {
        assert_eq!(
            equal_columns(3),
            vec![
                Constraint::Percentage(34),
                Constraint::Percentage(33),
                Constraint::Percentage(33),
            ]
        );
        assert_eq!(
            equal_columns(6)
                .into_iter()
                .map(|constraint| match constraint {
                    Constraint::Percentage(value) => u32::from(value),
                    _ => 0,
                })
                .sum::<u32>(),
            100
        );
    }
}
