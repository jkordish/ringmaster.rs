pub mod dashboard;
pub mod ops;
pub mod timeline;
pub mod trends;

use crate::app::AppState;

pub trait Component {
    fn render(&self, state: &AppState) -> String;
}
