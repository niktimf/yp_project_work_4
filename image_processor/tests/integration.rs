use std::fs;
use std::path::{Path, PathBuf};

use assert_cmd::cargo::cargo_bin_cmd;
use image::{ImageReader, Rgba, RgbaImage};
use tempfile::TempDir;

/// Creates a 4x4 test image with a known pattern:
/// top-left quadrant is red, rest is blue.
fn create_test_image(path: &Path) {
    let mut img = RgbaImage::new(4, 4);
    for y in 0..4u32 {
        for x in 0..4u32 {
            let color = if x < 2 && y < 2 {
                Rgba([255, 0, 0, 255]) // red
            } else {
                Rgba([0, 0, 255, 255]) // blue
            };
            img.put_pixel(x, y, color);
        }
    }
    img.save(path).expect("failed to save test image");
}

/// Returns the absolute path to the built plugin directory.
/// `CARGO_MANIFEST_DIR` points to `image_processor/`,
/// so the workspace `target/debug/` is one level up.
fn plugin_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../target/debug")
}

/// Helper: runs image_processor with given args,
/// asserts success, and returns the output image.
fn run_and_load(
    input: &Path,
    output: &Path,
    plugin: &str,
    params_path: &Path,
) -> RgbaImage {
    cargo_bin_cmd!("image_processor")
        .arg("--input")
        .arg(input)
        .arg("--output")
        .arg(output)
        .arg("--plugin")
        .arg(plugin)
        .arg("--params")
        .arg(params_path)
        .arg("--plugin-path")
        .arg(&plugin_dir())
        .assert()
        .success();

    ImageReader::open(output)
        .expect("failed to open output")
        .decode()
        .expect("failed to decode output")
        .into_rgba8()
}

#[test]
fn mirror_horizontal_flips_pixels() {
    let dir = TempDir::new().unwrap();
    let input = dir.path().join("input.png");
    let output = dir.path().join("output.png");
    let params = dir.path().join("params.json");

    create_test_image(&input);
    fs::write(&params, r#"{"horizontal": true}"#).unwrap();

    let result = run_and_load(&input, &output, "mirror_plugin", &params);

    // After horizontal flip of a 4x4 image:
    // top-right quadrant should now be red (was top-left)
    let top_right = result.get_pixel(3, 0);
    assert_eq!(top_right, &Rgba([255, 0, 0, 255]));

    // top-left should now be blue (was top-right area)
    let top_left = result.get_pixel(0, 0);
    assert_eq!(top_left, &Rgba([0, 0, 255, 255]));
}

#[test]
fn mirror_vertical_flips_pixels() {
    let dir = TempDir::new().unwrap();
    let input = dir.path().join("input.png");
    let output = dir.path().join("output.png");
    let params = dir.path().join("params.json");

    create_test_image(&input);
    fs::write(&params, r#"{"vertical": true}"#).unwrap();

    let result = run_and_load(&input, &output, "mirror_plugin", &params);

    // After vertical flip:
    // bottom-left quadrant should now be red (was top-left)
    let bottom_left = result.get_pixel(0, 3);
    assert_eq!(bottom_left, &Rgba([255, 0, 0, 255]));

    // top-left should now be blue
    let top_left = result.get_pixel(0, 0);
    assert_eq!(top_left, &Rgba([0, 0, 255, 255]));
}

#[test]
fn blur_modifies_image() {
    let dir = TempDir::new().unwrap();
    let input = dir.path().join("input.png");
    let output = dir.path().join("output.png");
    let params = dir.path().join("params.json");

    create_test_image(&input);
    fs::write(&params, r#"{"radius": 1, "iterations": 1}"#).unwrap();

    let original = ImageReader::open(&input)
        .unwrap()
        .decode()
        .unwrap()
        .into_rgba8();

    let result = run_and_load(&input, &output, "blur_plugin", &params);

    // Blurred image should differ from original
    // (boundary between red and blue areas gets mixed)
    assert_ne!(original, result);
}

#[test]
fn missing_input_file_returns_error() {
    let dir = TempDir::new().unwrap();
    let params = dir.path().join("params.json");
    fs::write(&params, "{}").unwrap();

    cargo_bin_cmd!("image_processor")
        .arg("--input")
        .arg("nonexistent.png")
        .arg("--output")
        .arg(dir.path().join("out.png"))
        .arg("--plugin")
        .arg("mirror_plugin")
        .arg("--params")
        .arg(&params)
        .arg("--plugin-path")
        .arg(&plugin_dir())
        .assert()
        .failure();
}

#[test]
fn missing_plugin_returns_error() {
    let dir = TempDir::new().unwrap();
    let input = dir.path().join("input.png");
    let params = dir.path().join("params.json");

    create_test_image(&input);
    fs::write(&params, "{}").unwrap();

    cargo_bin_cmd!("image_processor")
        .arg("--input")
        .arg(&input)
        .arg("--output")
        .arg(dir.path().join("out.png"))
        .arg("--plugin")
        .arg("nonexistent_plugin")
        .arg("--params")
        .arg(&params)
        .arg("--plugin-path")
        .arg(&plugin_dir())
        .assert()
        .failure();
}

#[test]
fn missing_params_file_returns_error() {
    let dir = TempDir::new().unwrap();
    let input = dir.path().join("input.png");

    create_test_image(&input);

    cargo_bin_cmd!("image_processor")
        .arg("--input")
        .arg(&input)
        .arg("--output")
        .arg(dir.path().join("out.png"))
        .arg("--plugin")
        .arg("mirror_plugin")
        .arg("--params")
        .arg("nonexistent_params.json")
        .arg("--plugin-path")
        .arg(&plugin_dir())
        .assert()
        .failure();
}
