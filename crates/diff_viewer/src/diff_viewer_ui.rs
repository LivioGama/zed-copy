//! Diff Viewer UI using Zed's TextEditor components
//! Provides a JetBrains-style diff viewer with side-by-side editors and curved connectors

use editor::Editor;
use gpui::{
    App, AppContext, Bounds, Context, Corners, Entity, EventEmitter, FocusHandle, Focusable, Hsla,
    InteractiveElement, IntoElement, KeyBinding, PaintQuad, ParentElement, PathBuilder, Pixels,
    Point, Render, ScrollHandle, Size, Styled, Window, canvas, div, point, px,
};
use language::Buffer;
use multi_buffer::{Anchor, MultiBuffer, MultiBufferSnapshot};
use std::ops::Range;
use std::path::PathBuf;
use text::Point as TextPoint;
use theme::{ActiveTheme, Theme};
use ui::{Button, Clickable, Disableable, Icon, IconName, Label, SharedString};

use crate::highlight_renderer_gpui::HighlightRenderer;
use crate::layout_manager_gpui::LayoutManager;

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

/// Type for diff background highlights
enum DiffHighlight {}

/// Diff mode for navigation
#[derive(Debug, Clone, PartialEq)]
pub enum DiffMode {
    ProjectFile(usize), // index into file list
    DefaultDemo,        // legacy hardcoded demo diff
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

/// The main diff viewer UI component
pub struct DiffViewer {
    focus_handle: FocusHandle,
    left_editor: Entity<Editor>,
    right_editor: Entity<Editor>,
    left_path: Option<PathBuf>,
    right_path: Option<PathBuf>,
    imara_analysis: crate::perfect_diff_engine::ImaraDiffAnalysis,
    change_blocks: Vec<crate::perfect_diff_engine::NewChangeBlock>,
    connector_renderer: crate::perfect_connector_renderer::PerfectConnectorRenderer,
    highlight_renderer: crate::highlight_renderer_gpui::HighlightRenderer,
    layout_manager: crate::layout_manager_gpui::LayoutManager,
    scroll_handle: ScrollHandle,
    theme: std::sync::Arc<Theme>,
    toolbar_state: ToolbarState,
}

impl DiffViewer {
    /// Create a new diff viewer
    pub fn new(
        left_path: Option<PathBuf>,
        right_path: Option<PathBuf>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let focus_handle = cx.focus_handle();

        // Create text buffers for each side
        let left_buffer = cx.new(|cx| {
            let buffer = if let Some(path) = &left_path {
                // For demo, create local buffer with file content
                let content = std::fs::read_to_string(path).unwrap_or_default();
                cx.new(|cx| {
                    let mut buffer = Buffer::local(&content, cx);
                    buffer.set_language(None, cx);
                    buffer
                })
            } else {
                cx.new(|cx| {
                    let mut buffer = Buffer::local("", cx);
                    buffer.set_language(None, cx);
                    buffer
                })
            };

            MultiBuffer::singleton(buffer, cx)
        });

        let right_buffer = cx.new(|cx| {
            let buffer = if let Some(path) = &right_path {
                // For demo, create local buffer with file content
                let content = std::fs::read_to_string(path).unwrap_or_default();
                cx.new(|cx| Buffer::local(&content, cx))
            } else {
                cx.new(|cx| Buffer::local("", cx))
            };

            MultiBuffer::singleton(buffer, cx)
        });

        // Create editors
        let left_editor = cx.new(|cx| {
            Editor::new(
                editor::EditorMode::Full {
                    scale_ui_elements_with_buffer_font_size: true,
                    show_active_line_background: false,
                    sized_by_content: false,
                },
                left_buffer,
                None, // No project
                window,
                cx,
            )
        });

        let right_editor = cx.new(|cx| {
            Editor::new(
                editor::EditorMode::Full {
                    scale_ui_elements_with_buffer_font_size: true,
                    show_active_line_background: false,
                    sized_by_content: false,
                },
                right_buffer,
                None, // No project
                window,
                cx,
            )
        });

        // Hide line numbers to make the gutter narrower
        left_editor.update(cx, |editor, cx| {
            editor.set_show_line_numbers(false, cx);
        });
        right_editor.update(cx, |editor, cx| {
            editor.set_show_line_numbers(false, cx);
        });

        // Initialize with empty analysis
        let imara_analysis = crate::perfect_diff_engine::ImaraDiffAnalysis { blocks: Vec::new() };
        let change_blocks = Vec::new();

        // Initialize GPUI renderers
        let jetbrains_theme = crate::theme::JetBrainsTheme::dark_theme();
        let highlight_renderer =
            crate::highlight_renderer_gpui::HighlightRenderer::new(jetbrains_theme.clone());
        let layout_manager =
            crate::layout_manager_gpui::LayoutManager::with_default_config(jetbrains_theme);

        Self {
            focus_handle,
            left_editor,
            right_editor,
            left_path,
            right_path,
            imara_analysis,
            change_blocks,
            connector_renderer: crate::perfect_connector_renderer::PerfectConnectorRenderer::new(
                1.0, 45.0,
            ),
            highlight_renderer,
            layout_manager,
            scroll_handle: ScrollHandle::new(),
            theme: cx.theme().clone(),
            toolbar_state: ToolbarState::new(Vec::new()), // Initialize with empty, discover later
        }
    }

    /// Initialize the diff viewer after creation
    pub fn initialize(&mut self, cx: &mut Context<Self>) {
        self.discover_project_files();
        // Load initial diff if paths are provided
        if self.left_path.is_some() && self.right_path.is_some() {
            self.load_diff(cx);
        } else {
            // Load demo diff by default
            self.load_default_demo_diff(cx);
        }
    }

    /// Load diff data between the two files
    pub fn load_diff(&mut self, cx: &mut Context<Self>) {
        if let (Some(left_path), Some(right_path)) = (&self.left_path, &self.right_path) {
            // Read file contents
            let left_content = std::fs::read_to_string(left_path).unwrap_or_default();
            let right_content = std::fs::read_to_string(right_path).unwrap_or_default();

            println!("🔍 Loading diff between files:");
            println!(
                "  Left: {} ({} chars)",
                left_path.display(),
                left_content.len()
            );
            println!(
                "  Right: {} ({} chars)",
                right_path.display(),
                right_content.len()
            );

            // Compute Imara diff analysis
            self.imara_analysis = crate::perfect_diff_engine::compute_imara_diff_default(
                &left_content,
                &right_content,
            );

            println!(
                "📊 Imara analysis found {} blocks",
                self.imara_analysis.blocks.len()
            );
            for (i, block) in self.imara_analysis.blocks.iter().enumerate() {
                println!(
                    "  Block {}: {:?} - Left: {:?}, Right: {:?}",
                    i, block.operation, block.left_range, block.right_range
                );
            }

            // Create change blocks from imara analysis (matching reference project)
            self.change_blocks = self.create_change_blocks_from_imara();
            println!("📋 Created {} change blocks", self.change_blocks.len());

            // Update buffers with original file content
            self.update_buffers(cx);

            // Apply diff highlights based on change blocks
            self.apply_diff_highlights(cx); // Temporarily re-enabled to debug canvas overlays

            println!("✅ Diff loading complete");
        } else {
            println!("❌ No file paths provided for diff");
        }
    }

    /// Update the editor buffers with original file content
    fn update_buffers(&mut self, cx: &mut Context<Self>) {
        if let Some(left_path) = &self.left_path {
            if let Ok(content) = std::fs::read_to_string(left_path) {
                self.left_editor.update(cx, |editor, cx| {
                    editor.buffer().update(cx, |multi_buffer, cx| {
                        multi_buffer.edit([(0..multi_buffer.len(cx), content)], None, cx);
                    });
                });
            }
        }

        if let Some(right_path) = &self.right_path {
            if let Ok(content) = std::fs::read_to_string(right_path) {
                self.right_editor.update(cx, |editor, cx| {
                    editor.buffer().update(cx, |multi_buffer, cx| {
                        multi_buffer.edit([(0..multi_buffer.len(cx), content)], None, cx);
                    });
                });
            }
        }
    }

    /// Create change blocks from imara analysis (matching reference project logic)
    fn create_change_blocks_from_imara(&self) -> Vec<crate::perfect_diff_engine::NewChangeBlock> {
        let mut change_blocks = Vec::new();
        for imara_block in &self.imara_analysis.blocks {
            if !imara_block.is_change() {
                continue;
            }

            match imara_block.operation {
                crate::perfect_diff_engine::ImaraBlockOperation::Modify => {
                    change_blocks.push(crate::perfect_diff_engine::NewChangeBlock {
                        change_type: crate::perfect_diff_engine::ChangeType::Modification,
                        left_start_idx: imara_block.left_range.start,
                        left_len: imara_block.left_range.len(),
                        right_start_idx: imara_block.right_range.start,
                        right_len: imara_block.right_range.len(),
                    });
                }
                crate::perfect_diff_engine::ImaraBlockOperation::Delete => {
                    change_blocks.push(crate::perfect_diff_engine::NewChangeBlock {
                        change_type: crate::perfect_diff_engine::ChangeType::Deletion,
                        left_start_idx: imara_block.left_range.start,
                        left_len: imara_block.left_range.len(),
                        right_start_idx: 0,
                        right_len: 0,
                    });
                }
                crate::perfect_diff_engine::ImaraBlockOperation::Insert => {
                    change_blocks.push(crate::perfect_diff_engine::NewChangeBlock {
                        change_type: crate::perfect_diff_engine::ChangeType::Addition,
                        left_start_idx: 0,
                        left_len: 0,
                        right_start_idx: imara_block.right_range.start,
                        right_len: imara_block.right_range.len(),
                    });
                }
            }
        }
        change_blocks
    }

    /// Apply diff highlights to the editors based on change blocks
    fn apply_diff_highlights(&mut self, cx: &mut Context<Self>) {
        let left_blocks = self.imara_analysis.blocks.clone();
        let right_blocks = self.imara_analysis.blocks.clone();

        // Clear existing highlights
        self.left_editor.update(cx, |editor, cx| {
            editor.clear_row_highlights::<DiffHighlight>();
        });
        self.right_editor.update(cx, |editor, cx| {
            editor.clear_row_highlights::<DiffHighlight>();
        });

        // Apply highlights for left editor
        self.left_editor.update(cx, |editor, cx| {
            for block in &left_blocks {
                let (start_line, end_line, color) = match block.operation {
                    crate::perfect_diff_engine::ImaraBlockOperation::Delete => (
                        block.left_range.start,
                        block.left_range.end,
                        cx.theme().status().deleted_background,
                    ),
                    crate::perfect_diff_engine::ImaraBlockOperation::Modify => (
                        block.left_range.start,
                        block.left_range.end,
                        cx.theme().status().modified_background,
                    ),
                    crate::perfect_diff_engine::ImaraBlockOperation::Insert => continue,
                };

                let snapshot = editor.buffer().read(cx).snapshot(cx);
                let start_point = language::Point::new(start_line as u32, 0);
                let end_point = language::Point::new(end_line as u32, 0);
                let start_anchor = snapshot.anchor_before(start_point);
                let end_anchor = snapshot.anchor_before(end_point);

                editor.highlight_rows::<DiffHighlight>(
                    start_anchor..end_anchor,
                    color,
                    Default::default(),
                    cx,
                );
            }
        });

        // Apply highlights for right editor
        self.right_editor.update(cx, |editor, cx| {
            for block in &right_blocks {
                let (start_line, end_line, color) = match block.operation {
                    crate::perfect_diff_engine::ImaraBlockOperation::Insert => (
                        block.right_range.start,
                        block.right_range.end,
                        cx.theme().status().created_background,
                    ),
                    crate::perfect_diff_engine::ImaraBlockOperation::Modify => (
                        block.right_range.start,
                        block.right_range.end,
                        cx.theme().status().modified_background,
                    ),
                    crate::perfect_diff_engine::ImaraBlockOperation::Delete => continue,
                };

                let snapshot = editor.buffer().read(cx).snapshot(cx);
                let start_point = language::Point::new(start_line as u32, 0);
                let end_point = language::Point::new(end_line as u32, 0);
                let start_anchor = snapshot.anchor_before(start_point);
                let end_anchor = snapshot.anchor_before(end_point);

                editor.highlight_rows::<DiffHighlight>(
                    start_anchor..end_anchor,
                    color,
                    Default::default(),
                    cx,
                );
            }
        });
    }

    fn anchors_for_line_range(
        snapshot: &MultiBufferSnapshot,
        line_range: &Range<usize>,
    ) -> Option<Range<Anchor>> {
        if line_range.is_empty() {
            return None;
        }

        let max_point = snapshot.max_point();
        let row_count = max_point.row as usize + 1;

        if row_count == 0 || line_range.start >= row_count {
            return None;
        }

        let end_row = line_range.end.min(row_count);
        if line_range.start >= end_row {
            return None;
        }

        let start_anchor = snapshot.anchor_before(TextPoint::new(line_range.start as u32, 0));
        let end_anchor = if end_row == row_count {
            snapshot.anchor_after(max_point)
        } else {
            snapshot.anchor_before(TextPoint::new(end_row as u32, 0))
        };

        if start_anchor == end_anchor {
            None
        } else {
            Some(start_anchor..end_anchor)
        }
    }

    /// Render the connector gutter with complete connector system
    fn render_connector_gutter(
        &self,
        left_scroll_y: f32,
        right_scroll_y: f32,
        left_line_height: f32,
        right_line_height: f32,
    ) -> impl IntoElement {
        let colors = self.theme.colors();
        let gutter_width = self.connector_renderer.gutter_width;
        let blocks = self.imara_analysis.blocks.clone();
        let layout_manager = self.layout_manager.clone();
        // Estimate header height: padding (8px top/bottom) + border (1px) + label height (~16px) ≈ 33px
        let header_height = 33.0;

        div()
            .w(px(gutter_width))
            .h_full()
            .bg(colors.editor_background)
            .border_x_1()
            .border_color(colors.border)
            .child(canvas(
                move |_bounds, _window, _cx| {},
                move |bounds, _data, window, _cx| {
                    // Draw connectors using LayoutManager
                    Self::draw_connectors_with_layout_manager(
                        &blocks,
                        bounds,
                        window,
                        &layout_manager,
                        left_scroll_y,
                        right_scroll_y,
                        left_line_height,
                        right_line_height,
                        header_height,
                    );
                },
            ))
    }

    /// Draw connectors between panes using LayoutManager
    fn draw_connectors_with_layout_manager(
        blocks: &[crate::perfect_diff_engine::ImaraDiffBlock],
        bounds: Bounds<Pixels>,
        window: &mut Window,
        layout_manager: &crate::layout_manager_gpui::LayoutManager,
        left_scroll_y: f32,
        right_scroll_y: f32,
        left_line_height: f32,
        right_line_height: f32,
        header_height: f32,
    ) {
        // Create dummy DisplayLine arrays for LayoutManager (simplified approach)
        // In a full implementation, these would come from actual editor content
        let old_lines = Vec::new(); // TODO: Get actual lines from left editor
        let new_lines = Vec::new(); // TODO: Get actual lines from right editor

        // Use LayoutManager to render connectors
        // Note: This is a simplified integration - LayoutManager expects actual DisplayLine data
        layout_manager.render_connectors(
            window,
            &old_lines,
            &new_lines,
            bounds.origin.x.0, // pane_width approximation
            &crate::ImaraDiffAnalysis {
                blocks: blocks.to_vec(),
            },
            bounds,
            left_scroll_y,
            right_scroll_y,
            left_line_height,
            right_line_height,
            header_height,
        );
    }

    /// Draw a connector curve using cubic bezier
    fn draw_connector_curve(
        window: &mut Window,
        origin: Point<Pixels>,
        x1: f32,
        y1_start: f32,
        y1_end: f32,
        x2: f32,
        y2_start: f32,
        y2_end: f32,
        color: Hsla,
    ) {
        let segments = 32;
        let mut top_points = Vec::with_capacity(segments + 1);
        let mut bottom_points = Vec::with_capacity(segments + 1);

        let control_point_offset = (x2 - x1) * 0.35;

        // Generate curve points
        for i in 0..=segments {
            let t = i as f32 / segments as f32;

            // Top curve
            let top_start = point(px(x1), px(y1_start));
            let top_end = point(px(x2), px(y2_start));
            let top_ctrl1 = point(px(x1 + control_point_offset), px(y1_start));
            let top_ctrl2 = point(px(x2 - control_point_offset), px(y2_start));
            top_points.push(Self::cubic_bezier(
                top_start, top_ctrl1, top_ctrl2, top_end, t,
            ));

            // Bottom curve
            let bottom_start = point(px(x1), px(y1_end));
            let bottom_end = point(px(x2), px(y2_end));
            let bottom_ctrl1 = point(px(x1 + control_point_offset), px(y1_end));
            let bottom_ctrl2 = point(px(x2 - control_point_offset), px(y2_end));
            bottom_points.push(Self::cubic_bezier(
                bottom_start,
                bottom_ctrl1,
                bottom_ctrl2,
                bottom_end,
                t,
            ));
        }

        // Create path for filled connector
        let mut path_builder = PathBuilder::fill();
        path_builder.move_to(top_points[0]);

        // Add top curve
        for &point in &top_points[1..] {
            path_builder.line_to(point);
        }

        // Add bottom curve in reverse
        for &point in bottom_points.iter().rev() {
            path_builder.line_to(point);
        }

        path_builder.close();

        if let Ok(path) = path_builder.build() {
            window.paint_path(path, color);
        }
    }

    /// Cubic bezier calculation
    fn cubic_bezier(
        p0: Point<Pixels>,
        p1: Point<Pixels>,
        p2: Point<Pixels>,
        p3: Point<Pixels>,
        t: f32,
    ) -> Point<Pixels> {
        let u = 1.0 - t;
        let u2 = u * u;
        let u3 = u2 * u;
        let t2 = t * t;
        let t3 = t2 * t;

        point(
            px(u3 * p0.x.0 + 3.0 * u2 * t * p1.x.0 + 3.0 * u * t2 * p2.x.0 + t3 * p3.x.0),
            px(u3 * p0.y.0 + 3.0 * u2 * t * p1.y.0 + 3.0 * u * t2 * p2.y.0 + t3 * p3.y.0),
        )
    }

    fn estimated_line_height(&self) -> f32 {
        20.0
    }

    fn max_visual_line(&self) -> usize {
        let mut max_line = 0;

        for block in &self.change_blocks {
            if block.left_len > 0 {
                max_line = max_line.max(block.left_start_idx + block.left_len);
            }
            if block.right_len > 0 {
                max_line = max_line.max(block.right_start_idx + block.right_len);
            }
        }

        if max_line == 0 {
            for block in &self.imara_analysis.blocks {
                max_line = max_line.max(block.left_range.end.max(block.right_range.end));
            }
        }

        max_line
    }

    fn block_vertical_span(range: &Range<usize>, fallback: usize, line_height: f32) -> (f32, f32) {
        if range.is_empty() {
            let top = fallback as f32 * line_height;
            (top, top + line_height)
        } else {
            let top = range.start as f32 * line_height;
            let height = range.len().max(1) as f32 * line_height;
            (top, top + height)
        }
    }

    fn block_center(range: &Range<usize>, fallback: usize, line_height: f32) -> f32 {
        let (top, bottom) = Self::block_vertical_span(range, fallback, line_height);
        (top + bottom) * 0.5
    }

    fn adjust_alpha(color: Hsla, multiplier: f32) -> Hsla {
        Hsla {
            a: (color.a * multiplier).clamp(0.0, 1.0),
            ..color
        }
    }

    fn paint_block(
        window: &mut Window,
        origin: Point<Pixels>,
        x_start: f32,
        width: f32,
        top: f32,
        bottom: f32,
        fill: Hsla,
    ) {
        let height = (bottom - top).max(1.0);
        let bounds = Bounds {
            origin: point(origin.x + px(x_start), origin.y + px(top)),
            size: Size {
                width: px(width.max(1.0)),
                height: px(height),
            },
        };

        window.paint_quad(PaintQuad {
            bounds,
            background: fill.into(),
            border_widths: Default::default(),
            border_color: fill.into(),
            corner_radii: Corners::all(px(3.0)),
            border_style: Default::default(),
        });
    }

    fn draw_connector(
        window: &mut Window,
        origin: Point<Pixels>,
        start: (f32, f32),
        end: (f32, f32),
        color: Hsla,
    ) {
        let (start_x, start_y) = start;
        let (end_x, end_y) = end;
        let control_distance = (end_x - start_x).abs() * 0.35;

        let mut path = PathBuilder::stroke(px(2.0));
        path.move_to(point(origin.x + px(start_x), origin.y + px(start_y)));
        path.cubic_bezier_to(
            point(origin.x + px(end_x), origin.y + px(end_y)),
            point(
                origin.x + px(start_x + control_distance),
                origin.y + px(start_y),
            ),
            point(
                origin.x + px(end_x - control_distance),
                origin.y + px(end_y),
            ),
        );

        if let Ok(path) = path.build() {
            window.paint_path(path, color);
        }
    }

    /// Discover project files that have changes
    pub fn discover_project_files(&mut self) {
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
        use crate::perfect_diff_engine::compute_imara_diff_default;

        println!("Loading diff for file: {:?}", file_path);

        let git_ops = crate::git::GitOps::with_current_dir();

        // Read current content from filesystem
        let current_content = std::fs::read_to_string(file_path).unwrap_or_default();

        // Get original content from git (last committed version)
        let original_content = match git_ops.show_file("HEAD", &file_path.to_string_lossy()) {
            crate::git::GitResult {
                success: true,
                stdout,
                ..
            } => stdout,
            _ => current_content.clone(),
        };

        // Compute diff analysis
        self.imara_analysis = compute_imara_diff_default(&original_content, &current_content);

        // Update paths
        self.left_path = Some(file_path.clone());
        self.right_path = Some(file_path.clone());

        // Create change blocks
        self.change_blocks = self.create_change_blocks_from_imara();

        // Update buffers
        self.update_buffers(cx);

        // Apply highlights
        self.apply_diff_highlights(cx);

        println!("✅ File diff loaded successfully");
    }

    /// Load the default demo diff
    pub fn load_default_demo_diff(&mut self, cx: &mut Context<Self>) {
        use crate::perfect_diff_engine::compute_imara_diff_default;

        println!("Loading default demo diff");

        // Use hardcoded demo content for now
        let original_content = "import React from 'react';\nimport { BrowserRouter as Router, Routes, Route } from 'react-router-dom';\nimport { ThemeProvider } from './theme';\nimport { AuthProvider } from './auth';\nimport { NotificationProvider } from './notifications';\n\nfunction AppProviders({ children }) {\n  return (\n    <ThemeProvider>\n      <AuthProvider>\n        <NotificationProvider>\n          <Router>\n            {children}\n          </Router>\n        </NotificationProvider>\n      </AuthProvider>\n    </ThemeProvider>\n  );\n}\n\nexport default AppProviders;\n".to_string();

        let current_content = "import React from 'react';\nimport { BrowserRouter as Router, Routes, Route } from 'react-router-dom';\nimport { ThemeProvider } from './theme';\nimport { AuthProvider } from './auth';\n\nfunction AppProviders({ children }) {\n  return (\n    <ThemeProvider>\n      <AuthProvider>\n          <Router>\n            {children}\n          </Router>\n      </AuthProvider>\n    </ThemeProvider>\n  );\n}\n\nexport default AppProviders;\n".to_string();

        // Compute diff analysis
        self.imara_analysis = compute_imara_diff_default(&original_content, &current_content);

        // Create change blocks
        self.change_blocks = self.create_change_blocks_from_imara();

        // Update buffers with demo content
        self.left_editor.update(cx, |editor, cx| {
            editor.buffer().update(cx, |multi_buffer, cx| {
                multi_buffer.edit([(0..multi_buffer.len(cx), original_content)], None, cx);
            });
        });

        self.right_editor.update(cx, |editor, cx| {
            editor.buffer().update(cx, |multi_buffer, cx| {
                multi_buffer.edit([(0..multi_buffer.len(cx), current_content)], None, cx);
            });
        });

        // Apply highlights
        self.apply_diff_highlights(cx);

        println!("✅ Demo diff loaded successfully");
    }
}

impl Render for DiffViewer {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Get scroll positions from editors
        let left_scroll_y = self.left_editor.update(cx, |editor, cx| {
            editor.snapshot(window, cx).scroll_position().y
        });
        let right_scroll_y = self.right_editor.update(cx, |editor, cx| {
            editor.snapshot(window, cx).scroll_position().y
        });

        // Use fixed line height for accurate positioning
        let left_line_height = 20.0;
        let right_line_height = 20.0;

        let left_editor = self.left_editor.clone();
        let right_editor = self.right_editor.clone();
        let connector_gutter = self.render_connector_gutter(
            left_scroll_y,
            right_scroll_y,
            left_line_height,
            right_line_height,
        );

        // Create toolbar with immutable borrow
        let toolbar = {
            let toolbar_state = &self.toolbar_state;
            let can_go_previous = toolbar_state.can_go_previous();
            let can_go_next = toolbar_state.can_go_next();

            div()
                .flex()
                .flex_row()
                .items_center()
                .p_2()
                .border_b_1()
                .border_color(cx.theme().colors().border)
                .bg(cx.theme().colors().background)
                .children([
                    // Previous button
                    div()
                        .child(
                            Button::new("previous", "⬅ Previous")
                                .disabled(!can_go_previous)
                                .on_click(cx.listener(|this, _, _window, cx| {
                                    this.handle_toolbar_action("previous_file", cx);
                                })),
                        )
                        .into_any_element(),
                    // Next button
                    div()
                        .ml_2()
                        .child(
                            Button::new("next", "Next ➡")
                                .disabled(!can_go_next)
                                .on_click(cx.listener(|this, _, _window, cx| {
                                    this.handle_toolbar_action("next_file", cx);
                                })),
                        )
                        .into_any_element(),
                    // Separator
                    div()
                        .ml_4()
                        .w(px(1.0))
                        .h(px(20.0))
                        .bg(cx.theme().colors().border)
                        .into_any_element(),
                    // Demo button
                    div()
                        .ml_2()
                        .child(Button::new("demo", "Demo").on_click(cx.listener(
                            |this, _, _window, cx| {
                                this.handle_toolbar_action("default_demo", cx);
                            },
                        )))
                        .into_any_element(),
                    // Separator
                    div()
                        .ml_4()
                        .w(px(1.0))
                        .h(px(20.0))
                        .bg(cx.theme().colors().border)
                        .into_any_element(),
                    // File info
                    div()
                        .ml_2()
                        .flex_1()
                        .child(match &toolbar_state.current_mode {
                            DiffMode::ProjectFile(index) => {
                                if let Some(file) = toolbar_state.project_files.get(*index) {
                                    format!(
                                        "File: {} ({}/{})",
                                        file.display(),
                                        index + 1,
                                        toolbar_state.project_files.len()
                                    )
                                } else {
                                    "No files available".to_string()
                                }
                            }
                            DiffMode::DefaultDemo => "Demo Diff".to_string(),
                        })
                        .into_any_element(),
                ])
        };

        div()
            .key_context("DiffViewer")
            .flex()
            .flex_col()
            .size_full()
            .bg(cx.theme().colors().editor_background)
            .child(toolbar)
            .child(
                div().flex().flex_row().flex_grow().children([
                    // Left pane (Original)
                    div()
                        .flex_1()
                        .flex()
                        .flex_col()
                        .children([
                            div()
                                .p_2()
                                .border_b_1()
                                .border_color(cx.theme().colors().border)
                                .bg(cx.theme().colors().background)
                                .child(Label::new("Original"))
                                .into_any_element(),
                            div().flex_grow().child(left_editor).into_any_element(),
                        ])
                        .into_any_element(),
                    connector_gutter.into_any_element(),
                    // Right pane (Modified)
                    div()
                        .flex_1()
                        .flex()
                        .flex_col()
                        .children([
                            div()
                                .p_2()
                                .border_b_1()
                                .border_color(cx.theme().colors().border)
                                .bg(cx.theme().colors().background)
                                .child(Label::new("Modified"))
                                .into_any_element(),
                            div().flex_grow().child(right_editor).into_any_element(),
                        ])
                        .into_any_element(),
                ]),
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
    // Register diff viewer actions
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
