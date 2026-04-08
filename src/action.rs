#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    Tick,
    Render,
    NextScreen,
    PreviousScreen,
    RefreshRequested,
    Quit,
}
