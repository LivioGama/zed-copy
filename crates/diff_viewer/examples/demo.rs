use assets::Assets;
use diff_viewer::diff_viewer_ui::DiffViewer;
use editor;
use gpui::{
    App, AppContext, Application, Bounds, KeyBinding, WindowBounds, WindowOptions, px, size,
};
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

        cx.bind_keys([
            KeyBinding::new("up", diff_viewer::app::ScrollUp, Some("DiffViewer")),
            KeyBinding::new("down", diff_viewer::app::ScrollDown, Some("DiffViewer")),
            KeyBinding::new("pageup", diff_viewer::app::PageUp, Some("DiffViewer")),
            KeyBinding::new("pagedown", diff_viewer::app::PageDown, Some("DiffViewer")),
            KeyBinding::new("j", diff_viewer::app::NextDiff, Some("DiffViewer")),
            KeyBinding::new("k", diff_viewer::app::PreviousDiff, Some("DiffViewer")),
            KeyBinding::new("]", diff_viewer::app::NextFile, Some("DiffViewer")),
            KeyBinding::new("[", diff_viewer::app::PreviousFile, Some("DiffViewer")),
            KeyBinding::new("d", diff_viewer::app::LoadDemo, Some("DiffViewer")),
        ]);

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
                let left_path = Some(
                    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples/ProvidersOld.tsx"),
                );
                let right_path = Some(
                    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples/ProvidersNew.tsx"),
                );

                let diff_viewer = cx.new(|cx| DiffViewer::new(left_path, right_path, window, cx));

                diff_viewer.update(cx, |viewer: &mut DiffViewer, cx| {
                    viewer.initialize(cx);
                    window.focus(&viewer.focus_handle);
                });

                diff_viewer
            },
        )
        .unwrap();
        cx.activate(true);
    });
}
