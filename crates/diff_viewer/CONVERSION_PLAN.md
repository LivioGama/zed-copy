# Complete GPUI Conversion Plan for Diff Viewer

## Overview

Convert the `gpui_version` (Canvas-based imperative rendering) to use real GPUI's declarative element tree system in `src/`.

## Architecture Comparison

### Current (gpui_version)

- **Rendering**: Imperative Canvas API (`canvas.draw_rect()`, `canvas.draw_text()`, etc.)
- **Events**: Custom `GpuiEvent` enum with manual handling
- **Layout**: Manual pixel calculations and positioning
- **State**: Mutable app state with direct mutations
- **Font**: ab_glyph FontArc with manual text rendering

### Target (Real GPUI)

- **Rendering**: Declarative element tree (`div().bg().child()`)
- **Events**: GPUI's built-in event system (`on_click`, actions, etc.)
- **Layout**: Flexbox with automatic layout
- **State**: Entity-based with reactive updates via `cx.notify()`
- **Font**: GPUI's text rendering with Zed's font system

---

## Phase 1: Core Infrastructure (Foundation)

### 1.1 Update DiffViewerApp Structure

**File**: `src/app/mod.rs`

**Changes**:

- Remove `FontArc`, use GPUI's text system
- Remove manual viewport width/height tracking (GPUI handles this)
- Keep all business logic methods intact
- Add scroll handles for GPUI scroll tracking

**Keep**:

- `StateManager`, `ActionHandler`, `ConfigManager`
- All navigation methods
- All file loading methods
- Color palette

### 1.2 Convert DiffViewerElement to GPUI Entity

**File**: `src/app/mod.rs`

**Current**:

```rust
pub struct DiffViewerElement {
    pub app: DiffViewerApp,
}
```

**Target**:

```rust
pub struct DiffViewerElement {
    app: DiffViewerApp,
    left_scroll_handle: ScrollHandle,
    right_scroll_handle: ScrollHandle,
    focus_handle: FocusHandle,
}
```

### 1.3 Replace GpuiApp trait with Render trait

**Current**: `impl GpuiApp for DiffViewerApp`
**Target**: `impl Render for DiffViewerElement`

---

## Phase 2: Render Method Conversion (The Big One)

### 2.1 Header Rendering

**From**: `draw_header(canvas: &mut Canvas)`
**To**: `render_header() -> impl IntoElement`

**Conversion**:

```rust
// OLD (gpui_version)
fn draw_header(&self, canvas: &mut Canvas) {
    canvas.draw_rect(bounds, COLOR_HEADER);
    canvas.draw_text(&font, title, pos, size, color);
}

// NEW (real GPUI)
fn render_header(&self, cx: &mut Context<Self>) -> impl IntoElement {
    div()
        .h(px(HEADER_HEIGHT))
        .bg(header_color)
        .child(
            h_flex()
                .justify_between()
                .child(Label::new(title).color(title_color).text_size(px(18.0)))
                .child(Label::new(instructions).color(instruction_color))
        )
}
```

**Components to create**:

- Header title (left side)
- Keyboard shortcuts/instructions (right side)
- File counter/demo indicator

### 2.2 Diff Column Rendering

**From**: `draw_diff_columns(canvas: &mut Canvas)` + `draw_column_lines()`
**To**: `render_diff_panes() -> impl IntoElement`

**Structure**:

```rust
fn render_diff_panes(&self) -> impl IntoElement {
    h_flex()
        .flex_1()
        .child(self.render_left_pane())
        .child(self.render_connector_column())
        .child(self.render_right_pane())
}
```

**Each pane needs**:

- Gutter with line numbers (50-64px wide)
- Content area with text
- Scrollable container
- Diff highlighting (background colors)
- Active block highlighting

### 2.3 Line Rendering

**From**: Manual `draw_text()` calls in loops
**To**: Element children with proper styling

```rust
fn render_line(&self, line: &DisplayLine, idx: usize) -> impl IntoElement {
    let bg_color = match line.line_type {
        LineType::Addition => self.palette.addition_background,
        LineType::Deletion => self.palette.deletion_background,
        LineType::Modification => self.palette.modification_background,
        LineType::Context => transparent(),
    };

    h_flex()
        .bg(bg_color)
        .min_h(px(self.line_height()))
        .child(
            // Line number gutter
            div()
                .w(px(GUTTER_WIDTH))
                .child(Label::new(format!("{}", idx + 1)))
        )
        .child(
            // Line content
            div().flex_1().child(Label::new(&line.content))
        )
}
```

### 2.4 Connector Rendering

**From**: `draw_connectors()` with manual Bezier curve drawing
**To**: Custom GPUI element or Canvas element

**Challenge**: Connectors are complex curves. Options:

1. **Use GPUI's Canvas element** (if available for custom drawing)
2. **Create SVG-like path element**
3. **Use positioned divs with borders** (hacky but works)
4. **Custom element with paint method** (preferred)

**Solution**: Create `ConnectorElement` that implements `IntoElement` with custom painting:

```rust
struct ConnectorElement {
    curves: Vec<ConnectorCurve>,
    palette: DiffPalette,
}

impl IntoElement for ConnectorElement {
    // Custom paint implementation for Bezier curves
}
```

---

## Phase 3: Event Handling

### 3.1 Replace GpuiEvent with GPUI Actions

**From**: `handle_event(event: GpuiEvent)`
**To**: GPUI actions + keyboard handlers

**Convert**:

```rust
// OLD
match event {
    GpuiEvent::KeyPress { code: KeyCode::Up, .. } => scroll_up(),
    GpuiEvent::KeyPress { code: KeyCode::Down, .. } => scroll_down(),
}

// NEW
actions!(diff_viewer, [ScrollUp, ScrollDown, NextDiff, PrevDiff]);

impl DiffViewerElement {
    fn scroll_up(&mut self, _: &ScrollUp, cx: &mut Context<Self>) {
        // scroll logic
        cx.notify();
    }
}
```

**Actions needed**:

- `ScrollUp`, `ScrollDown`
- `ScrollPageUp`, `ScrollPageDown`
- `NextDiffBlock`, `PrevDiffBlock`
- `NextFile`, `PrevFile`
- `LoadDemo`

### 3.2 Keyboard Bindings

Register in keymap:

- Arrow keys → Scroll
- Page Up/Down → Page scroll
- J/K → Next/Previous diff block
- [/] → Next/Previous file
- D → Load demo

### 3.3 Mouse Events

- Scroll wheel → Scroll handling (handled by GPUI scroll containers)
- Click on lines → Focus/selection (future enhancement)

---

## Phase 4: Scroll Management

### 4.1 Remove Manual Scroll State

**From**: `scroll_offset: f32` with manual tracking
**To**: GPUI `ScrollHandle`

```rust
struct DiffViewerElement {
    left_scroll_handle: ScrollHandle,
    right_scroll_handle: ScrollHandle,
}

// Usage in render:
div()
    .id("left-pane")
    .track_scroll(&self.left_scroll_handle)
    .overflow_y_scroll()
```

### 4.2 Synchronized Scrolling

**Challenge**: Keep left and right panes in sync

**Solution**: Use scroll handles to read/write positions:

```rust
fn sync_scroll(&mut self, cx: &mut Context<Self>) {
    let left_offset = self.left_scroll_handle.offset();
    self.right_scroll_handle.set_offset(left_offset);
    cx.notify();
}
```

---

## Phase 5: State Management & Reactivity

### 5.1 Convert to Entity-based Updates

**Current**: Direct mutations
**Target**: Entity updates with notifications

```rust
// OLD
self.state_manager.update_state(|state| {
    state.left_scroll_offset = new_offset;
});

// NEW
viewer.update(cx, |this, cx| {
    this.app.state_manager.update_state(|state| {
        state.left_scroll_offset = new_offset;
    });
    cx.notify(); // Trigger re-render
});
```

### 5.2 Keep StateManager As-Is

The `StateManager` and `AppState` logic is good and can be reused directly. Only the render layer changes.

---

## Phase 6: Text & Font Handling

### 6.1 Remove ab_glyph Font

**From**: `FontArc` with manual glyph rendering
**To**: GPUI's `Label` component with theme fonts

### 6.2 Use Zed's Font System

GPUI handles fonts automatically through themes. Use:

- `Label::new()` for text
- `.text_size()` for sizing
- `.font_family()` if needed
- `.color()` for colors

---

## Phase 7: Layout Calculations

### 7.1 Remove Manual Layout Math

**From**: Manual pixel calculations for column widths, positions
**To**: Flexbox automatic layout

**Example**:

```rust
// OLD
let column_width = ((width - (2.0 * (gutter_width + padding)) - connector_spacing) / 2.0).max(MIN_WIDTH);
let left_x = padding;
let right_x = left_x + gutter_width + column_width + connector_spacing;

// NEW
h_flex()
    .gap(px(4.0))  // connector spacing
    .child(div().flex_1())  // left pane (auto-sized)
    .child(div().flex_1())  // right pane (auto-sized)
```

### 7.2 Keep Line Height Calculations

The `line_height()` method is still useful for scroll calculations and can be kept.

---

## Phase 8: Integration & Polish

### 8.1 Update demo.rs Example

Make sure the demo properly:

- Initializes GPUI app
- Creates window with proper settings
- Loads diff data
- Handles window lifecycle

### 8.2 Testing Checklist

- [ ] Header displays correctly
- [ ] Both diff panes visible
- [ ] Line numbers show
- [ ] Diff highlighting works (colors)
- [ ] Scrolling works
- [ ] Keyboard shortcuts work
- [ ] Connectors display (if implemented)
- [ ] File switching works
- [ ] Demo mode loads
- [ ] Navigation between diff blocks works

### 8.3 Performance Optimization

- Use `div().id()` for scroll containers
- Avoid recreating elements unnecessarily
- Consider virtual scrolling for large files (future)

---

## Phase 9: Advanced Features (Post-MVP)

### 9.1 Syntax Highlighting

- Integrate with Zed's syntax highlighter
- Apply token colors to code

### 9.2 Word-Level Diff Highlighting

- Use `word_highlights` from `DisplayLine`
- Render inline highlights within lines

### 9.3 Interactive Connectors

- Hover effects
- Click to navigate
- Visual feedback

---

## Implementation Order (Step-by-Step)

### Week 1: Foundation

1. ✅ Fix integer overflow bug
2. Update `DiffViewerElement` structure with scroll handles
3. Add `FocusHandle` for keyboard handling
4. Convert `Render` trait implementation (basic structure)

### Week 2: Basic Rendering

5. Implement `render_header()`
6. Implement basic `render_diff_panes()` without connectors
7. Implement `render_line()` with highlighting
8. Test basic two-pane view

### Week 3: Scrolling & Events

9. Add scroll handles and tracking
10. Implement keyboard actions
11. Add scroll synchronization
12. Test all navigation features

### Week 4: Connectors & Polish

13. Implement connector rendering (custom element)
14. Add active block highlighting
15. Test with real git diffs
16. Performance optimization

---

## File-by-File Conversion Map

### Core Files (Must Convert)

- `src/app/mod.rs` - Main app & render logic ⚠️ **CRITICAL**
- `src/diff_viewer_ui.rs` - Public API (update initialization)

### Keep As-Is (Business Logic)

- `src/state/` - State management ✅
- `src/diff/` - Diff computation ✅
- `src/git/` - Git operations ✅
- `src/models/` - Data structures ✅
- `src/sync/` - Sync calculations ✅
- `src/navigation/` - Navigation logic ✅
- `src/toolbar.rs` - Toolbar state ✅
- `src/config/` - Configuration ✅

### Adapt/Update (Rendering Helpers)

- `src/ui/line_renderer.rs` - Convert to element-based ⚙️
- `src/ui/connector_renderer.rs` - Convert to custom element ⚙️
- `src/ui/layout/` - May not be needed with flexbox ⚠️

### Remove/Replace

- `src/rendering/` - egui-specific, replace with GPUI ❌
- `src/theme/jetbrains_theme.rs` - Use GPUI themes ❌
- `src/syntax/` - Use Zed's syntax system later ⏸️

---

## Success Criteria

The conversion is complete when:

1. ✅ Demo runs without crashes
2. ✅ Shows header with title and instructions
3. ✅ Shows two side-by-side panes
4. ✅ Line numbers display correctly
5. ✅ Diff highlighting shows (green/red/blue)
6. ✅ Both panes scroll independently
7. ✅ Keyboard shortcuts work (arrows, page up/down, J/K, [/])
8. ✅ Connectors display between changed lines (if implemented)
9. ✅ Active diff block highlights
10. ✅ Multiple files can be navigated
11. ✅ Demo mode works with sample files
12. ✅ No console errors or panics

---

## Risk Areas

1. **Connector Rendering**: Most complex part, may need custom GPUI element
2. **Scroll Synchronization**: Ensuring panes stay aligned
3. **Performance**: Large files with many lines
4. **Font Rendering**: Matching the look of the original
5. **Event Handling**: Proper focus and keyboard routing

---

## Notes

- The `gpui_version` is the **reference implementation** - match its behavior exactly
- Don't simplify or remove features
- Business logic (diff computation, state management) stays unchanged
- Only the rendering layer (Canvas → GPUI elements) changes
- Test incrementally - don't try to convert everything at once
- Keep both versions running during development for comparison

---

## Next Steps

Run through this plan phase by phase. Start with Phase 1, then Phase 2.1 (header), then Phase 2.2 (panes without connectors), and build up from there.

Each phase should result in a working (if incomplete) demo that can be tested.
