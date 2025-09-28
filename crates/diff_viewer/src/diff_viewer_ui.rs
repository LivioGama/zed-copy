//! JetBrains-style Diff Viewer implemented with Zed's gpui primitives.

use crate::perfect_connector_renderer::PerfectConnectorRenderer;
use crate::perfect_diff_engine::{
    DiffView, ImaraDiffAnalysis, Line, LineState, NewChangeBlock, compute_imara_diff_default,
    generate_perfect_diff_view,
};
use gpui::prelude::*;
use gpui::{
    App, Context, EventEmitter, FocusHandle, Focusable, Hsla, InteractiveElement, IntoElement,
    KeyBinding, ParentElement, Render, ScrollHandle, StatefulInteractiveElement, Styled, Window,
    canvas, div, hsla, px,
};
use log::{info, warn};
use std::path::PathBuf;
use ui::prelude::*;
use ui::{
    ActiveTheme, Button, Clickable, Color, Disableable, Icon, IconName, IconSize, Label,
    LabelCommon, LabelSize,
};

/// Actions for the diff viewer
#[derive(Clone, Debug, PartialEq)]
pub enum DiffViewerAction {
    Close,
    NextChange,
    PreviousChange,
    ToggleWhitespace,
}

// GPUI Actions for diff viewer commands
gpui::actions!(
    diff_viewer,
    [
        OpenDiffViewer,
        CloseDiffViewer,
        NextChange,
        PreviousChange,
        ToggleWhitespace,
        NextFile,
        PreviousFile,
        DefaultDemo,
    ]
);

/// Diff mode for navigation
#[derive(Debug, Clone, PartialEq)]
pub enum DiffMode {
    ProjectFile(usize),
    DefaultDemo,
}

/// Toolbar state
#[derive(Debug, Clone)]
pub struct ToolbarState {
    pub project_files: Vec<PathBuf>,
    pub current_mode: DiffMode,
}

impl ToolbarState {
    pub fn new(project_files: Vec<PathBuf>) -> Self {
        let current_mode = if project_files.is_empty() {
            DiffMode::DefaultDemo
        } else {
            DiffMode::ProjectFile(0)
        };

        Self {
            project_files,
            current_mode,
        }
    }

    pub fn current_index(&self) -> Option<usize> {
        match self.current_mode {
            DiffMode::ProjectFile(index) => Some(index),
            DiffMode::DefaultDemo => None,
        }
    }

    pub fn can_go_previous(&self) -> bool {
        match self.current_mode {
            DiffMode::ProjectFile(index) => index > 0,
            DiffMode::DefaultDemo => !self.project_files.is_empty(),
        }
    }

    pub fn can_go_next(&self) -> bool {
        match self.current_mode {
            DiffMode::ProjectFile(index) => index < self.project_files.len().saturating_sub(1),
            DiffMode::DefaultDemo => !self.project_files.is_empty(),
        }
    }

    pub fn go_previous(&mut self) {
        match self.current_mode {
            DiffMode::ProjectFile(ref mut index) => {
                if *index > 0 {
                    *index -= 1;
                }
            }
            DiffMode::DefaultDemo => {
                if !self.project_files.is_empty() {
                    let last_index = self.project_files.len() - 1;
                    self.current_mode = DiffMode::ProjectFile(last_index);
                }
            }
        }
    }

    pub fn go_next(&mut self) {
        match self.current_mode {
            DiffMode::ProjectFile(ref mut index) => {
                if *index < self.project_files.len().saturating_sub(1) {
                    *index += 1;
                }
            }
            DiffMode::DefaultDemo => {
                if !self.project_files.is_empty() {
                    self.current_mode = DiffMode::ProjectFile(0);
                }
            }
        }
    }

    pub fn go_to_default(&mut self) {
        self.current_mode = DiffMode::DefaultDemo;
    }

    pub fn current_file(&self) -> Option<&PathBuf> {
        match self.current_mode {
            DiffMode::ProjectFile(index) => self.project_files.get(index),
            DiffMode::DefaultDemo => None,
        }
    }
}

const CONNECTOR_COLUMN_WIDTH: f32 = 96.0;
const COLUMN_HEADER_HEIGHT: f32 = 36.0;
const TOOLBAR_HEIGHT: f32 = 48.0;
const HEADER_HEIGHT: f32 = 52.0;
const LINE_HEIGHT: f32 = 20.0;

#[derive(Clone, Copy)]
enum PaneSide {
    Left,
    Right,
}

/// The main diff viewer UI component
pub struct DiffViewer {
    focus_handle: FocusHandle,
    left_path: Option<PathBuf>,
    right_path: Option<PathBuf>,
    diff_view: Option<DiffView>,
    change_blocks: Vec<NewChangeBlock>,
    imara_analysis: ImaraDiffAnalysis,
    toolbar_state: ToolbarState,
    scroll_handle: ScrollHandle,
}

impl DiffViewer {
    /// Create a new diff viewer
    pub fn new(
        left_path: Option<PathBuf>,
        right_path: Option<PathBuf>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let focus_handle = cx.focus_handle();

        Self {
            focus_handle,
            left_path,
            right_path,
            diff_view: None,
            change_blocks: Vec::new(),
            imara_analysis: ImaraDiffAnalysis { blocks: Vec::new() },
            toolbar_state: ToolbarState::new(Vec::new()),
            scroll_handle: ScrollHandle::new(),
        }
    }

    /// Initialize the diff viewer after creation
    pub fn initialize(&mut self, cx: &mut Context<Self>) {
        self.discover_project_files();
        if self.left_path.is_some() && self.right_path.is_some() {
            self.load_diff(cx);
        } else {
            self.load_default_demo_diff(cx);
        }
    }

    /// Load diff data between the two files
    pub fn load_diff(&mut self, cx: &mut Context<Self>) {
        if let (Some(left_path), Some(right_path)) = (&self.left_path, &self.right_path) {
            let left_content = std::fs::read_to_string(left_path).unwrap_or_default();
            let right_content = std::fs::read_to_string(right_path).unwrap_or_default();

            self.imara_analysis = compute_imara_diff_default(&left_content, &right_content);
            let diff_view = generate_perfect_diff_view(&left_content, &right_content);
            self.change_blocks = diff_view.change_blocks.clone();
            self.diff_view = Some(diff_view);
            self.scroll_handle.set_offset(gpui::point(px(0.0), px(0.0)));
            cx.notify();
        } else {
            warn!("diff_viewer: attempted to load diff without file paths");
        }
    }

    fn line_background(state: &LineState) -> Hsla {
        match state {
            LineState::Added => hsla(120.0 / 360.0, 0.38, 0.25, 0.45),
            LineState::Deleted => hsla(0.0 / 360.0, 0.45, 0.28, 0.45),
            LineState::Modified { .. } => hsla(220.0 / 360.0, 0.40, 0.30, 0.45),
            LineState::Placeholder => hsla(220.0 / 360.0, 0.18, 0.16, 0.35),
            LineState::Unchanged => hsla(220.0 / 360.0, 0.16, 0.14, 0.32),
        }
    }

    fn indicator_color(state: &LineState, side: PaneSide) -> Option<Hsla> {
        match (side, state) {
            (PaneSide::Left, LineState::Deleted) => Some(hsla(0.0 / 360.0, 0.65, 0.40, 0.8)),
            (PaneSide::Left, LineState::Modified { .. }) => {
                Some(hsla(220.0 / 360.0, 0.55, 0.45, 0.8))
            }
            (PaneSide::Right, LineState::Added) => Some(hsla(120.0 / 360.0, 0.55, 0.40, 0.8)),
            (PaneSide::Right, LineState::Modified { .. }) => {
                Some(hsla(220.0 / 360.0, 0.55, 0.45, 0.8))
            }
            _ => None,
        }
    }

    fn line_text_color(state: &LineState) -> Color {
        match state {
            LineState::Added => Color::Custom(hsla(120.0 / 360.0, 0.25, 0.70, 1.0)),
            LineState::Deleted => Color::Custom(hsla(0.0 / 360.0, 0.35, 0.70, 1.0)),
            LineState::Modified { .. } => Color::Custom(hsla(220.0 / 360.0, 0.10, 0.85, 1.0)),
            LineState::Placeholder => Color::Muted,
            LineState::Unchanged => Color::Default,
        }
    }

    fn render_line_row(number: &str, line: &Line, side: PaneSide) -> gpui::AnyElement {
        let background = Self::line_background(&line.state);
        let text_color = Self::line_text_color(&line.state);
        let indicator_color =
            Self::indicator_color(&line.state, side).unwrap_or_else(|| hsla(0.0, 0.0, 0.0, 0.0));

        let content = if line.content.is_empty() {
            "".to_string()
        } else {
            line.content.clone()
        };

        h_flex()
            .h(px(LINE_HEIGHT))
            .w_full()
            .items_center()
            .bg(background)
            .children([
                div()
                    .w(px(3.0))
                    .h_full()
                    .bg(indicator_color)
                    .into_any_element(),
                h_flex()
                    .w(px(56.0))
                    .pr(px(8.0))
                    .justify_end()
                    .items_center()
                    .child(
                        Label::new(number.to_string())
                            .size(LabelSize::Small)
                            .color(Color::Muted),
                    )
                    .into_any_element(),
                h_flex()
                    .flex_1()
                    .items_center()
                    .child(Label::new(content).size(LabelSize::Small).color(text_color))
                    .into_any_element(),
            ])
            .into_any_element()
    }

    fn render_pane(
        &self,
        diff_view: &DiffView,
        side: PaneSide,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let (title, lines) = match side {
            PaneSide::Left => ("Original", &diff_view.left_pane),
            PaneSide::Right => ("Modified", &diff_view.right_pane),
        };

        let mut line_number: u32 = 1;
        let rows: Vec<_> = lines
            .iter()
            .map(|line| {
                let display_number = match line.state {
                    LineState::Placeholder => String::new(),
                    LineState::Added if matches!(side, PaneSide::Left) => String::new(),
                    LineState::Deleted if matches!(side, PaneSide::Right) => String::new(),
                    _ => {
                        let current = line_number;
                        line_number += 1;
                        current.to_string()
                    }
                };
                Self::render_line_row(&display_number, line, side)
            })
            .collect();

        v_flex()
            .flex_1()
            .children([
                self.render_column_header(title, cx).into_any_element(),
                v_flex()
                    .overflow_y_scroll()
                    .track_scroll(&self.scroll_handle)
                    .child(v_flex().children(rows))
                    .into_any_element(),
            ])
            .into_any_element()
    }

    fn render_toolbar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = cx.theme().colors();
        let state = &self.toolbar_state;
        let can_go_previous = state.can_go_previous();
        let can_go_next = state.can_go_next();

        let file_label = match state.current_mode {
            DiffMode::ProjectFile(index) => state
                .project_files
                .get(index)
                .map(|path| {
                    format!(
                        "{} ({}/{})",
                        path.display(),
                        index + 1,
                        state.project_files.len()
                    )
                })
                .unwrap_or_else(|| "No diff selected".to_string()),
            DiffMode::DefaultDemo => "Demo diff".to_string(),
        };

        let left_cluster = div()
            .flex()
            .items_center()
            .child(
                Icon::new(IconName::FileDiff)
                    .size(IconSize::Medium)
                    .color(Color::Accent),
            )
            .child(
                div()
                    .ml_2()
                    .flex()
                    .flex_col()
                    .child(
                        Label::new("JetBrains Diff Viewer")
                            .color(Color::Default)
                            .size(LabelSize::Default),
                    )
                    .child(
                        Label::new(file_label)
                            .size(LabelSize::Small)
                            .color(Color::Muted),
                    ),
            );

        let controls = div().flex().items_center().children([
            div()
                .child(
                    Button::new("previous", "◀ Previous")
                        .disabled(!can_go_previous)
                        .on_click(cx.listener(|this, _, _window, cx| {
                            this.handle_toolbar_action("previous_file", cx);
                        })),
                )
                .into_any_element(),
            div()
                .ml_2()
                .child(
                    Button::new("next", "Next ▶")
                        .disabled(!can_go_next)
                        .on_click(cx.listener(|this, _, _window, cx| {
                            this.handle_toolbar_action("next_file", cx);
                        })),
                )
                .into_any_element(),
            div()
                .ml_4()
                .child(
                    Button::new("demo", "Demo").on_click(cx.listener(|this, _, _window, cx| {
                        this.handle_toolbar_action("default_demo", cx);
                    })),
                )
                .into_any_element(),
        ]);

        div()
            .flex()
            .items_center()
            .justify_between()
            .h(px(TOOLBAR_HEIGHT))
            .px_4()
            .bg(colors.title_bar_background)
            .border_b_1()
            .border_color(colors.border)
            .child(left_cluster)
            .child(controls)
    }

    fn render_header(&self) -> impl IntoElement {
        let title = Label::new("JetBrains Diff Viewer")
            .color(Color::Default)
            .size(LabelSize::Default);
        let instructions = Label::new("⌘⇧←/→ navigate • D demo • ↑/↓ scroll diffs")
            .color(Color::Muted)
            .size(LabelSize::Small);

        div()
            .h(px(HEADER_HEIGHT))
            .px_4()
            .flex()
            .items_center()
            .justify_between()
            .bg(hsla(224.0 / 360.0, 0.34, 0.13, 1.0))
            .border_b_1()
            .border_color(hsla(224.0 / 360.0, 0.3, 0.23, 1.0))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(title)
                    .child(instructions),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(
                        Label::new("Ctrl+Shift+G stage")
                            .size(LabelSize::Small)
                            .color(Color::Muted),
                    )
                    .child(
                        Label::new("Ctrl+Shift+H revert")
                            .size(LabelSize::Small)
                            .color(Color::Muted),
                    ),
            )
    }

    fn render_column_header(&self, title: &str, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = cx.theme().colors();
        div()
            .h(px(COLUMN_HEADER_HEIGHT))
            .px_3()
            .flex()
            .items_center()
            .bg(colors.background)
            .border_b_1()
            .border_color(colors.border)
            .child(
                Label::new(title.to_string())
                    .size(LabelSize::Small)
                    .color(Color::Muted),
            )
    }

    fn render_connector_header(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let colors = cx.theme().colors();
        div()
            .h(px(COLUMN_HEADER_HEIGHT))
            .flex()
            .items_center()
            .justify_center()
            .bg(colors.background)
            .border_b_1()
            .border_color(colors.border)
            .child(
                Label::new("Changes")
                    .size(LabelSize::Small)
                    .color(Color::Muted),
            )
    }

    fn render_connector_gutter(
        &self,
        left_scroll_lines: f32,
        right_scroll_lines: f32,
        left_line_height: f32,
        right_line_height: f32,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let colors = cx.theme().colors();
        let mut connector_renderer =
            PerfectConnectorRenderer::new(1.0, CONNECTOR_COLUMN_WIDTH.max(1.0));
        connector_renderer.generate_connectors_from_imara(
            &self.imara_analysis,
            left_line_height,
            0.0,
            CONNECTOR_COLUMN_WIDTH,
        );

        let mut connectors = connector_renderer.connectors.clone();
        for connector in &mut connectors {
            connector.start_point.y -= left_scroll_lines * left_line_height;
            connector.end_point.y -= right_scroll_lines * right_line_height;
        }

        div()
            .w(px(CONNECTOR_COLUMN_WIDTH))
            .flex()
            .flex_col()
            .bg(colors.editor_background)
            .border_x_1()
            .border_color(colors.border)
            .children([
                self.render_connector_header(cx).into_any_element(),
                canvas(
                    move |_bounds, _window, _cx| {},
                    move |bounds, _data, window, _cx| {
                        let background = hsla(224.0 / 360.0, 0.20, 0.18, 1.0);
                        window.paint_quad(gpui::fill(bounds, background));
                        for connector in &connectors {
                            connector.render_connector_band(bounds, window);
                        }
                    },
                )
                .flex_grow()
                .w_full()
                .into_any_element(),
            ])
    }

    fn render_empty_state(&self) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .flex_grow()
            .child(Label::new("No diff loaded").color(Color::Muted))
    }

    fn discover_project_files(&mut self) {
        let git_ops = crate::git::GitOps::with_current_dir();
        let changed_files = git_ops.get_changed_files(None, None);
        let project_files: Vec<PathBuf> = changed_files.into_iter().map(PathBuf::from).collect();
        self.toolbar_state = ToolbarState::new(project_files);
    }

    /// Handle toolbar navigation actions
    pub fn handle_toolbar_action(&mut self, action: &str, cx: &mut Context<Self>) {
        match action {
            "next_file" => {
                self.toolbar_state.go_next();
                if let Some(file_path) = self.toolbar_state.current_file().cloned() {
                    self.load_file_diff(&file_path, cx);
                }
            }
            "previous_file" => {
                self.toolbar_state.go_previous();
                if let Some(file_path) = self.toolbar_state.current_file().cloned() {
                    self.load_file_diff(&file_path, cx);
                }
            }
            "default_demo" => {
                self.toolbar_state.go_to_default();
                self.load_default_demo_diff(cx);
            }
            _ => {}
        }
    }

    /// Load diff for a specific file
    pub fn load_file_diff(&mut self, file_path: &PathBuf, cx: &mut Context<Self>) {
        info!("diff_viewer: loading diff for file {file_path:?}");

        let git_ops = crate::git::GitOps::with_current_dir();
        let current_content = std::fs::read_to_string(file_path).unwrap_or_default();
        let original_content = match git_ops.show_file("HEAD", &file_path.to_string_lossy()) {
            crate::git::GitResult {
                success: true,
                stdout,
                ..
            } => stdout,
            _ => current_content.clone(),
        };

        self.left_path = Some(file_path.clone());
        self.right_path = Some(file_path.clone());

        self.imara_analysis = compute_imara_diff_default(&original_content, &current_content);
        let diff_view = generate_perfect_diff_view(&original_content, &current_content);
        self.change_blocks = diff_view.change_blocks.clone();
        self.diff_view = Some(diff_view);
        self.scroll_handle.set_offset(gpui::point(px(0.0), px(0.0)));
        cx.notify();
    }

    /// Load the default demo diff
    pub fn load_default_demo_diff(&mut self, cx: &mut Context<Self>) {
        let original_content = "import React from 'react';\nimport { BrowserRouter as Router, Routes, Route } from 'react-router-dom';\nimport { ThemeProvider } from './theme';\nimport { AuthProvider } from './auth';\nimport { NotificationProvider } from './notifications';\n\nfunction AppProviders({ children }) {\n  return (\n    <ThemeProvider>\n      <AuthProvider>\n        <NotificationProvider>\n          <Router>\n            {children}\n          </Router>\n        </NotificationProvider>\n      </AuthProvider>\n    </ThemeProvider>\n  );\n}\n\nexport default AppProviders;\n".to_string();

        let current_content = "import React from 'react';\nimport { BrowserRouter as Router, Routes, Route } from 'react-router-dom';\nimport { ThemeProvider } from './theme';\nimport { AuthProvider } from './auth';\n\nfunction AppProviders({ children }) {\n  return (\n    <ThemeProvider>\n      <AuthProvider>\n          <Router>\n            {children}\n          </Router>\n      </AuthProvider>\n    </ThemeProvider>\n  );\n}\n\nexport default AppProviders;\n".to_string();

        self.imara_analysis = compute_imara_diff_default(&original_content, &current_content);
        let diff_view = generate_perfect_diff_view(&original_content, &current_content);
        self.change_blocks = diff_view.change_blocks.clone();
        self.diff_view = Some(diff_view);
        self.scroll_handle.set_offset(gpui::point(px(0.0), px(0.0)));
        cx.notify();
    }
}

impl Render for DiffViewer {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let diff_view = match &self.diff_view {
            Some(view) => view,
            None => {
                return div()
                    .flex()
                    .flex_col()
                    .size_full()
                    .child(self.render_header())
                    .child(self.render_toolbar(cx))
                    .child(self.render_empty_state());
            }
        };

        let scroll_offset_lines = self.scroll_handle.offset().y.0.max(0.0) / LINE_HEIGHT.max(1.0);

        let left_panel = self
            .render_pane(diff_view, PaneSide::Left, cx)
            .into_any_element();
        let right_panel = self
            .render_pane(diff_view, PaneSide::Right, cx)
            .into_any_element();
        let connector_gutter = self
            .render_connector_gutter(
                scroll_offset_lines,
                scroll_offset_lines,
                LINE_HEIGHT,
                LINE_HEIGHT,
                cx,
            )
            .into_any_element();

        let header = self.render_header().into_any_element();
        let toolbar = self.render_toolbar(cx).into_any_element();
        let colors = cx.theme().colors();

        div()
            .key_context("DiffViewer")
            .flex()
            .flex_col()
            .size_full()
            .bg(colors.editor_background)
            .child(header)
            .child(toolbar)
            .child(
                div()
                    .flex()
                    .flex_row()
                    .flex_grow()
                    .bg(colors.editor_background)
                    .children([left_panel, connector_gutter, right_panel]),
            )
    }
}

impl Focusable for DiffViewer {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl EventEmitter<DiffViewerAction> for DiffViewer {}

/// Initialize the diff viewer feature in Zed
pub fn init(cx: &mut gpui::App) {
    cx.bind_keys([
        KeyBinding::new("cmd+shift+d", OpenDiffViewer, Some("DiffViewer")),
        KeyBinding::new("escape", CloseDiffViewer, Some("DiffViewer")),
        KeyBinding::new("j", NextChange, Some("DiffViewer")),
        KeyBinding::new("k", PreviousChange, Some("DiffViewer")),
        KeyBinding::new("w", ToggleWhitespace, Some("DiffViewer")),
        KeyBinding::new("ctrl-shift-left", PreviousFile, Some("DiffViewer")),
        KeyBinding::new("ctrl-shift-right", NextFile, Some("DiffViewer")),
        KeyBinding::new("d", DefaultDemo, Some("DiffViewer")),
    ]);
}
