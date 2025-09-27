// crates/diff_viewer/src/main.rs
// Main entry point for the GPUI JetBrains Diff Viewer
// Complete rewrite from EGUI to GPUI architecture

use assets::Assets;
use diff_viewer::diff_viewer_ui::DiffViewer;
use editor;
use gpui::{App, AppContext, Application, Bounds, WindowBounds, WindowOptions, px, size};
use language;
use project;
use settings;
use std::path::PathBuf;
use theme;
use workspace;

fn main() {
    Application::new().with_assets(Assets).run(|cx: &mut App| {
        println!("🚀 Starting JetBrains Diff Viewer - GPUI Edition");

        // Initialize Zed subsystems
        settings::init(cx);
        theme::init(theme::LoadThemes::JustBase, cx);
        language::init(cx);
        project::Project::init_settings(cx);
        workspace::init_settings(cx);
        editor::init(cx);

        // Create main application window with professional sizing
        let bounds = Bounds::centered(None, size(px(1600.0), px(1000.0)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(gpui::TitlebarOptions {
                    title: Some("JetBrains Diff Viewer - GPUI".into()),
                    ..Default::default()
                }),
                ..Default::default()
            },
            |window, cx| {
                // Initialize with demo files for development
                let left_path = Some(
                    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples/ProvidersOld.tsx"),
                );
                let right_path = Some(
                    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples/ProvidersNew.tsx"),
                );

                // Create the main diff viewer application
                let diff_viewer = cx.new(|cx| DiffViewer::new(left_path, right_path, window, cx));

                // Load the initial diff data
                diff_viewer.update(cx, |viewer: &mut DiffViewer, cx| {
                    viewer.load_diff(cx);
                });

                diff_viewer
            },
        )
        .unwrap();

        cx.activate(true);
    });
}
