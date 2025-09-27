// crates/diff_viewer/src/lib.rs
// GPUI JetBrains Diff Viewer - Complete rewrite from EGUI
// Faithful conversion maintaining all features and architecture

// Core algorithmic modules (preserved from egui_version)
pub mod diff;
pub mod git;
pub mod models;
pub mod state;

// UI and rendering modules (converted to GPUI)
pub mod actions;
pub mod config;
pub mod highlight_renderer_gpui;
pub mod layout_manager_gpui;
pub mod navigation;
pub mod theme;
pub mod utils;

// Main GPUI components
pub mod diff_viewer_ui;
pub mod perfect_connector_renderer;
pub mod perfect_diff_engine;

// Re-exports for convenience
pub use config::ConfigManager;
pub use diff_viewer_ui::DiffViewer;
pub use diff_viewer_ui::init;
pub use perfect_connector_renderer::{PerfectBezierConnector, PerfectConnectorRenderer};
pub use perfect_diff_engine::{
    DiffView as PerfectDiffView, DisplayLine, ImaraBlockOperation, ImaraDiffAnalysis,
    ImaraDiffBlock, Line, LineType, MappingSegment as PerfectMappingSegment, NewChangeBlock,
    compute_imara_diff_default, generate_perfect_diff_view,
};
pub use theme::JetBrainsTheme;
