pub use crate::app::DiffViewerElement as DiffViewer;

use gpui::ScrollHandle;

impl DiffViewer {
    pub fn new(
        left_path: Option<std::path::PathBuf>,
        right_path: Option<std::path::PathBuf>,
        _window: &mut gpui::Window,
        cx: &mut gpui::App,
    ) -> Self {
        let project_files: Vec<std::path::PathBuf> = vec![left_path.clone(), right_path.clone()]
            .into_iter()
            .flatten()
            .collect();

        let viewing_demo = project_files.is_empty();

        Self {
            app: crate::app::DiffViewerApp {
                state_manager: crate::state::StateManager::new(),
                action_handler: crate::actions::ActionHandler::new(),
                config_manager: crate::config::ConfigManager::new(),
                layout_config: crate::config::LayoutConfig::default(),
                project_files: project_files.clone(),
                current_file_index: 0,
                window_title: "Diff Viewer".to_string(),
                palette: crate::app::DiffPalette::new(),
                viewing_demo,
                toolbar_state: crate::toolbar::ToolbarState::new(project_files),
            },
            scroll_handle: ScrollHandle::new(),
            focus_handle: cx.focus_handle(),
        }
    }

    pub fn initialize(&mut self, _cx: &mut gpui::Context<Self>) {
        self.app.load_demo_view();
    }
}
