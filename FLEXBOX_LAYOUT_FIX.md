# Flexbox Layout Fix - Split Diff Viewer

## The Problem

The split diff viewer was expanding beyond the viewport height, making bottom borders invisible and preventing proper scrolling. Content height was dictating container height instead of being constrained.

## Root Causes

### 1. Missing Flex Container on PURPLE Wrapper

**Issue:** The PURPLE wrapper had `.flex_1()` (making it a flex child) but was NOT a flex container itself.
**Impact:** Its child (RED DiffViewer) couldn't use `.flex_1()` to be constrained.
**Fix:** Added `.flex().flex_col()` to PURPLE wrapper.

```rust
// Before
div().flex_1().min_h_0().w_full().child(split_diff_view)

// After
div().flex_1().min_h_0().w_full().flex().flex_col().child(split_diff_view)
```

### 2. Wrong Height Constraint on GREEN & MAGENTA Columns

**Issue:** GREEN and MAGENTA used `.min_h_0()` but they are children of a **horizontal** flex container (YELLOW).
**Impact:** In horizontal flex, `.min_h_0()` doesn't constrain height - you need `.h_full()` to match parent height.
**Fix:** Changed from `.min_h_0()` to `.h_full()`.

```rust
// Before (children of h_flex)
v_flex().flex_1().min_h_0().overflow_hidden()

// After (children of h_flex)
v_flex().flex_1().h_full().overflow_hidden()
```

### 3. ORANGE Container Expanding Beyond Viewport

**Issue:** ORANGE had `.size_full()` which tries to be 100% of parent, but content was pushing it larger.
**Impact:** The entire container expanded by ~41px beyond viewport, hiding bottom border.
**Fix:** Added `.overflow_hidden()` to clip overflow and keep it constrained.

```rust
// Before
div().flex().flex_col().size_full()

// After
div().flex().flex_col().size_full().overflow_hidden()
```

## The Golden Rules of Flexbox in GPUI

### For Column Flex (v_flex):

- **Parent:** Use `v_flex()` or `.flex().flex_col()`
- **Children:** Use `.flex_1().min_h_0().w_full()` to be constrained in height
- **Min height 0** breaks the chain where content dictates parent height

### For Row Flex (h_flex):

- **Parent:** Use `h_flex()` or `.flex().flex_row()`
- **Children:** Use `.flex_1()` for width distribution, `.h_full()` to match parent height
- **Min height doesn't apply** - use explicit height instead

### Critical Pattern:

**Every parent of a `.flex_1()` child MUST be a flex container**

```rust
// ❌ Wrong - parent is not flex
div().child(
    div().flex_1() // This won't work!
)

// ✅ Correct - parent is flex container
div().flex().flex_col().child(
    div().flex_1() // Now it works!
)
```

## Final Layout Hierarchy

```
ORANGE (div)           .flex().flex_col().size_full().overflow_hidden()
├─ PURPLE (div)        .flex_1().min_h_0().w_full().flex().flex_col()
   └─ RED (v_flex)     .flex_1().min_h_0().w_full()
      └─ YELLOW (h_flex) .flex_1().min_h_0().overflow_hidden()
         ├─ GREEN (v_flex)   .flex_1().h_full().overflow_hidden()
         │  └─ CYAN (div)    .flex_1().min_h_0().w_full().overflow_y_scroll()
         ├─ BLUE (div)       .w(40px).h_full()
         └─ MAGENTA (v_flex) .flex_1().h_full().overflow_hidden()
            └─ CYAN (div)    .flex_1().min_h_0().w_full().overflow_y_scroll()
```

## Key Takeaway

In flexbox, using `.flex_1()` alone doesn't magically constrain size. You need:

1. Parent MUST be a flex container (`.flex()` or `.flex_col()`)
2. Child needs `.min_h_0()` (column flex) or `.h_full()` (row flex)
3. Top-level container needs `.overflow_hidden()` to prevent expansion

Without all three, content will dictate height all the way up the tree! 🎯
