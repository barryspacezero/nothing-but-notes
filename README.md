# Nothing But Notes

A sleek, lightweight desktop notes and checklist widget engineered in Rust with a Nothing OS hardware aesthetic.

Features a draggable minimized notch ("dynamic island") that docks to any screen edge, dot-matrix typography, interactive markdown-backed checklists, and instant global hotkey access.

---

## ✨ Features

- **Nothing OS Visual Identity**: Stark monochrome styling, signature Nothing Red accents, dot-matrix headers ([Doto](https://fonts.google.com/specimen/Doto)), and Space Mono timestamps.
- **Edge Docking & Morphing Notch**: Docks seamlessly to the **Top**, **Bottom**, **Left**, or **Right** screen edge. Click to expand; drag and drop to snap to any border.
- **Global Hotkey**: Press <kbd>Ctrl</kbd> + <kbd>Alt</kbd> + <kbd>N</kbd> from anywhere in Windows to toggle the widget.
- **Interactive Checklists**: Circular dot-checkboxes with completion tracking and live dot-matrix progress indicators.
- **Dynamic Title Scaling**: Title dynamically scales font size and stays strictly clipped inside designated bounds.
- **Keyboard-First Workflow**:
  - <kbd>Enter</kbd> to add a new task item.
  - <kbd>Backspace</kbd> on empty items to delete and focus previous task.
  - <kbd>↑</kbd> / <kbd>↓</kbd> to navigate items.
  - <kbd>Alt</kbd> + <kbd>↑</kbd> / <kbd>↓</kbd> to reorder tasks.
  - <kbd>Ctrl</kbd> + <kbd>Enter</kbd> to toggle completion.
- **Smart Clipboard Capture**: Background clipboard listener automatically saves copied text snippets into quick notes with echo suppression.
- **Note Organization**: Pin critical notes to top, instant zero-allocation case-insensitive search, and rich right-click context menus.
- **Native & Ultra-Lightweight**: Built with `eframe` (egui) in native Rust — zero Electron overhead, low memory footprint, and atomic safe saves.
- **Windows Startup**: Optional launch on Windows startup via native registry integration.

---

## 🛠️ Building & Running

Ensure you have Rust and Cargo installed:

```bash
# Run locally in release mode
cargo run --release

# Build optimized binary
cargo build --release
```

The resulting executable will be located at `target/release/nothing-notes.exe`.

---

## 📄 License

MIT / Apache-2.0
