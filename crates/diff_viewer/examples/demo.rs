use assets::Assets;
use diff_viewer::diff_viewer_ui::DiffViewer;
use editor;
use gpui::{App, AppContext, Application, Bounds, WindowBounds, WindowOptions, px, size};
use language;
use project;
use settings;
use std::env;
use std::path::PathBuf;
use theme;
use workspace;

fn main() {
    Application::new().with_assets(Assets).run(|cx: &mut App| {
        settings::init(cx);
        theme::init(theme::LoadThemes::JustBase, cx);
        language::init(cx);
        project::Project::init_settings(cx);
        workspace::init_settings(cx);
        editor::init(cx);
        let bounds = Bounds::centered(None, size(px(1200.0), px(800.0)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(gpui::TitlebarOptions {
                    title: Some("Diff Viewer Demo".into()),
                    ..Default::default()
                }),
                ..Default::default()
            },
            |window, cx| {
                // Sample file paths for demo - you can change these to actual files
                let left_path = Some(
                    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples/ProvidersOld.tsx"),
                );
                let right_path = Some(
                    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples/ProvidersNew.tsx"),
                );

                // Create the diff viewer
                let diff_viewer = cx.new(|cx| DiffViewer::new(left_path, right_path, window, cx));

                // Initialize the diff viewer (discover files and load diff)
                diff_viewer.update(cx, |viewer: &mut DiffViewer, cx| {
                    viewer.initialize(cx);
                });

                diff_viewer
            },
        )
        .unwrap();
        cx.activate(true);
    });
}
