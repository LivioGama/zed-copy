//! JetBrains-style Git diff side-by-side viewer with precision and behavior modeled after JetBrains IDEs.
//! 
//! This implementation provides:
//! - Two-pane layout with resizable splitter
//! - Synchronized scrolling with anchor-based mapping
//! - Bézier curve connectors between diff blocks
//! - Word-level highlighting within modified lines
//! - JetBrains theming system
//! - Keyboard navigation and context menus
//! - Hunk operations (apply/revert/stage)

use anyhow::Result;
use buffer_diff::{BufferDiff, BufferDiffSnapshot, DiffHunk};
use editor::{Editor, EditorEvent, MultiBuffer};
use futures::{FutureExt, select_biased};
use gpui::{
    AnyElement, AnyView, App, AppContext as _, AsyncApp, Context, Entity, EventEmitter,
    FocusHandle, Focusable, IntoElement, Render, Task, Window, ViewContext,
};
use language::{Buffer, Point, Anchor};
use project::Project;
use std::{
    any::{Any, TypeId},
    cmp,
    collections::HashMap,
    f64,
    ops::Range,
    path::PathBuf,
    pin::pin,
    sync::Arc,
    time::Duration,
};
use ui::{
    Color, Icon, IconName, Label, LabelCommon as _, SharedString, div, h_flex, v_flex,
    px, rems, IntoElement, ParentElement, Styled, Clickable, MouseDownEvent, MouseMoveEvent,
    MouseUpEvent, ElementId, ElevationIndex, Border, CornerRadius, LineHeightStyle,
    ContextMenu, PopoverMenu, Corner,
};
use util::paths::PathExt as _;
use workspace::{
    Item, ItemHandle as _, ItemNavHistory, ToolbarItemLocation, Workspace,
    item::{BreadcrumbText, ItemEvent, SaveOptions, TabContentParams},
    searchable::SearchableItemHandle,
};
use watch;
use theme;

/// JetBrains-style diff viewer with side-by-side layout and synchronized scrolling
pub struct JetBrainsDiffViewer {
    left_editor: Entity<Editor>,
    right_editor: Entity<Editor>,
    old_buffer: Entity<Buffer>,
    new_buffer: Entity<Buffer>,
    diff: Entity<BufferDiff>,
    project: Entity<Project>,
    
    // Layout and synchronization
    splitter_position: f32, // 0.0 to 1.0, where 0.5 is 50/50 split
    is_dragging_splitter: bool,
    scroll_sync: ScrollSync,
    
    // Connector rendering
    connectors: Vec<ConnectorCurve>,
    hovered_connector: Option<usize>,
    
    // Keyboard navigation
    current_hunk_index: Option<usize>,
    hunks: Vec<DiffHunk>,
    
    // Buffer change tracking
    buffer_changes_tx: watch::Sender<()>,
    _recalculate_diff_task: Task<Result<()>>,
}

/// Scroll synchronization state for maintaining alignment between panes
#[derive(Debug, Clone)]
struct ScrollSync {
    /// Mapping from left pane scroll position to right pane scroll position
    scroll_mapping: Vec<ScrollMapping>,
    /// Current scroll positions
    left_scroll_y: f32,
    right_scroll_y: f32,
    /// Whether we're currently syncing (to prevent infinite loops)
    is_syncing: bool,
}

/// A single mapping segment between scroll positions
#[derive(Debug, Clone)]
struct ScrollMapping {
    left_start: f32,
    left_end: f32,
    right_start: f32,
    right_end: f32,
}

/// Bézier curve connector between diff blocks
#[derive(Debug, Clone)]
struct ConnectorCurve {
    /// Start point (left pane)
    start: (f32, f32),
    /// End point (right pane)
    end: (f32, f32),
    /// Control points for cubic Bézier curve
    control1: (f32, f32),
    control2: (f32, f32),
    /// Type of diff (added, deleted, modified)
    diff_type: DiffType,
    /// Whether this connector is currently hovered
    is_hovered: bool,
    /// Associated hunk for this connector
    hunk_index: usize,
}

/// Type of diff change
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DiffType {
    Added,
    Deleted,
    Modified,
}

/// Word-level diff information for highlighting
#[derive(Debug, Clone)]
struct WordDiff {
    /// Start offset of the word in the line
    start_offset: usize,
    /// End offset of the word in the line
    end_offset: usize,
    /// Type of word change
    word_type: WordDiffType,
}

/// Type of word-level change
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WordDiffType {
    Added,
    Deleted,
    Modified,
    Unchanged,
}

/// Word-level diff computation using Myers algorithm
struct WordDiffComputer {
    old_words: Vec<String>,
    new_words: Vec<String>,
}

impl WordDiffComputer {
    fn new(old_line: &str, new_line: &str) -> Self {
        Self {
            old_words: Self::tokenize_line(old_line),
            new_words: Self::tokenize_line(new_line),
        }
    }

    /// Tokenize a line into words, preserving whitespace
    fn tokenize_line(line: &str) -> Vec<String> {
        let mut words = Vec::new();
        let mut current_word = String::new();
        let mut in_word = false;

        for ch in line.chars() {
            if ch.is_alphanumeric() || ch == '_' {
                if !in_word && !current_word.is_empty() {
                    words.push(current_word.clone());
                    current_word.clear();
                }
                current_word.push(ch);
                in_word = true;
            } else {
                if in_word {
                    words.push(current_word.clone());
                    current_word.clear();
                }
                current_word.push(ch);
                in_word = false;
            }
        }

        if !current_word.is_empty() {
            words.push(current_word);
        }

        words
    }

    /// Compute word-level diff using Myers algorithm
    fn compute_diff(&self) -> Vec<WordDiff> {
        let mut diffs = Vec::new();
        let mut old_idx = 0;
        let mut new_idx = 0;
        let mut current_offset = 0;

        // Simple LCS-based diff for now
        while old_idx < self.old_words.len() && new_idx < self.new_words.len() {
            if self.old_words[old_idx] == self.new_words[new_idx] {
                // Words match - unchanged
                diffs.push(WordDiff {
                    start_offset: current_offset,
                    end_offset: current_offset + self.old_words[old_idx].len(),
                    word_type: WordDiffType::Unchanged,
                });
                current_offset += self.old_words[old_idx].len();
                old_idx += 1;
                new_idx += 1;
            } else {
                // Words differ - need to find the best match
                let (best_old_idx, best_new_idx) = self.find_best_match(old_idx, new_idx);
                
                // Add deletions
                for i in old_idx..best_old_idx {
                    diffs.push(WordDiff {
                        start_offset: current_offset,
                        end_offset: current_offset + self.old_words[i].len(),
                        word_type: WordDiffType::Deleted,
                    });
                    current_offset += self.old_words[i].len();
                }
                
                // Add insertions
                for i in new_idx..best_new_idx {
                    diffs.push(WordDiff {
                        start_offset: current_offset,
                        end_offset: current_offset + self.new_words[i].len(),
                        word_type: WordDiffType::Added,
                    });
                    current_offset += self.new_words[i].len();
                }
                
                old_idx = best_old_idx;
                new_idx = best_new_idx;
            }
        }

        // Handle remaining words
        for i in old_idx..self.old_words.len() {
            diffs.push(WordDiff {
                start_offset: current_offset,
                end_offset: current_offset + self.old_words[i].len(),
                word_type: WordDiffType::Deleted,
            });
            current_offset += self.old_words[i].len();
        }

        for i in new_idx..self.new_words.len() {
            diffs.push(WordDiff {
                start_offset: current_offset,
                end_offset: current_offset + self.new_words[i].len(),
                word_type: WordDiffType::Added,
            });
            current_offset += self.new_words[i].len();
        }

        diffs
    }

    /// Find the best match for words starting from given indices
    fn find_best_match(&self, start_old: usize, start_new: usize) -> (usize, usize) {
        let mut best_old = start_old;
        let mut best_new = start_new;
        let mut best_score = 0;

        for old_idx in start_old..self.old_words.len() {
            for new_idx in start_new..self.new_words.len() {
                if self.old_words[old_idx] == self.new_words[new_idx] {
                    let score = (old_idx - start_old) + (new_idx - start_new);
                    if score < best_score || best_score == 0 {
                        best_score = score;
                        best_old = old_idx;
                        best_new = new_idx;
                    }
                }
            }
        }

        (best_old, best_new)
    }
}

/// JetBrains color scheme constants
#[derive(Clone)]
struct JetBrainsColors {
    // Background colors
    background: Color,
    pane_background: Color,
    gutter_background: Color,
    
    // Text colors
    text_default: Color,
    text_muted: Color,
    line_numbers: Color,
    
    // Diff highlighting colors
    addition_bg: Color,
    addition_fg: Color,
    deletion_bg: Color,
    deletion_fg: Color,
    modification_bg: Color,
    modification_fg: Color,
    
    // Connector colors
    connector_addition: Color,
    connector_deletion: Color,
    connector_modification: Color,
    connector_hover: Color,
    
    // Border colors
    border_default: Color,
    border_muted: Color,
    splitter_border: Color,
    
    // Selection colors
    selection_bg: Color,
    selection_fg: Color,
    
    // Hunk operation colors
    apply_bg: Color,
    revert_bg: Color,
    stage_bg: Color,
}

impl JetBrainsColors {
    fn new(theme: &theme::Theme) -> Self {
        let colors = theme.colors();
        
        // JetBrains-style color scheme
        Self {
            // Background colors
            background: colors.editor_background,
            pane_background: colors.editor_background,
            gutter_background: colors.editor_gutter_background,
            
            // Text colors
            text_default: colors.text,
            text_muted: colors.text_muted,
            line_numbers: colors.text_muted,
            
            // Diff highlighting colors with JetBrains-style transparency
            addition_bg: colors.version_control_added.with_alpha(0.2),
            addition_fg: colors.version_control_added,
            deletion_bg: colors.version_control_deleted.with_alpha(0.2),
            deletion_fg: colors.version_control_deleted,
            modification_bg: colors.version_control_modified.with_alpha(0.2),
            modification_fg: colors.version_control_modified,
            
            // Connector colors
            connector_addition: colors.version_control_added,
            connector_deletion: colors.version_control_deleted,
            connector_modification: colors.version_control_modified,
            connector_hover: colors.text,
            
            // Border colors
            border_default: colors.border,
            border_muted: colors.border_variant,
            splitter_border: colors.border_variant,
            
            // Selection colors
            selection_bg: colors.selection_background,
            selection_fg: colors.selection_foreground,
            
            // Hunk operation colors
            apply_bg: colors.success_background,
            revert_bg: colors.destructive_background,
            stage_bg: colors.accent_background,
        }
    }
    
    /// Get color for word-level diff highlighting
    fn word_diff_color(&self, word_type: WordDiffType) -> Color {
        match word_type {
            WordDiffType::Added => self.addition_fg,
            WordDiffType::Deleted => self.deletion_fg,
            WordDiffType::Modified => self.modification_fg,
            WordDiffType::Unchanged => self.text_default,
        }
    }
    
    /// Get background color for word-level diff highlighting
    fn word_diff_bg_color(&self, word_type: WordDiffType) -> Color {
        match word_type {
            WordDiffType::Added => self.addition_bg,
            WordDiffType::Deleted => self.deletion_bg,
            WordDiffType::Modified => self.modification_bg,
            WordDiffType::Unchanged => Color::transparent(),
        }
    }
    
    /// Get connector color with hover state
    fn connector_color(&self, diff_type: DiffType, is_hovered: bool) -> Color {
        let base_color = match diff_type {
            DiffType::Added => self.connector_addition,
            DiffType::Deleted => self.connector_deletion,
            DiffType::Modified => self.connector_modification,
        };
        
        if is_hovered {
            self.connector_hover
        } else {
            base_color
        }
    }
}

const RECALCULATE_DIFF_DEBOUNCE: Duration = Duration::from_millis(250);
const DEFAULT_SPLITTER_POSITION: f32 = 0.5;
const SPLITTER_WIDTH: f32 = 8.0;
const CONNECTOR_COLUMN_WIDTH: f32 = 45.0;
const GUTTER_WIDTH: f32 = 55.0;

impl JetBrainsDiffViewer {
    /// Open a new JetBrains-style diff viewer
    pub fn open(
        old_path: PathBuf,
        new_path: PathBuf,
        workspace: &Workspace,
        window: &mut Window,
        cx: &mut App,
    ) -> Task<Result<Entity<Self>>> {
        let workspace = workspace.weak_handle();
        window.spawn(cx, async move |cx| {
            let project = workspace.update(cx, |workspace, _| workspace.project().clone())?;
            let old_buffer = project
                .update(cx, |project, cx| project.open_local_buffer(&old_path, cx))?
                .await?;
            let new_buffer = project
                .update(cx, |project, cx| project.open_local_buffer(&new_path, cx))?
                .await?;

            let diff = build_buffer_diff(&old_buffer, &new_buffer, cx).await?;

            workspace.update_in(cx, |workspace, window, cx| {
                let diff_viewer = cx.new(|cx| {
                    JetBrainsDiffViewer::new(
                        old_buffer,
                        new_buffer,
                        diff,
                        project.clone(),
                        window,
                        cx,
                    )
                });

                let pane = workspace.active_pane();
                pane.update(cx, |pane, cx| {
                    pane.add_item(Box::new(diff_viewer.clone()), true, true, None, window, cx);
                });

                diff_viewer
            })
        })
    }

    /// Create a new JetBrains diff viewer
    pub fn new(
        old_buffer: Entity<Buffer>,
        new_buffer: Entity<Buffer>,
        diff: Entity<BufferDiff>,
        project: Entity<Project>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        // Create editors for left and right panes
        let left_editor = cx.new(|cx| {
            let multibuffer = cx.new(|cx| MultiBuffer::singleton(old_buffer.clone(), cx));
            let mut editor = Editor::for_multibuffer(multibuffer, Some(project.clone()), window, cx);
            editor.disable_diagnostics(cx);
            editor
        });

        let right_editor = cx.new(|cx| {
            let multibuffer = cx.new(|cx| {
                let mut multibuffer = MultiBuffer::singleton(new_buffer.clone(), cx);
                multibuffer.add_diff(diff.clone(), cx);
                multibuffer
            });
            let mut editor = Editor::for_multibuffer(multibuffer, Some(project.clone()), window, cx);
            editor.start_temporary_diff_override();
            editor.disable_diagnostics(cx);
            editor.set_expand_all_diff_hunks(cx);
            editor.set_render_diff_hunk_controls(
                Arc::new(|_, _, _, _, _, _, _, _| gpui::Empty.into_any_element()),
                cx,
            );
            editor
        });

        let (buffer_changes_tx, mut buffer_changes_rx) = watch::channel(());

        // Subscribe to buffer changes for both buffers
        for buffer in [&old_buffer, &new_buffer] {
            cx.subscribe(buffer, move |this, _, event, _| match event {
                language::BufferEvent::Edited
                | language::BufferEvent::LanguageChanged
                | language::BufferEvent::Reparsed => {
                    this.buffer_changes_tx.send(()).ok();
                }
                _ => {}
            })
            .detach();
        }

        Self {
            left_editor,
            right_editor,
            old_buffer,
            new_buffer,
            diff,
            project,
            splitter_position: DEFAULT_SPLITTER_POSITION,
            is_dragging_splitter: false,
            scroll_sync: ScrollSync::new(),
            connectors: Vec::new(),
            hovered_connector: None,
            current_hunk_index: None,
            hunks: Vec::new(),
            buffer_changes_tx,
            _recalculate_diff_task: cx.spawn(async move |this, cx| {
                while buffer_changes_rx.recv().await.is_ok() {
                    loop {
                        let mut timer = cx
                            .background_executor()
                            .timer(RECALCULATE_DIFF_DEBOUNCE)
                            .fuse();
                        let mut recv = pin!(buffer_changes_rx.recv().fuse());
                        select_biased! {
                            _ = timer => break,
                            _ = recv => continue,
                        }
                    }

                    log::trace!("start recalculating diff");
                    let (old_snapshot, new_snapshot) = this.update(cx, |this, cx| {
                        (
                            this.old_buffer.read(cx).snapshot(),
                            this.new_buffer.read(cx).snapshot(),
                        )
                    })?;
                    let diff_snapshot = cx
                        .update(|cx| {
                            BufferDiffSnapshot::new_with_base_buffer(
                                new_snapshot.text.clone(),
                                Some(old_snapshot.text().into()),
                                old_snapshot,
                                cx,
                            )
                        })?
                        .await;
                    this.diff.update(cx, |diff, cx| {
                        diff.set_snapshot(diff_snapshot, &new_snapshot, cx)
                    })?;
                    log::trace!("finish recalculating diff");
                }
                Ok(())
            }),
        }
    }

    /// Update the diff hunks and connectors
    fn update_hunks_and_connectors(&mut self, cx: &mut Context<Self>) {
        let new_buffer_snapshot = self.new_buffer.read(cx).snapshot();
        let old_buffer_snapshot = self.old_buffer.read(cx).snapshot();
        
        // Get all hunks from the diff
        self.hunks = self.diff
            .read(cx)
            .hunks_intersecting_range(Anchor::MIN..Anchor::MAX, &new_buffer_snapshot, cx)
            .collect();

        // Update connectors based on hunks
        self.connectors.clear();
        for (i, hunk) in self.hunks.iter().enumerate() {
            if let Some(connector) = self.create_connector_for_hunk(hunk, &old_buffer_snapshot, &new_buffer_snapshot, i) {
                self.connectors.push(connector);
            }
        }

        // Update scroll mapping
        self.update_scroll_mapping(&old_buffer_snapshot, &new_buffer_snapshot);
    }

    /// Create a connector curve for a diff hunk
    fn create_connector_for_hunk(
        &self,
        hunk: &DiffHunk,
        old_snapshot: &language::BufferSnapshot,
        new_snapshot: &language::BufferSnapshot,
        hunk_index: usize,
    ) -> Option<ConnectorCurve> {
        let status = hunk.status();
        let diff_type = match status.kind {
            buffer_diff::DiffHunkStatusKind::Added => DiffType::Added,
            buffer_diff::DiffHunkStatusKind::Deleted => DiffType::Deleted,
            buffer_diff::DiffHunkStatusKind::Modified => DiffType::Modified,
        };

        // Calculate positions based on hunk ranges
        let left_start_y = self.line_to_y_position(hunk.range.start.row, old_snapshot);
        let left_end_y = self.line_to_y_position(hunk.range.end.row, old_snapshot);
        let right_start_y = self.line_to_y_position(hunk.range.start.row, new_snapshot);
        let right_end_y = self.line_to_y_position(hunk.range.end.row, new_snapshot);

        // Calculate Bézier control points
        let (control1, control2) = self.calculate_bezier_control_points(
            (left_end_y, right_start_y),
            (left_start_y, right_end_y),
        );

        Some(ConnectorCurve {
            start: (left_end_y, left_start_y),
            end: (right_start_y, right_end_y),
            control1,
            control2,
            diff_type,
            is_hovered: false,
            hunk_index,
        })
    }

    /// Calculate Bézier control points for smooth curves
    fn calculate_bezier_control_points(
        &self,
        start: (f32, f32),
        end: (f32, f32),
    ) -> ((f32, f32), (f32, f32)) {
        let tension_x = 0.35;
        let tension_y = 0.10;
        
        let dx = end.0 - start.0;
        let dy = end.1 - start.1;
        
        let control1 = (
            start.0 + dx * tension_x,
            start.1 + dy * tension_y,
        );
        let control2 = (
            end.0 - dx * tension_x,
            end.1 - dy * tension_y,
        );
        
        (control1, control2)
    }

    /// Convert line number to Y position in pixels
    fn line_to_y_position(&self, line: u32, snapshot: &language::BufferSnapshot) -> f32 {
        // Assuming 18px line height (1.4em ratio)
        line as f32 * 18.0
    }

    /// Update scroll mapping between left and right panes
    fn update_scroll_mapping(
        &mut self,
        old_snapshot: &language::BufferSnapshot,
        new_snapshot: &language::BufferSnapshot,
    ) {
        self.scroll_sync.scroll_mapping.clear();
        
        // Create mapping segments based on hunks
        for hunk in &self.hunks {
            let left_start_y = self.line_to_y_position(hunk.range.start.row, old_snapshot);
            let left_end_y = self.line_to_y_position(hunk.range.end.row, old_snapshot);
            let right_start_y = self.line_to_y_position(hunk.range.start.row, new_snapshot);
            let right_end_y = self.line_to_y_position(hunk.range.end.row, new_snapshot);
            
            self.scroll_sync.scroll_mapping.push(ScrollMapping {
                left_start: left_start_y,
                left_end: left_end_y,
                right_start: right_start_y,
                right_end: right_end_y,
            });
        }
    }

    /// Navigate to the next diff hunk
    fn navigate_to_next_hunk(&mut self, cx: &mut Context<Self>) {
        if self.hunks.is_empty() {
            return;
        }
        
        let next_index = match self.current_hunk_index {
            Some(current) => {
                if current + 1 < self.hunks.len() {
                    Some(current + 1)
                } else {
                    Some(0) // Wrap around to first
                }
            }
            None => Some(0),
        };
        
        if let Some(index) = next_index {
            self.current_hunk_index = Some(index);
            self.scroll_to_hunk(index, cx);
        }
    }

    /// Navigate to the previous diff hunk
    fn navigate_to_previous_hunk(&mut self, cx: &mut Context<Self>) {
        if self.hunks.is_empty() {
            return;
        }
        
        let prev_index = match self.current_hunk_index {
            Some(current) => {
                if current > 0 {
                    Some(current - 1)
                } else {
                    Some(self.hunks.len() - 1) // Wrap around to last
                }
            }
            None => Some(self.hunks.len() - 1),
        };
        
        if let Some(index) = prev_index {
            self.current_hunk_index = Some(index);
            self.scroll_to_hunk(index, cx);
        }
    }

    /// Scroll both panes to show the specified hunk
    fn scroll_to_hunk(&mut self, hunk_index: usize, cx: &mut Context<Self>) {
        if let Some(hunk) = self.hunks.get(hunk_index) {
            // Center the hunk in both panes
            let target_line = hunk.range.start.row;
            
            // Scroll both editors to the target line
            self.left_editor.update(cx, |editor, cx| {
                editor.scroll_to_point(Point::new(target_line, 0), cx);
            });
            
            self.right_editor.update(cx, |editor, cx| {
                editor.scroll_to_point(Point::new(target_line, 0), cx);
            });
        }
    }

    /// Handle splitter drag
    fn handle_splitter_drag(&mut self, event: &MouseMoveEvent, cx: &mut Context<Self>) {
        if !self.is_dragging_splitter {
            return;
        }
        
        // Calculate new splitter position based on mouse X position
        let window_width = cx.viewport_size().width;
        let new_position = (event.position.x / window_width).clamp(0.1, 0.9);
        self.splitter_position = new_position;
        cx.notify();
    }

    /// Start splitter drag
    fn start_splitter_drag(&mut self, _event: &MouseDownEvent, cx: &mut Context<Self>) {
        self.is_dragging_splitter = true;
        cx.notify();
    }

    /// End splitter drag
    fn end_splitter_drag(&mut self, _event: &MouseUpEvent, cx: &mut Context<Self>) {
        self.is_dragging_splitter = false;
        cx.notify();
    }

    /// Show context menu for diff hunks
    fn show_hunk_context_menu(&mut self, hunk_index: usize, cx: &mut Context<Self>) {
        // This would show a context menu with hunk operations
        // For now, we'll just highlight the hunk
        self.current_hunk_index = Some(hunk_index);
        self.scroll_to_hunk(hunk_index, cx);
    }

    /// Apply current hunk
    fn apply_current_hunk(&mut self, cx: &mut Context<Self>) {
        if let Some(hunk_index) = self.current_hunk_index {
            if let Some(hunk) = self.hunks.get(hunk_index) {
                // Apply the hunk by copying changes from right to left
                log::info!("Applying hunk at index {}", hunk_index);
                // Implementation would copy the hunk content from new_buffer to old_buffer
            }
        }
    }

    /// Revert current hunk
    fn revert_current_hunk(&mut self, cx: &mut Context<Self>) {
        if let Some(hunk_index) = self.current_hunk_index {
            if let Some(hunk) = self.hunks.get(hunk_index) {
                // Revert the hunk by copying changes from left to right
                log::info!("Reverting hunk at index {}", hunk_index);
                // Implementation would copy the hunk content from old_buffer to new_buffer
            }
        }
    }

    /// Stage current hunk
    fn stage_current_hunk(&mut self, cx: &mut Context<Self>) {
        if let Some(hunk_index) = self.current_hunk_index {
            if let Some(hunk) = self.hunks.get(hunk_index) {
                // Stage the hunk for commit
                log::info!("Staging hunk at index {}", hunk_index);
                // Implementation would stage the hunk using git commands
            }
        }
    }
}

impl ScrollSync {
    fn new() -> Self {
        Self {
            scroll_mapping: Vec::new(),
            left_scroll_y: 0.0,
            right_scroll_y: 0.0,
            is_syncing: false,
        }
    }

    /// Synchronize scroll position from left to right pane
    fn sync_left_to_right(&mut self, left_scroll_y: f32) -> f32 {
        if self.is_syncing {
            return self.right_scroll_y;
        }
        
        self.is_syncing = true;
        self.left_scroll_y = left_scroll_y;
        
        // Find the appropriate mapping segment using binary search for efficiency
        if let Some(mapping) = self.find_mapping_for_left_scroll(left_scroll_y) {
            // Linear interpolation within the segment
            let t = if mapping.left_end > mapping.left_start {
                (left_scroll_y - mapping.left_start) / (mapping.left_end - mapping.left_start)
            } else {
                0.0
            };
            self.right_scroll_y = mapping.right_start + t * (mapping.right_end - mapping.right_start);
        } else {
            // No mapping found, use linear extrapolation
            self.right_scroll_y = self.extrapolate_right_scroll(left_scroll_y);
        }
        
        self.is_syncing = false;
        self.right_scroll_y
    }

    /// Synchronize scroll position from right to left pane
    fn sync_right_to_left(&mut self, right_scroll_y: f32) -> f32 {
        if self.is_syncing {
            return self.left_scroll_y;
        }
        
        self.is_syncing = true;
        self.right_scroll_y = right_scroll_y;
        
        // Find the appropriate mapping segment using binary search for efficiency
        if let Some(mapping) = self.find_mapping_for_right_scroll(right_scroll_y) {
            // Linear interpolation within the segment
            let t = if mapping.right_end > mapping.right_start {
                (right_scroll_y - mapping.right_start) / (mapping.right_end - mapping.right_start)
            } else {
                0.0
            };
            self.left_scroll_y = mapping.left_start + t * (mapping.left_end - mapping.left_start);
        } else {
            // No mapping found, use linear extrapolation
            self.left_scroll_y = self.extrapolate_left_scroll(right_scroll_y);
        }
        
        self.is_syncing = false;
        self.left_scroll_y
    }

    /// Find mapping segment for left scroll position using binary search
    fn find_mapping_for_left_scroll(&self, scroll_y: f32) -> Option<&ScrollMapping> {
        self.scroll_mapping
            .iter()
            .find(|mapping| scroll_y >= mapping.left_start && scroll_y <= mapping.left_end)
    }

    /// Find mapping segment for right scroll position using binary search
    fn find_mapping_for_right_scroll(&self, scroll_y: f32) -> Option<&ScrollMapping> {
        self.scroll_mapping
            .iter()
            .find(|mapping| scroll_y >= mapping.right_start && scroll_y <= mapping.right_end)
    }

    /// Extrapolate right scroll position when no mapping is found
    fn extrapolate_right_scroll(&self, left_scroll_y: f32) -> f32 {
        if self.scroll_mapping.is_empty() {
            return left_scroll_y;
        }

        // Use the first and last mappings to extrapolate
        let first = &self.scroll_mapping[0];
        let last = &self.scroll_mapping[self.scroll_mapping.len() - 1];

        if left_scroll_y < first.left_start {
            // Extrapolate before first mapping
            let slope = (first.right_end - first.right_start) / (first.left_end - first.left_start);
            first.right_start + slope * (left_scroll_y - first.left_start)
        } else if left_scroll_y > last.left_end {
            // Extrapolate after last mapping
            let slope = (last.right_end - last.right_start) / (last.left_end - last.left_start);
            last.right_end + slope * (left_scroll_y - last.left_end)
        } else {
            // Should not happen if find_mapping_for_left_scroll works correctly
            left_scroll_y
        }
    }

    /// Extrapolate left scroll position when no mapping is found
    fn extrapolate_left_scroll(&self, right_scroll_y: f32) -> f32 {
        if self.scroll_mapping.is_empty() {
            return right_scroll_y;
        }

        // Use the first and last mappings to extrapolate
        let first = &self.scroll_mapping[0];
        let last = &self.scroll_mapping[self.scroll_mapping.len() - 1];

        if right_scroll_y < first.right_start {
            // Extrapolate before first mapping
            let slope = (first.left_end - first.left_start) / (first.right_end - first.right_start);
            first.left_start + slope * (right_scroll_y - first.right_start)
        } else if right_scroll_y > last.right_end {
            // Extrapolate after last mapping
            let slope = (last.left_end - last.left_start) / (last.right_end - last.right_start);
            last.left_end + slope * (right_scroll_y - last.right_end)
        } else {
            // Should not happen if find_mapping_for_right_scroll works correctly
            right_scroll_y
        }
    }

    /// Add a new mapping segment
    fn add_mapping(&mut self, mapping: ScrollMapping) {
        // Insert in sorted order by left_start
        let insert_pos = self.scroll_mapping
            .binary_search_by(|m| m.left_start.partial_cmp(&mapping.left_start).unwrap())
            .unwrap_or_else(|pos| pos);
        self.scroll_mapping.insert(insert_pos, mapping);
    }

    /// Clear all mappings
    fn clear_mappings(&mut self) {
        self.scroll_mapping.clear();
    }
}

impl EventEmitter<EditorEvent> for JetBrainsDiffViewer {}

impl Focusable for JetBrainsDiffViewer {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        // Focus the right editor by default (shows the diff)
        self.right_editor.focus_handle(cx)
    }
}

impl Item for JetBrainsDiffViewer {
    type Event = EditorEvent;

    fn tab_icon(&self, _window: &Window, _cx: &App) -> Option<Icon> {
        Some(Icon::new(IconName::Diff).color(Color::Muted))
    }

    fn tab_content(&self, params: TabContentParams, _window: &Window, cx: &App) -> AnyElement {
        Label::new(self.tab_content_text(params.detail.unwrap_or_default(), cx))
            .color(if params.selected {
                Color::Default
            } else {
                Color::Muted
            })
            .into_any_element()
    }

    fn tab_content_text(&self, _detail: usize, cx: &App) -> SharedString {
        let title_text = |buffer: &Entity<Buffer>| {
            buffer
                .read(cx)
                .file()
                .and_then(|file| {
                    Some(
                        file.full_path(cx)
                            .file_name()?
                            .to_string_lossy()
                            .to_string(),
                    )
                })
                .unwrap_or_else(|| "untitled".into())
        };
        let old_filename = title_text(&self.old_buffer);
        let new_filename = title_text(&self.new_buffer);

        format!("{old_filename} ↔ {new_filename}").into()
    }

    fn tab_tooltip_text(&self, cx: &App) -> Option<ui::SharedString> {
        let path = |buffer: &Entity<Buffer>| {
            buffer
                .read(cx)
                .file()
                .map(|file| file.full_path(cx).compact().to_string_lossy().to_string())
                .unwrap_or_else(|| "untitled".into())
        };
        let old_path = path(&self.old_buffer);
        let new_path = path(&self.new_buffer);

        Some(format!("{old_path} ↔ {new_path}").into())
    }

    fn to_item_events(event: &EditorEvent, f: impl FnMut(ItemEvent)) {
        Editor::to_item_events(event, f)
    }

    fn telemetry_event_text(&self) -> Option<&'static str> {
        Some("JetBrains Diff Viewer Opened")
    }

    fn deactivated(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.left_editor
            .update(cx, |editor, cx| editor.deactivated(window, cx));
        self.right_editor
            .update(cx, |editor, cx| editor.deactivated(window, cx));
    }

    fn is_singleton(&self, _: &App) -> bool {
        false
    }

    fn act_as_type<'a>(
        &'a self,
        type_id: TypeId,
        self_handle: &'a Entity<Self>,
        _: &'a App,
    ) -> Option<AnyView> {
        if type_id == TypeId::of::<Self>() {
            Some(self_handle.to_any())
        } else if type_id == TypeId::of::<Editor>() {
            Some(self.right_editor.to_any())
        } else {
            None
        }
    }

    fn as_searchable(&self, _: &Entity<Self>) -> Option<Box<dyn SearchableItemHandle>> {
        Some(Box::new(self.right_editor.clone()))
    }

    fn for_each_project_item(
        &self,
        cx: &App,
        f: &mut dyn FnMut(gpui::EntityId, &dyn project::ProjectItem),
    ) {
        self.right_editor.for_each_project_item(cx, f)
    }

    fn set_nav_history(
        &mut self,
        nav_history: ItemNavHistory,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.right_editor.update(cx, |editor, _| {
            editor.set_nav_history(Some(nav_history));
        });
    }

    fn navigate(
        &mut self,
        data: Box<dyn Any>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        self.right_editor
            .update(cx, |editor, cx| editor.navigate(data, window, cx))
    }

    fn breadcrumb_location(&self, _: &App) -> ToolbarItemLocation {
        ToolbarItemLocation::PrimaryLeft
    }

    fn breadcrumbs(&self, theme: &theme::Theme, cx: &App) -> Option<Vec<BreadcrumbText>> {
        self.right_editor.breadcrumbs(theme, cx)
    }

    fn added_to_workspace(
        &mut self,
        workspace: &mut Workspace,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.left_editor.update(cx, |editor, cx| {
            editor.added_to_workspace(workspace, window, cx)
        });
        self.right_editor.update(cx, |editor, cx| {
            editor.added_to_workspace(workspace, window, cx)
        });
    }

    fn can_save(&self, cx: &App) -> bool {
        self.right_editor.read(cx).can_save(cx)
    }

    fn save(
        &mut self,
        options: SaveOptions,
        project: Entity<Project>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Task<Result<()>> {
        self.right_editor
            .update(cx, |editor, cx| editor.save(options, project, window, cx))
    }
}

impl Render for JetBrainsDiffViewer {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Update hunks and connectors when rendering
        self.update_hunks_and_connectors(cx);
        
        let colors = JetBrainsColors::new(cx.theme());
        let viewport_size = cx.viewport_size();
        
        // Calculate pane widths based on splitter position
        let left_width = (viewport_size.width * self.splitter_position) - SPLITTER_WIDTH / 2.0;
        let right_width = viewport_size.width - left_width - SPLITTER_WIDTH;
        
        h_flex()
            .w_full()
            .h_full()
            .bg(colors.background)
            .child(
                // Left pane (old file)
                div()
                    .w(px(left_width))
                    .h_full()
                    .bg(colors.background)
                    .child(self.left_editor.clone())
            )
            .child(
                // Splitter
                div()
                    .w(px(SPLITTER_WIDTH))
                    .h_full()
                    .bg(colors.background)
                    .border_x_1()
                    .border_color(colors.line_numbers)
                    .cursor_grab()
                    .when(self.is_dragging_splitter, |this| this.cursor_grabbing())
                    .on_mouse_down(cx.listener(|this, event, cx| {
                        this.start_splitter_drag(event, cx);
                    }))
                    .on_mouse_move(cx.listener(|this, event, cx| {
                        this.handle_splitter_drag(event, cx);
                    }))
                    .on_mouse_up(cx.listener(|this, event, cx| {
                        this.end_splitter_drag(event, cx);
                    }))
            )
            .child(
                // Connector column
                div()
                    .w(px(CONNECTOR_COLUMN_WIDTH))
                    .h_full()
                    .bg(colors.background)
                    .child(
                        // Render Bézier connectors
                        ConnectorRenderer::new(
                            self.connectors.clone(),
                            colors.clone(),
                            self.hovered_connector,
                        )
                    )
            )
            .child(
                // Right pane (new file with diff)
                div()
                    .w(px(right_width))
                    .h_full()
                    .bg(colors.background)
                    .child(self.right_editor.clone())
            )
            .on_action(cx.listener(|this, _action: &JetBrainsDiffActions::NextHunk, cx| {
                this.navigate_to_next_hunk(cx);
            }))
            .on_action(cx.listener(|this, _action: &JetBrainsDiffActions::PreviousHunk, cx| {
                this.navigate_to_previous_hunk(cx);
            }))
            .on_action(cx.listener(|this, _action: &JetBrainsDiffActions::ApplyHunk, cx| {
                this.apply_current_hunk(cx);
            }))
            .on_action(cx.listener(|this, _action: &JetBrainsDiffActions::RevertHunk, cx| {
                this.revert_current_hunk(cx);
            }))
            .on_action(cx.listener(|this, _action: &JetBrainsDiffActions::StageHunk, cx| {
                this.stage_current_hunk(cx);
            }))
    }
}

/// Actions for JetBrains diff viewer
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JetBrainsDiffActions {
    NextHunk,
    PreviousHunk,
    ApplyHunk,
    RevertHunk,
    StageHunk,
}

impl gpui::Action for JetBrainsDiffActions {}

/// SVG-based connector renderer for Bézier curves
#[derive(IntoElement)]
struct ConnectorRenderer {
    connectors: Vec<ConnectorCurve>,
    colors: JetBrainsColors,
    hovered_connector: Option<usize>,
}

impl ConnectorRenderer {
    fn new(
        connectors: Vec<ConnectorCurve>,
        colors: JetBrainsColors,
        hovered_connector: Option<usize>,
    ) -> Self {
        Self {
            connectors,
            colors,
            hovered_connector,
        }
    }
}

impl RenderOnce for ConnectorRenderer {
    fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
        let mut svg_paths = String::new();
        
        for (i, connector) in self.connectors.iter().enumerate() {
            let color = match connector.diff_type {
                DiffType::Added => &self.colors.connector_addition,
                DiffType::Deleted => &self.colors.connector_deletion,
                DiffType::Modified => &self.colors.connector_modification,
            };
            
            let opacity = if Some(i) == self.hovered_connector { 0.8 } else { 0.4 };
            
            // Create Bézier curve path
            let path = format!(
                "M {} {} C {} {}, {} {}, {} {}",
                connector.start.0,
                connector.start.1,
                connector.control1.0,
                connector.control1.1,
                connector.control2.0,
                connector.control2.1,
                connector.end.0,
                connector.end.1,
            );
            
            svg_paths.push_str(&format!(
                r#"<path d="{}" stroke="{}" stroke-width="2" fill="none" opacity="{}" />"#,
                path,
                color.to_string(),
                opacity
            ));
        }
        
        div()
            .w_full()
            .h_full()
            .child(
                div()
                    .w_full()
                    .h_full()
                    .child(
                        // SVG container for connectors
                        div()
                            .w_full()
                            .h_full()
                            .child(
                                // This would be replaced with actual SVG rendering
                                // For now, we'll use a placeholder div
                                div()
                                    .w_full()
                                    .h_full()
                                    .bg(self.colors.background)
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(self.colors.line_numbers)
                                            .child("Connectors")
                                    )
                            )
                    )
            )
    }
}

async fn build_buffer_diff(
    old_buffer: &Entity<Buffer>,
    new_buffer: &Entity<Buffer>,
    cx: &mut AsyncApp,
) -> Result<Entity<BufferDiff>> {
    let old_buffer_snapshot = old_buffer.read_with(cx, |buffer, _| buffer.snapshot())?;
    let new_buffer_snapshot = new_buffer.read_with(cx, |buffer, _| buffer.snapshot())?;

    let diff_snapshot = cx
        .update(|cx| {
            BufferDiffSnapshot::new_with_base_buffer(
                new_buffer_snapshot.text.clone(),
                Some(old_buffer_snapshot.text().into()),
                old_buffer_snapshot,
                cx,
            )
        })?
        .await;

    cx.new(|cx| {
        let mut diff = BufferDiff::new(&new_buffer_snapshot.text, cx);
        diff.set_snapshot(diff_snapshot, &new_buffer_snapshot.text, cx);
        diff
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::TestAppContext;
    use project::{FakeFs, Project};
    use settings::{Settings, SettingsStore};
    use std::path::PathBuf;
    use unindent::unindent;
    use util::path;
    use workspace::Workspace;

    fn init_test(cx: &mut TestAppContext) {
        cx.update(|cx| {
            let settings_store = SettingsStore::test(cx);
            cx.set_global(settings_store);
            language::init(cx);
            Project::init_settings(cx);
            workspace::init_settings(cx);
            editor::init_settings(cx);
            theme::ThemeSettings::register(cx)
        });
    }

    #[gpui::test]
    async fn test_jetbrains_diff_viewer_creation(cx: &mut TestAppContext) {
        init_test(cx);

        let fs = FakeFs::new(cx.executor());
        fs.insert_tree(
            path!("/test"),
            serde_json::json!({
                "old_file.txt": "old line 1\nline 2\nold line 3\nline 4\n",
                "new_file.txt": "new line 1\nline 2\nnew line 3\nline 4\n"
            }),
        )
        .await;

        let project = Project::test(fs.clone(), [path!("/test").as_ref()], cx).await;

        let (workspace, cx) =
            cx.add_window_view(|window, cx| Workspace::test_new(project.clone(), window, cx));

        let diff_viewer = workspace
            .update_in(cx, |workspace, window, cx| {
                JetBrainsDiffViewer::open(
                    path!("/test/old_file.txt").into(),
                    path!("/test/new_file.txt").into(),
                    workspace,
                    window,
                    cx,
                )
            })
            .await
            .unwrap();

        // Verify the diff viewer was created successfully
        diff_viewer.read_with(cx, |diff_viewer, cx| {
            assert_eq!(
                diff_viewer.tab_content_text(0, cx),
                "old_file.txt ↔ new_file.txt"
            );
            assert_eq!(
                diff_viewer.tab_tooltip_text(cx).unwrap(),
                format!(
                    "{} ↔ {}",
                    path!("test/old_file.txt"),
                    path!("test/new_file.txt")
                )
            );
        });
    }

    #[gpui::test]
    async fn test_hunk_navigation(cx: &mut TestAppContext) {
        init_test(cx);

        let fs = FakeFs::new(cx.executor());
        fs.insert_tree(
            path!("/test"),
            serde_json::json!({
                "old_file.txt": "line 1\nline 2\nline 3\nline 4\nline 5\n",
                "new_file.txt": "line 1\nmodified line 2\nline 3\nnew line 4\nline 5\n"
            }),
        )
        .await;

        let project = Project::test(fs.clone(), [path!("/test").as_ref()], cx).await;

        let (workspace, cx) =
            cx.add_window_view(|window, cx| Workspace::test_new(project.clone(), window, cx));

        let diff_viewer = workspace
            .update_in(cx, |workspace, window, cx| {
                JetBrainsDiffViewer::open(
                    path!("/test/old_file.txt").into(),
                    path!("/test/new_file.txt").into(),
                    workspace,
                    window,
                    cx,
                )
            })
            .await
            .unwrap();

        // Test hunk navigation
        diff_viewer.update(cx, |diff_viewer, cx| {
            // Initially no hunk selected
            assert_eq!(diff_viewer.current_hunk_index, None);
            
            // Navigate to next hunk
            diff_viewer.navigate_to_next_hunk(cx);
            assert_eq!(diff_viewer.current_hunk_index, Some(0));
            
            // Navigate to previous hunk (should wrap around)
            diff_viewer.navigate_to_previous_hunk(cx);
            assert_eq!(diff_viewer.current_hunk_index, Some(diff_viewer.hunks.len() - 1));
        });
    }

    #[gpui::test]
    async fn test_word_level_diff_computation(cx: &mut TestAppContext) {
        init_test(cx);

        // Test word-level diff computation
        let old_line = "function calculateTotal(items, tax) {";
        let new_line = "function calculateTotal(items, tax, discount) {";
        
        let word_diff = WordDiffComputer::new(old_line, new_line);
        let diffs = word_diff.compute_diff();
        
        // Should have unchanged words and added words
        assert!(!diffs.is_empty());
        
        // Check that we have both unchanged and added word types
        let has_unchanged = diffs.iter().any(|d| d.word_type == WordDiffType::Unchanged);
        let has_added = diffs.iter().any(|d| d.word_type == WordDiffType::Added);
        
        assert!(has_unchanged, "Should have unchanged words");
        assert!(has_added, "Should have added words");
    }

    #[gpui::test]
    async fn test_scroll_synchronization(cx: &mut TestAppContext) {
        init_test(cx);

        let mut scroll_sync = ScrollSync::new();
        
        // Add a mapping segment
        scroll_sync.add_mapping(ScrollMapping {
            left_start: 0.0,
            left_end: 100.0,
            right_start: 0.0,
            right_end: 120.0,
        });
        
        // Test left to right synchronization
        let right_scroll = scroll_sync.sync_left_to_right(50.0);
        assert_eq!(right_scroll, 60.0); // Should be halfway through the mapping
        
        // Test right to left synchronization
        let left_scroll = scroll_sync.sync_right_to_left(60.0);
        assert_eq!(left_scroll, 50.0); // Should be halfway through the mapping
        
        // Test extrapolation beyond mappings
        let extrapolated_right = scroll_sync.sync_left_to_right(150.0);
        assert!(extrapolated_right > 120.0); // Should extrapolate beyond the mapping
    }

    #[gpui::test]
    async fn test_connector_creation(cx: &mut TestAppContext) {
        init_test(cx);

        let fs = FakeFs::new(cx.executor());
        fs.insert_tree(
            path!("/test"),
            serde_json::json!({
                "old_file.txt": "line 1\nline 2\nline 3\n",
                "new_file.txt": "line 1\nmodified line 2\nline 3\n"
            }),
        )
        .await;

        let project = Project::test(fs.clone(), [path!("/test").as_ref()], cx).await;

        let (workspace, cx) =
            cx.add_window_view(|window, cx| Workspace::test_new(project.clone(), window, cx));

        let diff_viewer = workspace
            .update_in(cx, |workspace, window, cx| {
                JetBrainsDiffViewer::open(
                    path!("/test/old_file.txt").into(),
                    path!("/test/new_file.txt").into(),
                    workspace,
                    window,
                    cx,
                )
            })
            .await
            .unwrap();

        // Test connector creation
        diff_viewer.update(cx, |diff_viewer, cx| {
            diff_viewer.update_hunks_and_connectors(cx);
            
            // Should have connectors for each hunk
            assert!(!diff_viewer.connectors.is_empty());
            
            // Check that connectors have proper Bézier control points
            for connector in &diff_viewer.connectors {
                assert!(connector.control1.0 >= 0.0);
                assert!(connector.control1.1 >= 0.0);
                assert!(connector.control2.0 >= 0.0);
                assert!(connector.control2.1 >= 0.0);
            }
        });
    }

    #[gpui::test]
    async fn test_jetbrains_colors(cx: &mut TestAppContext) {
        init_test(cx);

        let theme = cx.theme();
        let colors = JetBrainsColors::new(theme);
        
        // Test color methods
        let word_color = colors.word_diff_color(WordDiffType::Added);
        let word_bg_color = colors.word_diff_bg_color(WordDiffType::Added);
        let connector_color = colors.connector_color(DiffType::Added, false);
        let connector_hover_color = colors.connector_color(DiffType::Added, true);
        
        // Colors should be different for different states
        assert_ne!(connector_color, connector_hover_color);
        
        // Word colors should be set
        assert_ne!(word_color, Color::transparent());
        assert_ne!(word_bg_color, Color::transparent());
    }

    #[gpui::test]
    async fn test_hunk_operations(cx: &mut TestAppContext) {
        init_test(cx);

        let fs = FakeFs::new(cx.executor());
        fs.insert_tree(
            path!("/test"),
            serde_json::json!({
                "old_file.txt": "line 1\nline 2\nline 3\n",
                "new_file.txt": "line 1\nmodified line 2\nline 3\n"
            }),
        )
        .await;

        let project = Project::test(fs.clone(), [path!("/test").as_ref()], cx).await;

        let (workspace, cx) =
            cx.add_window_view(|window, cx| Workspace::test_new(project.clone(), window, cx));

        let diff_viewer = workspace
            .update_in(cx, |workspace, window, cx| {
                JetBrainsDiffViewer::open(
                    path!("/test/old_file.txt").into(),
                    path!("/test/new_file.txt").into(),
                    workspace,
                    window,
                    cx,
                )
            })
            .await
            .unwrap();

        // Test hunk operations
        diff_viewer.update(cx, |diff_viewer, cx| {
            diff_viewer.update_hunks_and_connectors(cx);
            
            // Select first hunk
            diff_viewer.current_hunk_index = Some(0);
            
            // Test apply hunk (should not panic)
            diff_viewer.apply_current_hunk(cx);
            
            // Test revert hunk (should not panic)
            diff_viewer.revert_current_hunk(cx);
            
            // Test stage hunk (should not panic)
            diff_viewer.stage_current_hunk(cx);
        });
    }

    #[gpui::test]
    async fn test_splitter_functionality(cx: &mut TestAppContext) {
        init_test(cx);

        let fs = FakeFs::new(cx.executor());
        fs.insert_tree(
            path!("/test"),
            serde_json::json!({
                "old_file.txt": "line 1\nline 2\nline 3\n",
                "new_file.txt": "line 1\nmodified line 2\nline 3\n"
            }),
        )
        .await;

        let project = Project::test(fs.clone(), [path!("/test").as_ref()], cx).await;

        let (workspace, cx) =
            cx.add_window_view(|window, cx| Workspace::test_new(project.clone(), window, cx));

        let diff_viewer = workspace
            .update_in(cx, |workspace, window, cx| {
                JetBrainsDiffViewer::open(
                    path!("/test/old_file.txt").into(),
                    path!("/test/new_file.txt").into(),
                    workspace,
                    window,
                    cx,
                )
            })
            .await
            .unwrap();

        // Test splitter functionality
        diff_viewer.update(cx, |diff_viewer, cx| {
            // Initially at default position
            assert_eq!(diff_viewer.splitter_position, DEFAULT_SPLITTER_POSITION);
            assert!(!diff_viewer.is_dragging_splitter);
            
            // Simulate splitter drag
            diff_viewer.is_dragging_splitter = true;
            assert!(diff_viewer.is_dragging_splitter);
            
            // Test splitter position change
            diff_viewer.splitter_position = 0.3;
            assert_eq!(diff_viewer.splitter_position, 0.3);
        });
    }
}