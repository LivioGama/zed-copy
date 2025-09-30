use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Result;
use gpui::{
    Background, FocusHandle, Hsla, IntoElement, ParentElement, PathBuilder, Render, ScrollHandle,
    Styled, Window, actions, canvas, div, hsla, point, px,
};
use ui::{Color as UiColor, Label, LabelCommon, h_flex, prelude::*, v_flex};

actions!(
    diff_viewer,
    [
        ScrollUp,
        ScrollDown,
        PageUp,
        PageDown,
        NextDiff,
        PreviousDiff,
        NextFile,
        PreviousFile,
        LoadDemo,
    ]
);

use crate::actions::ActionHandler;
use crate::config::{ConfigManager, LayoutConfig, WindowConfig};
use crate::core::app_bootstrap::AppBootstrap;
use crate::diff::imara::compute_imara_diff_default;
use crate::diff::parser::create_complete_side_by_side_with_diff;
use crate::git::GitOps;
// use crate::gpui::{Canvas, Color, GpuiApp, GpuiEvent, KeyCode, KeyState};

use crate::models::line::LineType;
use crate::navigation::{NavigationAction, NavigationState};
use crate::rendering::Color;
use crate::state::StateManager;
use crate::sync::{build_anchors_from_blocks, build_connector_curves, build_mapping_segments};
use crate::toolbar::{DiffMode, ToolbarState};

const HEADER_HEIGHT: f32 = 36.0;
const GUTTER_WIDTH: f32 = 64.0;

const COLOR_HEADER: Color = Color::from_rgb(32, 35, 46);

pub struct DiffData {
    pub file_label: String,
    pub left_lines: Vec<crate::models::line::DisplayLine>,
    pub right_lines: Vec<crate::models::line::DisplayLine>,
    pub change_blocks: Vec<crate::models::diff::ChangeBlock>,
    pub imara_analysis: crate::diff::imara::ImaraDiffAnalysis,
    pub anchors: Vec<crate::models::diff::AnchorPoint>,
    pub mapping_segments: Vec<crate::models::diff::MappingSegment>,
    pub connector_curves: Vec<crate::models::ConnectorCurve>,
    pub header_height: f32,
}
const COLOR_HEADER_TEXT: Color = Color::from_rgb(224, 229, 243);
const COLOR_INSTRUCTIONS: Color = Color::from_rgb(160, 170, 190);
const COLOR_SELECTION: Color = Color::from_rgb(96, 130, 200);

const HEADER_TITLE_SIZE: f32 = 18.0;
const HEADER_INSTRUCTION_SIZE: f32 = 14.0;
const INSTRUCTIONS_WIDTH: f32 = 420.0;
const TITLE_Y_OFFSET: f32 = 6.0;
const INSTRUCTION_BASELINE_OFFSET: f32 = 6.0;
const TEXT_PADDING: f32 = 8.0;
const MIN_HEADER_PADDING: f32 = 12.0;
const MIN_COLUMN_WIDTH: f32 = 100.0;
const GUTTER_FONT_SIZE: f32 = 14.0;
const CONTENT_FONT_SIZE: f32 = 14.0;

#[derive(Clone)]
pub struct DiffPalette {
    background: Hsla,
    text_primary: Hsla,
    line_numbers: Hsla,
    addition_background: Hsla,
    deletion_background: Hsla,
    modification_background: Hsla,
    addition_ribbon: Hsla,
    deletion_ribbon: Hsla,
    modification_ribbon: Hsla,
    crushed_insert: Hsla,
    crushed_delete: Hsla,
    gutter_background: Hsla,
    selection: Hsla,
}

impl DiffPalette {
    pub fn new() -> Self {
        Self {
            background: hsla(30.0 / 360.0, 0.2, 0.15, 1.0),
            text_primary: hsla(0.0, 0.0, 0.86, 1.0),
            line_numbers: hsla(210.0 / 360.0, 0.1, 0.45, 1.0),
            addition_background: hsla(120.0 / 360.0, 0.4, 0.35, 1.0),
            deletion_background: hsla(0.0, 0.5, 0.35, 1.0),
            modification_background: hsla(210.0 / 360.0, 0.5, 0.4, 1.0),
            addition_ribbon: hsla(120.0 / 360.0, 0.7, 0.5, 1.0),
            deletion_ribbon: hsla(0.0, 0.8, 0.5, 1.0),
            modification_ribbon: hsla(210.0 / 360.0, 0.7, 0.5, 1.0),
            crushed_insert: hsla(120.0 / 360.0, 0.7, 0.5, 1.0),
            crushed_delete: hsla(0.0, 0.8, 0.5, 1.0),
            gutter_background: hsla(220.0 / 360.0, 0.1, 0.12, 1.0),
            selection: hsla(220.0 / 360.0, 0.15, 0.25, 1.0),
        }
    }
}

pub struct DiffViewerApp {
    pub state_manager: StateManager,
    pub action_handler: ActionHandler,
    pub config_manager: ConfigManager,
    pub layout_config: LayoutConfig,
    pub project_files: Vec<PathBuf>,
    pub current_file_index: usize,
    pub window_title: String,
    pub palette: DiffPalette,
    pub viewing_demo: bool,
    pub toolbar_state: crate::toolbar::ToolbarState,
}

impl DiffViewerApp {
    pub fn from_bootstrap(bootstrap: AppBootstrap) -> Result<Self> {
        let viewing_demo = bootstrap.project_files.is_empty();
        let mut app = Self {
            state_manager: bootstrap.state_manager,
            action_handler: bootstrap.action_handler,
            config_manager: bootstrap.config_manager,
            layout_config: LayoutConfig::default(),
            project_files: bootstrap.project_files.clone(),
            current_file_index: 0,
            window_title: "Diff Viewer".to_string(),
            palette: DiffPalette::new(),
            viewing_demo,
            toolbar_state: crate::toolbar::ToolbarState::new(bootstrap.project_files),
        };

        app.layout_config = app.config_manager.get_config().layout.clone();
        app.toolbar_state = ToolbarState::new(app.project_files.clone());

        if !app.project_files.is_empty() {
            let first = app.project_files[0].clone();
            app.viewing_demo = false;
            app.load_file_diff(&first);
        } else {
            app.load_demo_view();
        }

        Ok(app)
    }

    pub fn title(&self) -> &str {
        &self.window_title
    }

    fn device_scale(&self) -> f32 {
        1.0
    }

    pub fn header_height(&self) -> f32 {
        HEADER_HEIGHT * self.device_scale()
    }

    fn gutter_width(&self) -> f32 {
        GUTTER_WIDTH * self.device_scale()
    }

    fn connector_column_width(&self) -> f32 {
        self.layout_config.connector_column_width * self.device_scale()
    }

    fn pane_padding(&self) -> f32 {
        self.layout_config.pane_padding * self.device_scale()
    }

    fn min_header_padding(&self) -> f32 {
        MIN_HEADER_PADDING * self.device_scale()
    }

    fn header_title_size(&self) -> f32 {
        HEADER_TITLE_SIZE * self.device_scale()
    }

    fn header_instruction_size(&self) -> f32 {
        HEADER_INSTRUCTION_SIZE * self.device_scale()
    }

    fn instructions_width(&self) -> f32 {
        INSTRUCTIONS_WIDTH * self.device_scale()
    }

    fn header_title_offset(&self) -> f32 {
        TITLE_Y_OFFSET * self.device_scale()
    }

    fn header_instruction_offset(&self) -> f32 {
        INSTRUCTION_BASELINE_OFFSET * self.device_scale()
    }

    fn text_padding(&self) -> f32 {
        TEXT_PADDING * self.device_scale()
    }

    fn min_column_width(&self) -> f32 {
        MIN_COLUMN_WIDTH * self.device_scale()
    }

    fn gutter_font_size(&self) -> f32 {
        GUTTER_FONT_SIZE * self.device_scale()
    }

    fn content_font_size(&self) -> f32 {
        self.config_manager.get_font_manager().buffer_font_size() * self.device_scale()
    }

    pub fn line_height(&self) -> f32 {
        WindowConfig::get_line_height(&self.config_manager) * self.device_scale()
    }

    fn navigation_state(&self) -> &NavigationState {
        &self.state_manager.get_current_state().navigation_state
    }

    fn navigation_state_mut(&mut self) -> &mut NavigationState {
        &mut self.state_manager.get_current_state_mut().navigation_state
    }

    fn scroll_offset(&self) -> f32 {
        let state = self.state_manager.get_current_state();
        state.scroll_offset
    }

    fn set_scroll_offset(&mut self, offset: f32) {
        let max = self.max_scroll_offset();
        let clamped = offset.clamp(0.0, max);
        let state = self.state_manager.get_current_state_mut();
        state.set_scroll_offset(clamped);
    }

    fn max_scroll_offset(&self) -> f32 {
        let state = self.state_manager.get_current_state();
        let total_lines = state.left_lines.len().max(state.right_lines.len()) as f32;
        let total_height = total_lines * self.line_height();
        let available = 800.0;
        if total_height <= available || available <= 0.0 {
            0.0
        } else {
            total_height - available
        }
    }

    fn handle_navigation_action(&mut self, action: NavigationAction) {
        match action {
            NavigationAction::NextDiffBlock => {
                if let Some(idx) = self.navigation_state_mut().next_block() {
                    self.goto_block(idx);
                }
            }
            NavigationAction::PreviousDiffBlock => {
                if let Some(idx) = self.navigation_state_mut().previous_block() {
                    self.goto_block(idx);
                }
            }
            NavigationAction::NextConnector => {
                if let Some(idx) = self.navigation_state_mut().next_connector() {
                    if let Some(curve) = self
                        .state_manager
                        .get_current_state()
                        .connector_curves
                        .get(idx)
                    {
                        self.scroll_to_line(curve.focus_line);
                    }
                }
            }
            NavigationAction::PreviousConnector => {
                if let Some(idx) = self.navigation_state_mut().previous_connector() {
                    if let Some(curve) = self
                        .state_manager
                        .get_current_state()
                        .connector_curves
                        .get(idx)
                    {
                        self.scroll_to_line(curve.focus_line);
                    }
                }
            }
            NavigationAction::ApplyHunk
            | NavigationAction::RevertHunk
            | NavigationAction::StageHunk => {
                let current_block = self.navigation_state().current_block_index;
                let result = self.action_handler.execute_action(action, current_block);
                if !result.success {
                    eprintln!("Action failed: {}", result.message);
                }
            }
            NavigationAction::None => {}
        }
    }

    pub fn load_diff_from_strings(
        &mut self,
        left_content: &str,
        right_content: &str,
        file_label: &str,
    ) {
        let (left_lines, right_lines, change_blocks) =
            create_complete_side_by_side_with_diff(left_content, right_content, "");
        let imara_analysis = compute_imara_diff_default(left_content, right_content);
        let line_height = self.line_height();
        let anchors = build_anchors_from_blocks(&change_blocks, line_height);
        let mapping_segments = build_mapping_segments(&anchors);
        let connector_curves = build_connector_curves(&imara_analysis);

        let header_height = self.header_height();
        let viewport_height = 800.0;
        self.state_manager.update_state(|state| {
            state.current_file = file_label.to_string();
            state.left_lines = left_lines;
            state.right_lines = right_lines;
            state.change_blocks = change_blocks;
            state.imara_analysis = imara_analysis;
            state.anchors = anchors;
            state.mapping_segments = mapping_segments;
            state.connector_curves = connector_curves.clone();
            state.reset_navigation();
            state.viewport_height = (viewport_height - header_height).max(0.0);
            state.scroll_offset = 0.0;
            state.update_navigation_state();
        });

        self.set_scroll_offset(0.0);
        self.viewing_demo = false;
        self.update_window_title(file_label.to_string());
    }

    pub fn load_diff_from_strings_async(
        left_content: String,
        right_content: String,
        file_label: String,
        line_height: f32,
        header_height: f32,
    ) -> DiffData {
        let (left_lines, right_lines, change_blocks) =
            create_complete_side_by_side_with_diff(&left_content, &right_content, "");
        let imara_analysis = compute_imara_diff_default(&left_content, &right_content);
        let anchors = build_anchors_from_blocks(&change_blocks, line_height);
        let mapping_segments = build_mapping_segments(&anchors);
        let connector_curves = build_connector_curves(&imara_analysis);

        DiffData {
            file_label,
            left_lines,
            right_lines,
            change_blocks,
            imara_analysis,
            anchors,
            mapping_segments,
            connector_curves,
            header_height,
        }
    }

    pub fn apply_diff_data(&mut self, data: DiffData) {
        let viewport_height = 800.0;
        self.state_manager.update_state(|state| {
            state.current_file = data.file_label.clone();
            state.left_lines = data.left_lines;
            state.right_lines = data.right_lines;
            state.change_blocks = data.change_blocks;
            state.imara_analysis = data.imara_analysis;
            state.anchors = data.anchors;
            state.mapping_segments = data.mapping_segments;
            state.connector_curves = data.connector_curves;
            state.reset_navigation();
            state.viewport_height = (viewport_height - data.header_height).max(0.0);
            state.scroll_offset = 0.0;
            state.update_navigation_state();
        });

        self.set_scroll_offset(0.0);
        self.viewing_demo = false;
        self.update_window_title(data.file_label);
    }

    fn load_file_diff(&mut self, file_path: &Path) {
        eprintln!("Loading diff for file: {:?}", file_path);
        let git_ops = GitOps::with_current_dir();
        let file_path_str = file_path.to_string_lossy().to_string();

        let current_content = fs::read_to_string(file_path).unwrap_or_else(|err| {
            eprintln!("❌ Failed to read current file ({}), using fallback", err);
            String::new()
        });

        let original_content = match git_ops.show_file("HEAD", &file_path_str) {
            crate::git::GitResult {
                success: true,
                stdout,
                ..
            } => stdout,
            _ => {
                eprintln!("❌ Failed to read original from git, using current as original");
                current_content.clone()
            }
        };

        let diff_text = match git_ops.diff_file(None, None, &file_path_str) {
            crate::git::GitResult {
                success: true,
                stdout,
                ..
            } => stdout,
            _ => String::new(),
        };

        let (left_lines, right_lines, change_blocks) =
            create_complete_side_by_side_with_diff(&original_content, &current_content, &diff_text);
        let imara_analysis = compute_imara_diff_default(&original_content, &current_content);
        let line_height = self.line_height();
        let anchors = build_anchors_from_blocks(&change_blocks, line_height);
        let mapping_segments = build_mapping_segments(&anchors);
        let connector_curves = build_connector_curves(&imara_analysis);

        let header_height = self.header_height();
        let viewport_height = 800.0;
        self.state_manager.update_state(|state| {
            state.current_file = file_path_str.clone();
            state.left_lines = left_lines;
            state.right_lines = right_lines;
            state.change_blocks = change_blocks;
            state.imara_analysis = imara_analysis;
            state.anchors = anchors;
            state.mapping_segments = mapping_segments;
            state.connector_curves = connector_curves.clone();
            state.reset_navigation();
            state.viewport_height = (viewport_height - header_height).max(0.0);
            state.scroll_offset = 0.0;
            state.update_navigation_state();
        });

        self.set_scroll_offset(0.0);
        self.viewing_demo = false;
        if !self.project_files.is_empty() {
            self.toolbar_state.current_mode = DiffMode::ProjectFile(self.current_file_index);
        }
        self.update_window_title(
            file_path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or(file_path_str),
        );
    }

    pub fn load_demo_view(&mut self) {
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let left_path = manifest_dir.join("examples/ProvidersOld.tsx");
        let right_path = manifest_dir.join("examples/ProvidersNew.tsx");

        let fallback_original =
            "function App() {\n  return <div>Hello World</div>;\n}\n\nexport default App;\n";
        let fallback_current = "import React from 'react';\nimport { BrowserRouter as Router, Routes, Route } from 'react-router-dom';\nimport { ThemeProvider } from './theme';\nimport { AuthProvider } from './auth';\nimport { NotificationProvider } from './notifications';\n\nfunction AppProviders({ children }) {\n  return (\n    <ThemeProvider>\n      <AuthProvider>\n        <NotificationProvider>\n          <Router>\n            {children}\n          </Router>\n        </NotificationProvider>\n      </AuthProvider>\n    </ThemeProvider>\n  );\n}\n\nexport default AppProviders;\n";

        let demo_original = fs::read_to_string(&left_path).unwrap_or_else(|err| {
            eprintln!(
                "⚠️ Failed to read demo original from {:?}: {} — using fallback",
                left_path, err
            );
            fallback_original.to_string()
        });

        let demo_current = fs::read_to_string(&right_path).unwrap_or_else(|err| {
            eprintln!(
                "⚠️ Failed to read demo current from {:?}: {} — using fallback",
                right_path, err
            );
            fallback_current.to_string()
        });

        let demo_label = format!(
            "{} → {}",
            left_path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("ProvidersOld.tsx"),
            right_path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("ProvidersNew.tsx"),
        );

        let (left_lines, right_lines, change_blocks) =
            create_complete_side_by_side_with_diff(&demo_original, &demo_current, "");
        let imara_analysis = compute_imara_diff_default(&demo_original, &demo_current);
        let connector_curves = build_connector_curves(&imara_analysis);
        let line_height = self.line_height();
        let anchors = build_anchors_from_blocks(&change_blocks, line_height);
        let mapping_segments = build_mapping_segments(&anchors);
        let viewport_height = 800.0;
        let header_height = self.header_height();

        self.state_manager.update_state(|state| {
            state.current_file = demo_label.clone();
            state.left_lines = left_lines;
            state.right_lines = right_lines;
            state.change_blocks = change_blocks;
            state.imara_analysis = imara_analysis.clone();
            state.anchors = anchors.clone();
            state.mapping_segments = mapping_segments.clone();
            state.connector_curves = connector_curves.clone();
            state.reset_navigation();
            state.viewport_height = (viewport_height - header_height).max(0.0);
            state.scroll_offset = 0.0;
            state.update_navigation_state();
        });

        self.set_scroll_offset(0.0);
        self.viewing_demo = true;
        self.toolbar_state.go_to_default();
        self.update_window_title(demo_label);
    }

    fn refresh_layout_metrics(&mut self) {
        let line_height = self.line_height();
        let viewport_height = 800.0;
        let header_height = self.header_height();
        self.state_manager.update_state(|state| {
            let anchors = build_anchors_from_blocks(&state.change_blocks, line_height);
            let mapping_segments = build_mapping_segments(&anchors);
            state.anchors = anchors;
            state.mapping_segments = mapping_segments;
            state.viewport_height = (viewport_height - header_height).max(0.0);
            state.update_navigation_state();
        });
    }

    fn update_window_title(&mut self, file_label: String) {
        let suffix = if self.viewing_demo {
            format!("Demo • {}", file_label)
        } else if !self.project_files.is_empty() {
            let total = self.project_files.len();
            let index = (self.current_file_index % total) + 1;
            format!("[{}/{}] {}", index, total, file_label)
        } else {
            file_label
        };
        self.window_title = format!("JetBrains Diff Viewer - {}", suffix);
    }

    fn scroll_lines(&mut self, lines: f32) {
        let line_height = self.line_height();
        let target = self.scroll_offset() + lines * line_height;
        self.set_scroll_offset(target);
    }

    fn scroll_pages(&mut self, pages: f32) {
        let line_height = self.line_height();
        let page = (800.0 - self.header_height()).max(line_height);
        let target = self.scroll_offset() + pages * page;
        self.set_scroll_offset(target);
    }

    fn goto_block(&mut self, index: usize) {
        if let Some(block) = self
            .state_manager
            .get_current_state()
            .change_blocks
            .get(index)
            .cloned()
        {
            let target = block.start_line as f32 * self.line_height();
            self.set_scroll_offset(target);
        }
    }

    fn scroll_to_line(&mut self, line_index: usize) {
        let target = line_index as f32 * self.line_height();
        self.set_scroll_offset(target);
    }

    fn next_file(&mut self) {
        if self.project_files.is_empty() {
            if !self.viewing_demo {
                self.load_demo_view();
            }
            return;
        }
        self.toolbar_state.go_next();
        match self.toolbar_state.current_mode {
            DiffMode::ProjectFile(index) => {
                self.current_file_index = index;
                if let Some(path) = self.project_files.get(index).cloned() {
                    self.load_file_diff(&path);
                }
            }
            DiffMode::DefaultDemo => self.load_demo_view(),
        }
    }

    fn previous_file(&mut self) {
        if self.project_files.is_empty() {
            if !self.viewing_demo {
                self.load_demo_view();
            }
            return;
        }
        self.toolbar_state.go_previous();
        match self.toolbar_state.current_mode {
            DiffMode::ProjectFile(index) => {
                self.current_file_index = index;
                if let Some(path) = self.project_files.get(index).cloned() {
                    self.load_file_diff(&path);
                }
            }
            DiffMode::DefaultDemo => self.load_demo_view(),
        }
    }

    fn handle_character_key(&mut self, ch: char) {
        match ch.to_ascii_lowercase() {
            '[' => self.previous_file(),
            ']' => self.next_file(),
            'j' => self.handle_navigation_action(NavigationAction::NextDiffBlock),
            'k' => self.handle_navigation_action(NavigationAction::PreviousDiffBlock),
            'n' => self.handle_navigation_action(NavigationAction::NextConnector),
            'p' => self.handle_navigation_action(NavigationAction::PreviousConnector),
            'a' => self.handle_navigation_action(NavigationAction::ApplyHunk),
            'r' => self.handle_navigation_action(NavigationAction::RevertHunk),
            's' => self.handle_navigation_action(NavigationAction::StageHunk),
            'd' => self.load_demo_view(),
            _ => self.handle_navigation_action(NavigationAction::None),
        }
    }

    fn header_instructions_text(&self) -> String {
        let base = "↑/↓ scroll  •  PgUp/PgDn jump  •  [/] change file  •  J/K next diff  •  D demo";
        let total = self.toolbar_state.project_files.len();
        if total == 0 {
            base.to_string()
        } else if self.viewing_demo {
            format!("{}  •  Viewing demo", base)
        } else {
            let index = (self.current_file_index % total) + 1;
            format!("{}  •  File {}/{}", base, index, total)
        }
    }
}

pub struct DiffViewerElement {
    pub app: DiffViewerApp,
    pub scroll_handle: ScrollHandle,
    pub focus_handle: FocusHandle,
}

impl DiffViewerElement {
    fn scroll_up(&mut self, _: &ScrollUp, _window: &mut Window, _cx: &mut gpui::Context<Self>) {
        self.app.scroll_lines(-3.0);
        self.scroll_handle
            .set_offset(gpui::Point::new(px(0.0), px(self.app.scroll_offset())));
    }

    fn scroll_down(&mut self, _: &ScrollDown, _window: &mut Window, _cx: &mut gpui::Context<Self>) {
        self.app.scroll_lines(3.0);
        self.scroll_handle
            .set_offset(gpui::Point::new(px(0.0), px(self.app.scroll_offset())));
    }

    fn page_up(&mut self, _: &PageUp, _window: &mut Window, _cx: &mut gpui::Context<Self>) {
        self.app.scroll_pages(-1.0);
        self.scroll_handle
            .set_offset(gpui::Point::new(px(0.0), px(self.app.scroll_offset())));
    }

    fn page_down(&mut self, _: &PageDown, _window: &mut Window, _cx: &mut gpui::Context<Self>) {
        self.app.scroll_pages(1.0);
        self.scroll_handle
            .set_offset(gpui::Point::new(px(0.0), px(self.app.scroll_offset())));
    }

    fn next_diff(&mut self, _: &NextDiff, _window: &mut Window, _cx: &mut gpui::Context<Self>) {
        self.app
            .handle_navigation_action(crate::navigation::NavigationAction::NextDiffBlock);
    }

    fn previous_diff(
        &mut self,
        _: &PreviousDiff,
        _window: &mut Window,
        _cx: &mut gpui::Context<Self>,
    ) {
        self.app
            .handle_navigation_action(crate::navigation::NavigationAction::PreviousDiffBlock);
    }

    fn next_file(&mut self, _: &NextFile, _window: &mut Window, _cx: &mut gpui::Context<Self>) {
        self.app.next_file();
    }

    fn previous_file(
        &mut self,
        _: &PreviousFile,
        _window: &mut Window,
        _cx: &mut gpui::Context<Self>,
    ) {
        self.app.previous_file();
    }

    fn load_demo(&mut self, _: &LoadDemo, _window: &mut Window, _cx: &mut gpui::Context<Self>) {
        self.app.load_demo_view();
    }
}

// fn draw_header(
//     app: &DiffViewerApp,
//     cx: &mut gpui::Context<DiffViewerElement>,
//     viewport_size: gpui::Size<gpui::Pixels>,
//     font_id: egui::FontId,
// ) {
//     let width = viewport_size.width.0;
//     let header_height = app.header_height();
//     let bounds = gpui::Bounds::new(
//         gpui::Point::new(gpui::px(0.0), gpui::px(0.0)),
//         gpui::Size::new(gpui::px(width), gpui::px(header_height)),
//     );
//     cx.fill_rect(bounds, color_to_hsla(COLOR_HEADER));
//     let padding = app.pane_padding().max(app.min_header_padding());
//     let title_y = app.header_title_offset();
//     let title_bounds = gpui::Bounds::new(
//         gpui::Point::new(gpui::px(padding), gpui::px(title_y)),
//         gpui::Size::new(gpui::px(1000.0), gpui::px(app.header_title_size())),
//     );
//     cx.draw_text(
//         &app.window_title,
//         font_id,
//         gpui::px(app.header_title_size()),
//         title_bounds,
//         color_to_hsla(COLOR_HEADER_TEXT),
//     );
//     let instructions = app.header_instructions_text();
//     let inst_x = (width - padding - app.instructions_width()).max(padding);
//     let instructions_y = title_y + app.header_title_size() - app.header_instruction_offset();
//     let instructions_bounds = gpui::Bounds::new(
//         gpui::Point::new(gpui::px(inst_x), gpui::px(instructions_y)),
//         gpui::Size::new(gpui::px(1000.0), gpui::px(app.header_instruction_size())),
//     );
//     cx.draw_text(
//         &instructions,
//         font_id,
//         gpui::px(app.header_instruction_size()),
//         instructions_bounds,
//         color_to_hsla(COLOR_INSTRUCTIONS),
//     );
// }

fn cubic_bezier(
    p0: gpui::Point<gpui::Pixels>,
    p1: gpui::Point<gpui::Pixels>,
    p2: gpui::Point<gpui::Pixels>,
    p3: gpui::Point<gpui::Pixels>,
    t: f32,
) -> gpui::Point<gpui::Pixels> {
    let t2 = t * t;
    let t3 = t2 * t;
    let mt = 1.0 - t;
    let mt2 = mt * mt;
    let mt3 = mt2 * mt;

    let x = mt3 * p0.x.0 + 3.0 * mt2 * t * p1.x.0 + 3.0 * mt * t2 * p2.x.0 + t3 * p3.x.0;
    let y = mt3 * p0.y.0 + 3.0 * mt2 * t * p1.y.0 + 3.0 * mt * t2 * p2.y.0 + t3 * p3.y.0;

    point(px(x), px(y))
}

fn highlight_color() -> Color {
    COLOR_SELECTION
}

// fn draw_diff_columns(
//     app: &DiffViewerApp,
//     cx: &mut gpui::Context,
//     viewport_size: Size<Pixels>,
//     font_id: FontId,
// ) {
//     let width = viewport_size.width.0;
//     let height = viewport_size.height.0;
//     let header_height = app.header_height();
//     let available_height = height - header_height;
//     let bounds = Bounds::new(
//         Point::new(px(0.0), px(header_height)),
//         Size::new(px(width), px(available_height)),
//     );
//     cx.fill_rect(bounds, color_to_hsla(app.palette.background));
//     let gutter_width = app.gutter_width();
//     let connector_column_width = app.connector_column_width();
//     let pane_padding = app.pane_padding();
//     let left_x = pane_padding;
//     let right_x = width / 2.0 + connector_column_width / 2.0;
//     let column_width = (width - connector_column_width) / 2.0 - pane_padding * 2.0;
//     let font_size = app.content_font_size();
//     let line_height = app.line_height();
//     let left_scroll = app.scroll_offset();
//     let right_scroll = app.scroll_offset();
//     let state = app.state_manager.get_current_state();
//     let active_block = state
//         .change_blocks
//         .get(state.navigation_state.current_block_index)
//         .cloned();
//     draw_column_lines(
//         cx,
//         &state.left_lines,
//         left_x,
//         gutter_width,
//         column_width,
//         font_size,
//         left_scroll,
//         available_height,
//         line_height,
//         header_height,
//         &active_block,
//         font_id,
//     );
//     draw_column_lines(
//         cx,
//         &state.right_lines,
//         right_x,
//         gutter_width,
//         column_width,
//         font_size,
//         right_scroll,
//         available_height,
//         line_height,
//         header_height,
//         &active_block,
//         font_id,
//     );
// }

// fn draw_column_lines(
//     cx: &mut gpui::Context,
//     lines: &[DisplayLine],
//     column_x: f32,
//     gutter_width: f32,
//     column_width: f32,
//     font_size: f32,
//     scroll_offset: f32,
//     available_height: f32,
//     line_height: f32,
//     header_y: f32,
//     active_block: &Option<crate::models::diff::ChangeBlock>,
//     font_id: FontId,
// ) {
//     if lines.is_empty() {
//         return;
//     }
//     for (i, line) in lines.iter().enumerate() {
//         let y = header_y + (i as f32 * line_height) - scroll_offset;
//         if y + line_height > header_y && y < header_y + available_height {
//             draw_line(
//                 cx,
//                 line,
//                 i,
//                 column_x,
//                 gutter_width,
//                 column_width,
//                 font_size,
//                 y,
//                 active_block,
//                 font_id,
//             );
//         }
//     }
// }

// fn draw_line(
//     cx: &mut gpui::Context,
//     line: &DisplayLine,
//     line_index: usize,
//     column_x: f32,
//     gutter_width: f32,
//     column_width: f32,
//     font_size: f32,
//     y: f32,
//     active_block: &Option<crate::models::diff::ChangeBlock>,
//     font_id: FontId,
// ) {
//     let base_color = match line.line_type {
//         LineType::Context => Color::from_rgb(23, 25, 33),
//         LineType::Addition => Color::from_rgb(52, 85, 52),
//         LineType::Deletion => Color::from_rgb(85, 56, 56),
//         LineType::Modification => Color::from_rgb(50, 66, 98),
//     };
//     let background_bounds = Bounds::new(
//         Point::new(px(column_x), px(y)),
//         Size::new(px(column_width), px(line_height)),
//     );
//     cx.fill_rect(background_bounds, color_to_hsla(base_color));
//     if let Some(block) = active_block {
//         if (block.start_line..=block.end_line).contains(&line_index) {
//             let highlight_color = highlight_color();
//             cx.fill_rect(background_bounds, color_to_hsla(highlight_color));
//         }
//     }
//     let number = line
//         .original_line_num
//         .map(|n| n.to_string())
//         .unwrap_or_else(|| "".to_string());
//     let gutter_color = Color::from_rgb(153, 153, 153);
//     let padding = 5.0; // placeholder
//     let gutter_font_size = 14.0; // placeholder
//     let number_bounds = Bounds::new(
//         Point::new(
//             px(column_x + padding),
//             px(y + 2.0), // placeholder
//         ),
//         Size::new(px(100.0), px(gutter_font_size)),
//     );
//     cx.draw_text(
//         &number,
//         font_id,
//         px(gutter_font_size),
//         number_bounds,
//         color_to_hsla(gutter_color),
//     );
//     let text = line.content.replace('\t', "    ");
//     let text_color = Color::from_rgb(212, 212, 212);
//     let text_bounds = Bounds::new(
//         Point::new(
//             px(column_x + gutter_width + padding),
//             px(y + 2.0), // placeholder
//         ),
//         Size::new(px(column_width - gutter_width - padding), px(font_size)),
//     );
//     cx.draw_text(
//         &text,
//         font_id,
//         px(font_size),
//         text_bounds,
//         color_to_hsla(text_color),
//     );
// }

impl DiffViewerElement {
    fn render_connectors(&self) -> impl IntoElement {
        let state = self.app.state_manager.get_current_state();
        let palette = self.app.palette.clone();
        let line_height = CONTENT_FONT_SIZE * 1.2;
        let _change_blocks = state.change_blocks.clone();
        let left_lines = state.left_lines.clone();
        let imara_analysis = state.imara_analysis.clone();
        let scroll_handle = self.scroll_handle.clone();

        canvas(
            move |_, _, _| {},
            move |bounds, _, window, _| {
                let gutter_width = bounds.size.width.0;
                let control_offset = gutter_width * 0.3;

                let scroll_offset = scroll_handle.offset();
                let left_scroll = scroll_offset;
                let right_scroll = scroll_offset;

                let mut cumulative_left_y = 0.0;
                let mut cumulative_right_y = 0.0;
                let mut left_line_idx = 0;
                let mut right_line_idx = 0;

                for imara_block in &imara_analysis.blocks {
                    if !imara_block.is_change() {
                        continue;
                    }

                    let target_left_line = if imara_block.is_pure_insertion() {
                        imara_block.right_range.start.saturating_sub(
                            imara_analysis
                                .blocks
                                .iter()
                                .filter(|b| {
                                    b.right_range.end <= imara_block.right_range.start
                                        && b.is_pure_insertion()
                                })
                                .map(|b| b.right_range.len())
                                .sum::<usize>(),
                        )
                    } else {
                        imara_block.left_range.start
                    };

                    let target_right_line = if imara_block.is_pure_deletion() {
                        imara_block.left_range.start.saturating_sub(
                            imara_analysis
                                .blocks
                                .iter()
                                .filter(|b| {
                                    b.left_range.end <= imara_block.left_range.start
                                        && b.is_pure_deletion()
                                })
                                .map(|b| b.left_range.len())
                                .sum::<usize>(),
                        )
                    } else {
                        imara_block.right_range.start
                    };

                    while left_line_idx < target_left_line.min(left_lines.len()) {
                        cumulative_left_y += line_height;
                        left_line_idx += 1;
                    }

                    while right_line_idx < target_right_line.min(left_lines.len()) {
                        cumulative_right_y += line_height;
                        right_line_idx += 1;
                    }

                    if imara_block.is_pure_insertion() && !imara_block.right_range.is_empty() {
                        let crushed_y = cumulative_left_y + left_scroll.y.0;
                        let right_top = cumulative_right_y + right_scroll.y.0;
                        let right_bottom = (cumulative_right_y
                            + imara_block.right_range.len() as f32 * line_height)
                            + right_scroll.y.0;

                        let color = palette.addition_ribbon;

                        let mut builder = PathBuilder::fill();
                        let segments = 32;

                        for i in 0..=segments {
                            let t = i as f32 / segments as f32;
                            let top_point = cubic_bezier(
                                point(px(bounds.origin.x.0), px(bounds.origin.y.0 + crushed_y)),
                                point(
                                    px(bounds.origin.x.0 + control_offset),
                                    px(bounds.origin.y.0 + crushed_y),
                                ),
                                point(
                                    px(bounds.origin.x.0 + gutter_width - control_offset),
                                    px(bounds.origin.y.0 + right_top),
                                ),
                                point(
                                    px(bounds.origin.x.0 + gutter_width),
                                    px(bounds.origin.y.0 + right_top),
                                ),
                                t,
                            );
                            if i == 0 {
                                builder.move_to(top_point);
                            } else {
                                builder.line_to(top_point);
                            }
                        }

                        for i in (0..=segments).rev() {
                            let t = i as f32 / segments as f32;
                            let bottom_point = cubic_bezier(
                                point(
                                    px(bounds.origin.x.0),
                                    px(bounds.origin.y.0 + crushed_y + 2.0),
                                ),
                                point(
                                    px(bounds.origin.x.0 + control_offset),
                                    px(bounds.origin.y.0 + crushed_y + 2.0),
                                ),
                                point(
                                    px(bounds.origin.x.0 + gutter_width - control_offset),
                                    px(bounds.origin.y.0 + right_bottom),
                                ),
                                point(
                                    px(bounds.origin.x.0 + gutter_width),
                                    px(bounds.origin.y.0 + right_bottom),
                                ),
                                t,
                            );
                            builder.line_to(bottom_point);
                        }

                        if let Ok(path) = builder.build() {
                            let background: Background = color.into();
                            window.paint_path(path, background);
                        }

                        cumulative_left_y += 2.0;
                        cumulative_right_y += imara_block.right_range.len() as f32 * line_height;
                        right_line_idx += imara_block.right_range.len();
                        continue;
                    }

                    if imara_block.is_pure_deletion() && !imara_block.left_range.is_empty() {
                        let left_top = cumulative_left_y + left_scroll.y.0;
                        let left_bottom = (cumulative_left_y
                            + imara_block.left_range.len() as f32 * line_height)
                            + left_scroll.y.0;
                        let crushed_y = cumulative_right_y + right_scroll.y.0;

                        let color = palette.deletion_ribbon;

                        let mut builder = PathBuilder::fill();
                        let segments = 32;

                        for i in 0..=segments {
                            let t = i as f32 / segments as f32;
                            let top_point = cubic_bezier(
                                point(px(bounds.origin.x.0), px(bounds.origin.y.0 + left_top)),
                                point(
                                    px(bounds.origin.x.0 + control_offset),
                                    px(bounds.origin.y.0 + left_top),
                                ),
                                point(
                                    px(bounds.origin.x.0 + gutter_width - control_offset),
                                    px(bounds.origin.y.0 + crushed_y),
                                ),
                                point(
                                    px(bounds.origin.x.0 + gutter_width),
                                    px(bounds.origin.y.0 + crushed_y),
                                ),
                                t,
                            );
                            if i == 0 {
                                builder.move_to(top_point);
                            } else {
                                builder.line_to(top_point);
                            }
                        }

                        for i in (0..=segments).rev() {
                            let t = i as f32 / segments as f32;
                            let bottom_point = cubic_bezier(
                                point(px(bounds.origin.x.0), px(bounds.origin.y.0 + left_bottom)),
                                point(
                                    px(bounds.origin.x.0 + control_offset),
                                    px(bounds.origin.y.0 + left_bottom),
                                ),
                                point(
                                    px(bounds.origin.x.0 + gutter_width - control_offset),
                                    px(bounds.origin.y.0 + crushed_y + 2.0),
                                ),
                                point(
                                    px(bounds.origin.x.0 + gutter_width),
                                    px(bounds.origin.y.0 + crushed_y + 2.0),
                                ),
                                t,
                            );
                            builder.line_to(bottom_point);
                        }

                        if let Ok(path) = builder.build() {
                            let background: Background = color.into();
                            window.paint_path(path, background);
                        }

                        cumulative_left_y += imara_block.left_range.len() as f32 * line_height;
                        cumulative_right_y += 2.0;
                        left_line_idx += imara_block.left_range.len();
                        continue;
                    }

                    if imara_block.left_range.is_empty() || imara_block.right_range.is_empty() {
                        continue;
                    }

                    if imara_block.left_range.start >= left_lines.len() {
                        continue;
                    }

                    let left_top = cumulative_left_y + left_scroll.y.0;
                    let left_bottom = (cumulative_left_y
                        + imara_block.left_range.len() as f32 * line_height)
                        + left_scroll.y.0;
                    let right_top = cumulative_right_y + right_scroll.y.0;
                    let right_bottom = (cumulative_right_y
                        + imara_block.right_range.len() as f32 * line_height)
                        + right_scroll.y.0;

                    let line_type = &left_lines[imara_block.left_range.start].line_type;
                    let color = match line_type {
                        crate::models::line::LineType::Addition => palette.addition_ribbon,
                        crate::models::line::LineType::Deletion => palette.deletion_ribbon,
                        crate::models::line::LineType::Modification => palette.modification_ribbon,
                        _ => continue,
                    };

                    let mut builder = PathBuilder::fill();
                    let segments = 32;

                    for i in 0..=segments {
                        let t = i as f32 / segments as f32;
                        let top_point = cubic_bezier(
                            point(px(bounds.origin.x.0), px(bounds.origin.y.0 + left_top)),
                            point(
                                px(bounds.origin.x.0 + control_offset),
                                px(bounds.origin.y.0 + left_top),
                            ),
                            point(
                                px(bounds.origin.x.0 + gutter_width - control_offset),
                                px(bounds.origin.y.0 + right_top),
                            ),
                            point(
                                px(bounds.origin.x.0 + gutter_width),
                                px(bounds.origin.y.0 + right_top),
                            ),
                            t,
                        );
                        if i == 0 {
                            builder.move_to(top_point);
                        } else {
                            builder.line_to(top_point);
                        }
                    }

                    for i in (0..=segments).rev() {
                        let t = i as f32 / segments as f32;
                        let bottom_point = cubic_bezier(
                            point(px(bounds.origin.x.0), px(bounds.origin.y.0 + left_bottom)),
                            point(
                                px(bounds.origin.x.0 + control_offset),
                                px(bounds.origin.y.0 + left_bottom),
                            ),
                            point(
                                px(bounds.origin.x.0 + gutter_width - control_offset),
                                px(bounds.origin.y.0 + right_bottom),
                            ),
                            point(
                                px(bounds.origin.x.0 + gutter_width),
                                px(bounds.origin.y.0 + right_bottom),
                            ),
                            t,
                        );
                        builder.line_to(bottom_point);
                    }

                    if let Ok(path) = builder.build() {
                        let background: Background = color.into();
                        window.paint_path(path, background);
                    }

                    cumulative_left_y += imara_block.left_range.len() as f32 * line_height;
                    cumulative_right_y += imara_block.right_range.len() as f32 * line_height;
                    left_line_idx += imara_block.left_range.len();
                    right_line_idx += imara_block.right_range.len();
                }
            },
        )
        .size_full()
    }

    fn render_line(
        &self,
        line: &crate::models::line::DisplayLine,
        line_number: usize,
        palette: &DiffPalette,
        is_active: bool,
    ) -> impl IntoElement {
        let line_bg = if is_active {
            palette.selection
        } else {
            match line.line_type {
                LineType::Addition => palette.addition_background,
                LineType::Deletion => palette.deletion_background,
                LineType::Modification => palette.modification_background,
                LineType::Context => palette.background,
            }
        };

        let text_color = UiColor::Custom(palette.text_primary);
        let line_num_color = UiColor::Custom(palette.line_numbers);

        h_flex()
            .h(px(CONTENT_FONT_SIZE * 1.2))
            .gap_0()
            .bg(line_bg)
            .child(
                div()
                    .w(px(GUTTER_WIDTH))
                    .flex_shrink_0()
                    .flex()
                    .justify_end()
                    .px_2()
                    .child(Label::new(format!("{}", line_number)).color(line_num_color)),
            )
            .child(
                div()
                    .flex_1()
                    .px_2()
                    .child(Label::new(line.content.clone()).color(text_color)),
            )
    }

    fn render_crushed_block(&self, palette: &DiffPalette, is_addition: bool) -> impl IntoElement {
        let color = if is_addition {
            palette.addition_background
        } else {
            palette.deletion_background
        };

        div().h(px(2.0)).w_full().bg(color)
    }

    fn render_column(
        &self,
        lines: &[crate::models::line::DisplayLine],
        scroll_handle: &gpui::ScrollHandle,
        scroll_id: impl Into<gpui::ElementId>,
        palette: &DiffPalette,
        active_block: Option<&crate::models::diff::ChangeBlock>,
        is_left: bool,
    ) -> impl IntoElement {
        let bg_color = palette.background;
        let state = self.app.state_manager.get_current_state();
        let imara_analysis = state.imara_analysis.clone();
        let mut elements: Vec<gpui::AnyElement> = Vec::new();
        let mut line_idx = 0;

        let line_height = CONTENT_FONT_SIZE * 1.2;
        let scroll_offset = scroll_handle.offset();
        let max_lines = state.left_lines.len().max(state.right_lines.len());
        let visible_line_count = 100; // approximate, adjust as needed

        let scroll_y = scroll_offset.y.0;
        let first_visible_line =
            ((scroll_y / line_height).floor().max(0.0) as usize).saturating_sub(5);
        let last_visible_line = (first_visible_line + visible_line_count).min(max_lines);

        if first_visible_line > 0 {
            let spacer_height = first_visible_line as f32 * line_height;
            elements.push(div().h(px(spacer_height)).w_full().into_any_element());
        }

        for imara_block in &imara_analysis.blocks {
            let target_line_end = if is_left {
                if imara_block.is_pure_insertion() {
                    imara_block.right_range.start.saturating_sub(
                        imara_analysis
                            .blocks
                            .iter()
                            .filter(|b| {
                                b.right_range.end <= imara_block.right_range.start
                                    && b.is_pure_insertion()
                            })
                            .map(|b| b.right_range.len())
                            .sum::<usize>(),
                    )
                } else {
                    imara_block.left_range.end
                }
            } else {
                if imara_block.is_pure_deletion() {
                    imara_block.left_range.start.saturating_sub(
                        imara_analysis
                            .blocks
                            .iter()
                            .filter(|b| {
                                b.left_range.end <= imara_block.left_range.start
                                    && b.is_pure_deletion()
                            })
                            .map(|b| b.left_range.len())
                            .sum::<usize>(),
                    )
                } else {
                    imara_block.right_range.end
                }
            };

            while line_idx < target_line_end.min(lines.len()) {
                if line_idx >= first_visible_line && line_idx < last_visible_line {
                    let line = &lines[line_idx];
                    let is_active = active_block
                        .map(|block| line_idx >= block.start_line && line_idx <= block.end_line)
                        .unwrap_or(false);
                    elements.push(
                        self.render_line(line, line_idx + 1, palette, is_active)
                            .into_any_element(),
                    );
                }
                line_idx += 1;
            }

            if (is_left && imara_block.is_pure_insertion() && !imara_block.right_range.is_empty())
                || (!is_left
                    && imara_block.is_pure_deletion()
                    && !imara_block.left_range.is_empty())
            {
                if line_idx >= first_visible_line && line_idx < last_visible_line {
                    let is_addition = is_left && imara_block.is_pure_insertion();
                    elements.push(
                        self.render_crushed_block(palette, is_addition)
                            .into_any_element(),
                    );
                }
            }
        }

        while line_idx < lines.len() {
            if line_idx >= first_visible_line && line_idx < last_visible_line {
                let line = &lines[line_idx];
                let is_active = active_block
                    .map(|block| line_idx >= block.start_line && line_idx <= block.end_line)
                    .unwrap_or(false);
                elements.push(
                    self.render_line(line, line_idx + 1, palette, is_active)
                        .into_any_element(),
                );
            }
            line_idx += 1;
        }

        if last_visible_line < max_lines {
            let spacer_height = (max_lines - last_visible_line) as f32 * line_height;
            elements.push(div().h(px(spacer_height)).w_full().into_any_element());
        }

        div()
            .id(scroll_id.into())
            .flex_1()
            .min_h_0()
            .w_full()
            .bg(bg_color)
            .overflow_y_scroll()
            .track_scroll(scroll_handle)
            .child(div().w_full().children(elements))
    }
}

impl Render for DiffViewerElement {
    fn render(&mut self, _window: &mut Window, cx: &mut gpui::Context<Self>) -> impl IntoElement {
        let state = self.app.state_manager.get_current_state();
        let palette = &self.app.palette;
        let bg_color = palette.background;

        let active_block = state
            .change_blocks
            .get(state.navigation_state.current_block_index)
            .cloned();

        v_flex()
            .flex_1()
            .min_h_0()
            .w_full()
            .bg(bg_color)
            .track_focus(&self.focus_handle)
            .key_context("DiffViewer")
            .on_action(cx.listener(Self::scroll_up))
            .on_action(cx.listener(Self::scroll_down))
            .on_action(cx.listener(Self::page_up))
            .on_action(cx.listener(Self::page_down))
            .on_action(cx.listener(Self::next_diff))
            .on_action(cx.listener(Self::previous_diff))
            .on_action(cx.listener(Self::next_file))
            .on_action(cx.listener(Self::previous_file))
            .on_action(cx.listener(Self::load_demo))
            .child(
                h_flex()
                    .flex_1()
                    .min_h_0()
                    .overflow_hidden()
                    .gap_0()
                    .child(
                        v_flex()
                            .flex_1()
                            .h_full()
                            .overflow_hidden()
                            .child(self.render_column(
                                &state.left_lines,
                                &self.scroll_handle,
                                "left-scroll",
                                palette,
                                active_block.as_ref(),
                                true,
                            )),
                    )
                    .child(
                        div()
                            .w(px(40.0))
                            .h_full()
                            .flex_shrink_0()
                            .bg(hsla(0.0, 0.0, 0.12, 1.0))
                            .child(self.render_connectors()),
                    )
                    .child(
                        v_flex()
                            .flex_1()
                            .h_full()
                            .overflow_hidden()
                            .child(self.render_column(
                                &state.right_lines,
                                &self.scroll_handle,
                                "right-scroll",
                                palette,
                                active_block.as_ref(),
                                false,
                            )),
                    ),
            )
    }
}
