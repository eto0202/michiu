# cargo-iu_ui

CLI tool for [`iu_ui`](https://github.com/eto0202/michiu/tree/main/iu_ui) — an accessible UI component catalog for Rust compliant with WCAG 2.2 and WAI-ARIA.

Instead of heavy dependencies, `cargo-iu_ui` lets you copy accessible, self-contained UI components directly into your own project (inspired by shadcn/ui).

---

## Installation

```bash
cargo install cargo-iu_ui
```

---

## Commands

| Command                         | Description                                                                     |
| :------------------------------ | :------------------------------------------------------------------------------ |
| `cargo iu_ui init`              | Sets up directory structure, downloads `core.rs`, and generates `setting.toml`. |
| `cargo iu_ui list`              | Lists all available catalog components, themes, and tags with install status.   |
| `cargo iu_ui add <comps...>`    | Downloads and adds one or more components (e.g., `simple_button`).              |
| `cargo iu_ui add --all`         | Installs all components from the catalog.                                       |
| `cargo iu_ui theme <themes...>` | Downloads accessible color themes (e.g., `simple_theme`).                       |
| `cargo iu_ui tag <tags...>`     | Downloads accessibility inference tags.                                         |
| `cargo iu_ui utils <utils...>`  | Downloads shared utility helpers.                                               |
| `cargo iu_ui check`             | Checks installed components/assets against remote for available updates.        |

---

## Quick Start

### 1. Initialize iu_ui in your project

```bash
cargo iu_ui init
```

This will create `src/iu_ui/` with `core.rs`, `mod.rs`, and a configuration file `iu_ui.toml`.

### 2. Add components

```bash
# Add single or multiple components
cargo iu_ui add simple_button simple_input

# Related static assets (like SVG icons) are automatically downloaded and tracked!
```

### 3. Use in your application

Add `mod iu_ui;` to your `src/main.rs` or `src/lib.rs`:

```rust
mod iu_ui;

use iu_ui::prelude::*; // Brings in MichiuWidget, MichiuTheme, and installed components

fn main() {
    let button = SimpleButton::new("Click me").build(&theme);
}
```

### 4. Check for updates

```bash
cargo iu_ui check
```

---

## Repository

- GitHub: [eto0202/michiu](https://github.com/eto0202/michiu)
- License: MIT OR Apache-2.0
