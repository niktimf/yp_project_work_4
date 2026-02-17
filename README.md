# Image FFI Plugin System

A CLI application that loads a PNG image, applies a dynamically loaded processing plugin, and saves the result.

## Project Structure

```
├── Cargo.toml              # Workspace root
├── image_processor/        # Main binary crate
│   ├── src/
│   │   ├── main.rs         # CLI entry point
│   │   ├── lib.rs          # Module re-exports
│   │   ├── error.rs        # Error types (thiserror)
│   │   └── plugin_loader.rs# Dynamic library loading (libloading)
│   └── tests/
│       └── integration.rs  # End-to-end tests
├── mirror_plugin/          # Mirror flip plugin (cdylib)
│   └── src/lib.rs
└── blur_plugin/            # Weighted blur plugin (cdylib)
    └── src/lib.rs
```

## Building

```bash
cargo build --workspace
```

## Usage

```bash
cargo run -- \
  --input photo.png \
  --output result.png \
  --plugin mirror_plugin \
  --params params.json \
  --plugin-path target/debug
```

### Arguments

| Argument        | Description                              | Default        |
|-----------------|------------------------------------------|----------------|
| `--input`       | Path to the input PNG image              | required       |
| `--output`      | Path to save the processed image         | required       |
| `--plugin`      | Plugin name without extension            | required       |
| `--params`      | Path to a JSON file with parameters      | required       |
| `--plugin-path` | Directory containing plugin libraries    | `target/debug` |

### Debug logging

```bash
RUST_LOG=debug cargo run -- --input photo.png --output result.png --plugin mirror_plugin --params params.json
```

## Plugins

### mirror_plugin

Flips the image horizontally and/or vertically.

**params.json:**
```json
{"horizontal": true, "vertical": false}
```

### blur_plugin

Applies weighted blur with configurable radius and iterations.

**params.json:**
```json
{"radius": 3, "iterations": 2}
```

## Plugin API

All plugins export a single C function:

```c
void process_image(
    uint32_t width,
    uint32_t height,
    uint8_t* rgba_data,
    const char* params
);
```

Plugins are compiled as `cdylib` and modify the RGBA buffer in-place.

## Running Tests

```bash
cargo test --workspace
```