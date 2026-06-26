# TUI Architecture

## Overview

The TUI is built with [Ratatui](https://ratatui.rs) (v0.29) + [crossterm](https://crates.io/crates/crossterm) (v0.28).  
The event loop lives in `cli/src/tui/mod.rs`, event dispatch in `cli/src/tui/events.rs`, and all application state in `cli/src/tui/app.rs`.

---

## Screen Layout

The terminal is divided into three vertical sections:

```
┌─ postlab v0.2.0-hash ───────────────────────┐  ← row 0 (nav bar top border)
│ 1. Dashboard  2. Packages  3. Security  ... │  ← row 1 (nav bar text)
├──────────────────────────────────────────────┤  ← row 2 (nav bar bottom border)
│                                              │  ← row 3+ (content area)
│  (screen-specific content)                   │
│                                              │
├──────────────────────────────────────────────┤  ← last row (status bar)
```

| Section    | Height | Terminal rows |
|------------|--------|---------------|
| Nav bar    | 3      | 0, 1, 2       |
| Content    | dynamic| 3 .. term_h-2 |
| Status bar | 1      | term_h-1      |

Within the content area, each screen may have a **sub-tab bar** at the top.

---

## Tab Bar Rendering

All tab bars use Ratatui's `Tabs` widget wrapped in a `Block`.  
Click detection in `tab_at_col()` replicates Ratatui's sequential layout engine:

```
[left_pad=1][title][right_pad=1][divider: d][left_pad=1][next_title]...
                ^^^^ tab's clickable region ^^^^
```

Where `d` depends on whether a `.divider()` override was set in Rust code.

Every layout property below MUST be kept in sync between the Rust rendering code and the click handler(s) in `events.rs`, or else mouse clicks will miss their targets silently (no compile error will mark the mismatch, only UX degradation at runtime). If they drift apart you will get **silent offset bugs** that are hard to catch.

### Nav bar (`cli/src/tui/mod.rs`)

| Property       | Value                            |
|----------------|----------------------------------|
| Block borders  | `Borders::ALL`                   |
| Block title    | Version string (rendered on top border line) |
| Divider        | Default `"│"` (1 char)           |
| Tab height     | 3 rows (border, text, border)    |
| Text row (term)| **row 1**                        |
| Inner left x   | **1** (after left border)        |

### Dashboard / System sub-tabs

| Property       | Value                            |
|----------------|----------------------------------|
| Block borders  | `Borders::BOTTOM`                |
| Divider        | `" │ "` (3 chars)                |
| Tab height     | **2 rows** (text, bottom border) |
| Text row (term)| **row 3** (content starts at row 3) |
| Inner left x   | **0** (no left border)           |

### Security sub-tabs

| Property       | Value                            |
|----------------|----------------------------------|
| Block borders  | `Borders::ALL`                   |
| Divider        | `" │ "` (3 chars)                |
| Tab height     | 3 rows                           |
| Text row (term)| **row 4**                        |
| Inner left x   | 1                                |

### Packages / Networking / Docker / WasmCloud sub-tabs

| Property       | Value                            |
|----------------|----------------------------------|
| Block borders  | `Borders::ALL`                   |
| Divider        | Default `"│"` (1 char)           |
| Tab height     | 3 rows                           |
| Text row (term)| **row 4**                        |
| Inner left x   | 1                                |

### Agent sub-tabs

| Property       | Value                            |
|----------------|----------------------------------|
| Block borders  | `Borders::ALL`                   |
| Divider        | Default `"│"` (1 char)           |
| Tab height     | 3 rows                           |
| Text row (term)| **row 4**                        |
| Inner left x   | 1                                |

---

## Mouse Capture — How It Works

### Enabling mouse events (in `cli/src/tui/mod.rs`)

```rust
// On startup:
execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;

// On shutdown:
execute!(terminal.backend_mut(), LeaveAlternateScreen, DisableMouseCapture)?;
```

### Event loop

```rust
match event::read()? {
    Event::Key(key) => { /* existing keyboard handler */ }
    Event::Mouse(mouse) => {
        if matches!(mouse.kind, MouseEventKind::Up(MouseButton::Left)) {
            events::handle_click(app, mouse.column, mouse.row);
        }
    }
    _ => {}
}
```

### Click dispatch (`handle_click` in `cli/src/tui/events.rs`)

1. If `row == 1` — dispatch to nav bar tab detection (switches screens).
2. Otherwise — match `app.screen` and route to the correct sub-tab handler.

### Position calculator (`tab_at_col`)

```rust
fn tab_at_col(col: u16, titles: &[&str], inner_left: u16, divider_w: u16) -> Option<usize>
```

Simulates Ratatui's `render_tabs()`:

```rust
x = inner_left
for each tab i:
    title_len = titles[i].len()
    tab_end = x + 1 + title_len + 1 + (divider_w if not last else 0)
    if col falls within [x, x + 1 + title_len + 1) -> return Some(i)
    x = tab_end
```

Returns `None` if the click lands on a divider or outside all rendered tabs.

---

## Guidelines for Adding Click Support to a New Screen

1. **Identify the tab bar's properties:**
   - What is the block border style (`Borders::ALL` or `Borders::BOTTOM`)?
   - What is the divider (check `.divider()` call; default is `"│"` = 1 char)?
   - What is the tab bar height (3 if `Borders::ALL`, 2 if `Borders::BOTTOM`)?
   - What terminal row is the text on? (content starts at row 3, so `Borders::ALL` → text at row 4, `Borders::BOTTOM` → text at row 3)
   - Is there a header before the tab bar? (e.g., Automation has a 3-row header first)

2. **Add a handler branch** in `handle_click()` inside `events.rs`:
   ```rust
   Screen::MyScreen => {
       if row == TEXT_ROW {
           let titles: Vec<&str> = MyTab::all().iter().map(|t| t.title()).collect();
           if let Some(idx) = tab_at_col(col, &titles, INNER_LEFT_X, DIVIDER_WIDTH) {
               app.my_screen.active_tab = MyTab::all()[idx].clone();
           }
       }
   }
   ```

3. **Keep the table above in sync** — these constants are not checked at compile time.

---

## Critical Warning

**The click position calculations in `tab_at_col()` rely on the exact same layout parameters used when rendering the `Tabs` widget.** If the rendering code changes — different divider, different padding, different border style, different height, or a header is added/removed — the corresponding click handler constants **must** be updated too. There is no compile-time link between the two; mismatches cause silent, hard-to-debug click misalignment.