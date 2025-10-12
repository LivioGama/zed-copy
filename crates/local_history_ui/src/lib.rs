mod manager;

pub use manager::LocalHistoryManager;

use gpui::App;

/// Initialize the local history system
pub fn init(cx: &mut App) {
    manager::init(cx);
}
