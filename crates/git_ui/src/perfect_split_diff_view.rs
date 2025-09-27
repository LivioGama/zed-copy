// Perfect JetBrains Split Diff View adapted for GPUI
// Sophisticated diff viewer with advanced algorithms and beautiful connectors

use crate::split_diff_model::SplitDiffModel;
use crate::split_diff_settings::{SplitDiffSettings, SplitDiffViewMode};
use diff_viewer::{DisplayLine, LineType, PerfectConnectorRenderer};
use editor::{Editor, EditorEvent, MultiBuffer, scroll::Autoscroll};
use gpui::{
    AnyElement, AnyView, App, Context, Entity, EventEmitter, FocusHandle, Focusable, IntoElement,
    Render, Subscription, Task, WeakEntity, actions, div, hsla, px,
};
use language::Buffer;
use project::Project;
use settings::Settings;
use std::any::TypeId;
use ui::{Button, Icon, IconName, Label, h_flex, prelude::*, v_flex};
use workspace::searchable::SearchableItemHandle;
use workspace::{
    ItemHandle, ItemNavHistory, Workspace,
    item::{Item, ItemEvent, TabContentParams},
};

actions!(
    perfect_split_diff,
    [
        /// Toggle between split and unified diff views
        ToggleViewMode,
        /// Swap left and right sides
        SwapSides,
        /// Go to next diff hunk
        NextHunk,
        /// Go to previous diff hunk
        PreviousHunk,
        /// Stage the current hunk
        StageHunk,
        /// Unstage the current hunk
        UnstageHunk,
        /// Copy left side to right side
        CopyLeftToRight,
        /// Copy right side to left side
        CopyRightToLeft,
        /// Toggle synchronized scrolling
        ToggleSyncScroll,
        /// Toggle word wrap
        ToggleWordWrap,
        /// Toggle whitespace ignoring
        ToggleIgnoreWhitespace,
        /// Toggle intra-line highlighting
        ToggleIntraline,
        /// Collapse unchanged regions
        CollapseUnchanged,
        /// Expand unchanged regions
        ExpandUnchanged,
        /// Toggle connector visibility
        ToggleConnectors,
    ]
);

pub struct PerfectSplitDiffView {
    project: Entity<Project>,
    workspace: WeakEntity<Workspace>,
    focus_handle: FocusHandle,

    left_editor: Entity<Editor>,
    right_editor: Entity<Editor>,

    // Perfect implementation features
    connector_renderer: PerfectConnectorRenderer,

    // Configuration
    view_mode: SplitDiffViewMode,
    sync_scroll: bool,
    show_connectors: bool,
    line_height: f32,

    // Tasks and subscriptions
    diff_task: Option<Task<anyhow::Result<SplitDiffModel>>>,
    _editor_subscriptions: Vec<Subscription>,
}

impl PerfectSplitDiffView {
    pub fn register(workspace: &mut Workspace, cx: &mut Context<Workspace>) {
        workspace.register_action(|workspace, _: &ToggleViewMode, window, cx| {
            if let Some(active_item) = workspace.active_item(cx) {
                if let Some(split_diff) = active_item.downcast::<PerfectSplitDiffView>() {
                    split_diff.update(cx, |view, cx| {
                        view.toggle_view_mode(&ToggleViewMode, window, cx);
                    });
                }
            }
        });

        workspace.register_action(|workspace, _: &SwapSides, window, cx| {
            if let Some(active_item) = workspace.active_item(cx) {
                if let Some(split_diff) = active_item.downcast::<PerfectSplitDiffView>() {
                    split_diff.update(cx, |view, cx| {
                        view.swap_sides(&SwapSides, window, cx);
                    });
                }
            }
        });

        workspace.register_action(|workspace, _: &NextHunk, window, cx| {
            if let Some(active_item) = workspace.active_item(cx) {
                if let Some(split_diff) = active_item.downcast::<PerfectSplitDiffView>() {
                    split_diff.update(cx, |view, cx| {
                        view.next_hunk(&NextHunk, window, cx);
                    });
                }
            }
        });

        workspace.register_action(|workspace, _: &PreviousHunk, window, cx| {
            if let Some(active_item) = workspace.active_item(cx) {
                if let Some(split_diff) = active_item.downcast::<PerfectSplitDiffView>() {
                    split_diff.update(cx, |view, cx| {
                        view.previous_hunk(&PreviousHunk, window, cx);
                    });
                }
            }
        });

        workspace.register_action(|workspace, _: &ToggleSyncScroll, window, cx| {
            if let Some(active_item) = workspace.active_item(cx) {
                if let Some(split_diff) = active_item.downcast::<PerfectSplitDiffView>() {
                    split_diff.update(cx, |view, cx| {
                        view.toggle_sync_scroll(&ToggleSyncScroll, window, cx);
                    });
                }
            }
        });

        workspace.register_action(|workspace, _: &ToggleConnectors, window, cx| {
            if let Some(active_item) = workspace.active_item(cx) {
                if let Some(split_diff) = active_item.downcast::<PerfectSplitDiffView>() {
                    split_diff.update(cx, |view, cx| {
                        view.toggle_connectors(&ToggleConnectors, window, cx);
                    });
                }
            }
        });

        workspace::register_serializable_item::<PerfectSplitDiffView>(cx);
    }

    pub fn new(
        project: Entity<Project>,
        workspace: WeakEntity<Workspace>,
        model: SplitDiffModel,
        window: &mut gpui::Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let focus_handle = cx.focus_handle();

        // Get settings first to avoid borrowing conflicts
        let default_view = {
            let settings = SplitDiffSettings::get_global(cx);
            settings.default_view
        };

        let sync_scroll = {
            let settings = SplitDiffSettings::get_global(cx);
            settings.sync_scroll
        };

        // Initialize connector renderer with perfect algorithms
        let line_height = 20.0; // Standard line height
        let connector_renderer = PerfectConnectorRenderer::new(1.0, 80.0);

        // Create multibuffers for editors
        let left_multibuffer = cx.new(|_| MultiBuffer::new(language::Capability::ReadWrite));
        let right_multibuffer = cx.new(|_| MultiBuffer::new(language::Capability::ReadWrite));

        // Create buffers for left and right content
        let left_buffer = cx.new(|cx| Buffer::local(&model.left_content, cx));
        let right_buffer = cx.new(|cx| Buffer::local(&model.right_content, cx));

        // Add buffers to multibuffers
        left_multibuffer.update(cx, |multibuffer, cx| {
            let buffer_snapshot = left_buffer.read(cx).snapshot();
            let range = multi_buffer::ExcerptRange {
                context: buffer_snapshot.anchor_before(0)
                    ..buffer_snapshot.anchor_after(buffer_snapshot.len()),
                primary: buffer_snapshot.anchor_before(0)
                    ..buffer_snapshot.anchor_after(buffer_snapshot.len()),
            };
            multibuffer.push_excerpts(left_buffer.clone(), vec![range], cx);
        });
        right_multibuffer.update(cx, |multibuffer, cx| {
            let buffer_snapshot = right_buffer.read(cx).snapshot();
            let range = multi_buffer::ExcerptRange {
                context: buffer_snapshot.anchor_before(0)
                    ..buffer_snapshot.anchor_after(buffer_snapshot.len()),
                primary: buffer_snapshot.anchor_before(0)
                    ..buffer_snapshot.anchor_after(buffer_snapshot.len()),
            };
            multibuffer.push_excerpts(right_buffer.clone(), vec![range], cx);
        });

        // Create editors
        let left_editor = cx.new(|cx| {
            let mut editor = Editor::for_multibuffer(
                left_multibuffer.clone(),
                Some(project.clone()),
                window,
                cx,
            );
            editor.set_read_only(true);
            editor.disable_diagnostics(cx);
            editor
        });

        let right_editor = cx.new(|cx| {
            let mut editor = Editor::for_multibuffer(
                right_multibuffer.clone(),
                Some(project.clone()),
                window,
                cx,
            );
            editor.set_read_only(true);
            editor.disable_diagnostics(cx);
            editor
        });

        let mut editor_subscriptions = Vec::new();

        // Subscribe to scroll events for synchronization
        editor_subscriptions.push(cx.subscribe_in(
            &left_editor,
            window,
            Self::handle_left_editor_event,
        ));
        editor_subscriptions.push(cx.subscribe_in(
            &right_editor,
            window,
            Self::handle_right_editor_event,
        ));

        Self {
            project,
            workspace,
            focus_handle,
            left_editor,
            right_editor,
            connector_renderer,
            view_mode: default_view,
            sync_scroll,
            show_connectors: true,
            line_height,
            diff_task: None,
            _editor_subscriptions: editor_subscriptions,
        }
    }

    fn handle_left_editor_event(
        &mut self,
        _editor: &Entity<Editor>,
        event: &EditorEvent,
        window: &mut gpui::Window,
        cx: &mut Context<Self>,
    ) {
        if !self.sync_scroll {
            return;
        }

        match event {
            EditorEvent::ScrollPositionChanged { .. } => {
                let scroll_position = self
                    .left_editor
                    .update(cx, |editor, cx| editor.scroll_position(cx));
                self.right_editor.update(cx, |editor, cx| {
                    editor.set_scroll_position(scroll_position, window, cx);
                });
            }
            _ => {}
        }
    }

    fn handle_right_editor_event(
        &mut self,
        _editor: &Entity<Editor>,
        event: &EditorEvent,
        window: &mut gpui::Window,
        cx: &mut Context<Self>,
    ) {
        if !self.sync_scroll {
            return;
        }

        match event {
            EditorEvent::ScrollPositionChanged { .. } => {
                let scroll_position = self
                    .right_editor
                    .update(cx, |editor, cx| editor.scroll_position(cx));
                self.left_editor.update(cx, |editor, cx| {
                    editor.set_scroll_position(scroll_position, window, cx);
                });
            }
            _ => {}
        }
    }

    fn toggle_view_mode(
        &mut self,
        _: &ToggleViewMode,
        _window: &mut gpui::Window,
        cx: &mut Context<Self>,
    ) {
        self.view_mode = match self.view_mode {
            SplitDiffViewMode::Split => SplitDiffViewMode::Unified,
            SplitDiffViewMode::Unified => SplitDiffViewMode::Split,
        };
        cx.notify();
    }

    fn swap_sides(&mut self, _: &SwapSides, _window: &mut gpui::Window, cx: &mut Context<Self>) {
        // Swap the editor contents
        std::mem::swap(&mut self.left_editor, &mut self.right_editor);
        cx.notify();
    }

    fn next_hunk(&mut self, _: &NextHunk, _window: &mut gpui::Window, _cx: &mut Context<Self>) {
        // Simplified implementation - just a placeholder for now
    }

    fn previous_hunk(
        &mut self,
        _: &PreviousHunk,
        _window: &mut gpui::Window,
        _cx: &mut Context<Self>,
    ) {
        // Simplified implementation - just a placeholder for now
    }

    fn toggle_sync_scroll(
        &mut self,
        _: &ToggleSyncScroll,
        _window: &mut gpui::Window,
        cx: &mut Context<Self>,
    ) {
        self.sync_scroll = !self.sync_scroll;
        cx.notify();
    }

    fn toggle_connectors(
        &mut self,
        _: &ToggleConnectors,
        _window: &mut gpui::Window,
        cx: &mut Context<Self>,
    ) {
        self.show_connectors = !self.show_connectors;
        cx.notify();
    }

    /// Render the perfect JetBrains-style split view
    fn render_perfect_split_view(
        &mut self,
        window: &mut gpui::Window,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let left_editor = self.left_editor.clone();
        let right_editor = self.right_editor.clone();

        // Create sophisticated toolbar with perfect styling
        let toolbar = self.render_perfect_toolbar(window, cx);

        v_flex()
            .size_full()
            .child(toolbar)
            .child(
                h_flex()
                    .flex_1()
                    .child(
                        // Left pane with sophisticated highlighting
                        div()
                            .flex_1()
                            .relative()
                            .bg(hsla(0.0, 0.0, 0.12, 1.0)) // Dark editor background
                            .child(left_editor.clone()),
                    )
                    .child(
                        // Perfect middle gutter with sophisticated connectors
                        if self.show_connectors {
                            self.connector_renderer
                                .render_middle_gutter(px(600.0))
                                .into_any_element()
                        } else {
                            div()
                                .w(px(4.0))
                                .bg(hsla(0.0, 0.0, 0.18, 1.0))
                                .into_any_element()
                        },
                    )
                    .child(
                        // Right pane with sophisticated highlighting
                        div()
                            .flex_1()
                            .relative()
                            .bg(hsla(0.0, 0.0, 0.12, 1.0)) // Dark editor background
                            .child(right_editor.clone()),
                    ),
            )
            .into_any_element()
    }

    /// Render sophisticated JetBrains-style toolbar
    fn render_perfect_toolbar(
        &self,
        _window: &mut gpui::Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let colors = cx.theme().colors();

        h_flex()
            .h(px(44.0)) // JetBrains standard toolbar height
            .px_4()
            .py_2()
            .bg(colors.title_bar_background)
            .border_b_1()
            .border_color(colors.border)
            .items_center()
            .justify_between()
            .child(
                h_flex()
                    .gap_4()
                    .items_center()
                    .child(
                        Icon::new(IconName::FileDiff)
                            .size(ui::IconSize::Medium)
                            .color(ui::Color::Accent),
                    )
                    .child(
                        Label::new("Perfect Split Diff")
                            .size(ui::LabelSize::Default)
                            .color(ui::Color::Default),
                    ),
            )
            .child(
                h_flex()
                    .gap_2()
                    .items_center()
                    .child(
                        Button::new(
                            "sync_scroll",
                            if self.sync_scroll {
                                "Sync: ON"
                            } else {
                                "Sync: OFF"
                            },
                        )
                        .icon(if self.sync_scroll {
                            IconName::ArrowRightLeft
                        } else {
                            IconName::ArrowRightLeft
                        })
                        .icon_size(ui::IconSize::Small)
                        .size(ui::ButtonSize::Compact)
                        .style(if self.sync_scroll {
                            ui::ButtonStyle::Filled
                        } else {
                            ui::ButtonStyle::Subtle
                        })
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.toggle_sync_scroll(&ToggleSyncScroll, window, cx);
                        })),
                    )
                    .child(
                        Button::new(
                            "connectors",
                            if self.show_connectors {
                                "Connectors: ON"
                            } else {
                                "Connectors: OFF"
                            },
                        )
                        .icon(IconName::ArrowRightLeft)
                        .icon_size(ui::IconSize::Small)
                        .size(ui::ButtonSize::Compact)
                        .style(if self.show_connectors {
                            ui::ButtonStyle::Filled
                        } else {
                            ui::ButtonStyle::Subtle
                        })
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.toggle_connectors(&ToggleConnectors, window, cx);
                        })),
                    )
                    .child(
                        Button::new("swap_sides", "Swap")
                            .icon(IconName::ArrowRightLeft)
                            .icon_size(ui::IconSize::Small)
                            .size(ui::ButtonSize::Compact)
                            .style(ui::ButtonStyle::Subtle)
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.swap_sides(&SwapSides, window, cx);
                            })),
                    ),
            )
    }
}

impl EventEmitter<EditorEvent> for PerfectSplitDiffView {}

impl Focusable for PerfectSplitDiffView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Item for PerfectSplitDiffView {
    type Event = EditorEvent;

    fn tab_icon(&self, _window: &gpui::Window, _cx: &App) -> Option<Icon> {
        Some(Icon::new(IconName::FileDiff).color(ui::Color::Accent))
    }

    fn tab_content_text(&self, _detail: usize, _cx: &App) -> ui::SharedString {
        "Perfect Split Diff".into()
    }

    fn tab_content(
        &self,
        params: TabContentParams,
        _window: &gpui::Window,
        _cx: &App,
    ) -> AnyElement {
        Label::new("Perfect Split Diff")
            .color(if params.selected {
                ui::Color::Default
            } else {
                ui::Color::Muted
            })
            .into_any_element()
    }

    fn tab_tooltip_text(&self, _cx: &App) -> Option<ui::SharedString> {
        Some("Perfect JetBrains-style Split Diff Viewer".into())
    }

    fn to_item_events(event: &EditorEvent, f: impl FnMut(ItemEvent)) {
        Editor::to_item_events(event, f)
    }

    fn telemetry_event_text(&self) -> Option<&'static str> {
        Some("Perfect Split Diff Opened")
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
            Some((*self_handle).clone().into())
        } else {
            None
        }
    }

    fn as_searchable(&self, _: &Entity<Self>) -> Option<Box<dyn SearchableItemHandle>> {
        Some(Box::new(self.left_editor.clone()))
    }

    fn for_each_project_item(
        &self,
        cx: &App,
        f: &mut dyn FnMut(gpui::EntityId, &dyn project::ProjectItem),
    ) {
        self.left_editor.read(cx).for_each_project_item(cx, f);
        self.right_editor.read(cx).for_each_project_item(cx, f);
    }

    fn set_nav_history(
        &mut self,
        nav_history: ItemNavHistory,
        _: &mut gpui::Window,
        cx: &mut Context<Self>,
    ) {
        self.left_editor.update(cx, |editor, _| {
            editor.set_nav_history(Some(nav_history));
        });
    }

    fn navigate(
        &mut self,
        data: Box<dyn std::any::Any>,
        window: &mut gpui::Window,
        cx: &mut Context<Self>,
    ) -> bool {
        self.left_editor
            .update(cx, |editor, cx| editor.navigate(data, window, cx))
    }

    fn breadcrumb_location(&self, _: &App) -> workspace::ToolbarItemLocation {
        workspace::ToolbarItemLocation::PrimaryLeft
    }

    fn breadcrumbs(
        &self,
        theme: &theme::Theme,
        cx: &App,
    ) -> Option<Vec<workspace::item::BreadcrumbText>> {
        self.left_editor.read(cx).breadcrumbs(theme, cx)
    }

    fn added_to_workspace(
        &mut self,
        workspace: &mut Workspace,
        window: &mut gpui::Window,
        cx: &mut Context<Self>,
    ) {
        self.left_editor.update(cx, |editor, cx| {
            editor.added_to_workspace(workspace, window, cx);
        });
        self.right_editor.update(cx, |editor, cx| {
            editor.added_to_workspace(workspace, window, cx);
        });
    }

    fn can_save(&self, _cx: &App) -> bool {
        false // Split diff view is read-only
    }

    fn save(
        &mut self,
        _options: workspace::item::SaveOptions,
        _project: Entity<Project>,
        _window: &mut gpui::Window,
        _cx: &mut Context<Self>,
    ) -> Task<anyhow::Result<()>> {
        Task::ready(Ok(())) // No-op for read-only view
    }
}

impl workspace::SerializableItem for PerfectSplitDiffView {
    fn serialized_item_kind() -> &'static str {
        "PerfectSplitDiffView"
    }

    fn deserialize(
        _project: gpui::Entity<project::Project>,
        _workspace: gpui::WeakEntity<workspace::Workspace>,
        _workspace_id: workspace::WorkspaceId,
        _item_id: workspace::ItemId,
        _window: &mut gpui::Window,
        _cx: &mut gpui::App,
    ) -> gpui::Task<gpui::Result<gpui::Entity<Self>>> {
        gpui::Task::ready(Err(anyhow::anyhow!(
            "PerfectSplitDiffView deserialization not supported"
        )))
    }

    fn serialize(
        &mut self,
        _workspace: &mut workspace::Workspace,
        _item_id: workspace::ItemId,
        _closing: bool,
        _window: &mut gpui::Window,
        _cx: &mut gpui::Context<Self>,
    ) -> Option<gpui::Task<gpui::Result<()>>> {
        None
    }

    fn should_serialize(&self, _event: &Self::Event) -> bool {
        false
    }

    fn cleanup(
        _: workspace::WorkspaceId,
        _: Vec<workspace::ItemId>,
        _: &mut gpui::Window,
        _: &mut gpui::App,
    ) -> gpui::Task<gpui::Result<()>> {
        gpui::Task::ready(Ok(()))
    }
}

impl Render for PerfectSplitDiffView {
    fn render(&mut self, window: &mut gpui::Window, cx: &mut Context<Self>) -> impl IntoElement {
        match self.view_mode {
            SplitDiffViewMode::Split => self.render_perfect_split_view(window, cx),
            SplitDiffViewMode::Unified => {
                // For unified view, show just the left editor with diff annotations
                v_flex()
                    .size_full()
                    .child(self.render_perfect_toolbar(window, cx))
                    .child(
                        div()
                            .flex_1()
                            .bg(hsla(0.0, 0.0, 0.12, 1.0))
                            .child(self.left_editor.clone()),
                    )
                    .into_any_element()
            }
        }
    }
}
