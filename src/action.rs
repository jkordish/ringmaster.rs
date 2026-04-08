use crate::app::Screen;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    Tick,
    Quit,
    NextScreen,
    PreviousScreen,
    ShowScreen(Screen),
    RefreshRequested,
}
