use std::ffi::{CStr, c_char, c_int};

use serde::Deserialize;

const BYTES_PER_PIXEL: usize = 4;

/// Mirror plugin parameters.
#[derive(Deserialize)]
struct MirrorParams {
    /// Flip horizontally (left to right).
    #[serde(default)]
    horizontal: bool,
    /// Flip vertically (top to bottom).
    #[serde(default)]
    vertical: bool,
}

/// Plugin entry point — exported with C-compatible ABI.
///
/// Returns 0 on success, non-zero on error.
///
/// # Safety
///
/// - `rgba_data` must point to a valid buffer of size
///   `width * height * 4` bytes.
/// - `params` must be a valid pointer to a null-terminated
///   C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn process_image(
    width: u32,
    height: u32,
    rgba_data: *mut u8,
    params: *const c_char,
) -> c_int {
    if rgba_data.is_null() || params.is_null() {
        return 1;
    }

    let Some(w) = usize::try_from(width).ok().filter(|&v| v > 0) else {
        return 2;
    };
    let Some(h) = usize::try_from(height).ok().filter(|&v| v > 0) else {
        return 2;
    };
    let Some(buf_len) = w
        .checked_mul(h)
        .and_then(|v| v.checked_mul(BYTES_PER_PIXEL))
    else {
        return 3;
    };

    // SAFETY: we verified that rgba_data is non-null and
    // buf_len does not overflow. The actual buffer size
    // behind the pointer is guaranteed by the caller
    // (the host application).
    let data = unsafe { std::slice::from_raw_parts_mut(rgba_data, buf_len) };

    // SAFETY: we verified that params is non-null.
    // The caller guarantees it points to a valid
    // null-terminated C string.
    let params_str = unsafe { CStr::from_ptr(params) }.to_str().unwrap_or("");

    let Ok(mirror_params) = serde_json::from_str::<MirrorParams>(params_str)
    else {
        return 4;
    };

    if mirror_params.horizontal {
        flip_horizontal(data, w, h);
    }
    if mirror_params.vertical {
        flip_vertical(data, w, h);
    }

    0
}

/// Flips the image horizontally — swaps pixels in each row
/// (left <-> right).
fn flip_horizontal(data: &mut [u8], width: usize, height: usize) {
    let row_bytes = width * BYTES_PER_PIXEL;

    for y in 0..height {
        let row_start = y * row_bytes;
        for x in 0..width / 2 {
            let left = row_start + x * BYTES_PER_PIXEL;
            let right = row_start + (width - 1 - x) * BYTES_PER_PIXEL;

            for i in 0..BYTES_PER_PIXEL {
                data.swap(left + i, right + i);
            }
        }
    }
}

/// Flips the image vertically — swaps rows
/// (top <-> bottom).
fn flip_vertical(data: &mut [u8], width: usize, height: usize) {
    let row_bytes = width * BYTES_PER_PIXEL;

    for y in 0..height / 2 {
        let top_start = y * row_bytes;
        let bottom_start = (height - 1 - y) * row_bytes;

        for i in 0..row_bytes {
            data.swap(top_start + i, bottom_start + i);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Creates a 2x2 test image with unique colors per pixel.
    /// Format: 2x2, each pixel = 4 bytes RGBA.
    fn make_2x2() -> Vec<u8> {
        vec![
            255, 0, 0, 255, // top-left — red
            0, 255, 0, 255, // top-right — green
            0, 0, 255, 255, // bottom-left — blue
            255, 255, 255, 255, // bottom-right — white
        ]
    }

    #[test]
    fn horizontal_flip_2x2() {
        let mut data = make_2x2();
        flip_horizontal(&mut data, 2, 2);

        // After horizontal flip:
        // green, red
        // white, blue
        assert_eq!(
            data,
            vec![
                0, 255, 0, 255, // green
                255, 0, 0, 255, // red
                255, 255, 255, 255, // white
                0, 0, 255, 255, // blue
            ]
        );
    }

    #[test]
    fn vertical_flip_2x2() {
        let mut data = make_2x2();
        flip_vertical(&mut data, 2, 2);

        // After vertical flip:
        // blue, white
        // red, green
        assert_eq!(
            data,
            vec![
                0, 0, 255, 255, // blue
                255, 255, 255, 255, // white
                255, 0, 0, 255, // red
                0, 255, 0, 255, // green
            ]
        );
    }

    #[test]
    fn both_flips_2x2() {
        let mut data = make_2x2();
        flip_horizontal(&mut data, 2, 2);
        flip_vertical(&mut data, 2, 2);

        // Horizontal + vertical = 180° rotation:
        // white, blue
        // green, red
        assert_eq!(
            data,
            vec![
                255, 255, 255, 255, // white
                0, 0, 255, 255, // blue
                0, 255, 0, 255, // green
                255, 0, 0, 255, // red
            ]
        );
    }

    #[test]
    fn horizontal_flip_single_column() {
        // 1x3 image — horizontal flip changes nothing
        let mut data = vec![
            1, 2, 3, 4, //
            5, 6, 7, 8, //
            9, 10, 11, 12, //
        ];
        let original = data.clone();
        flip_horizontal(&mut data, 1, 3);
        assert_eq!(data, original);
    }

    #[test]
    fn vertical_flip_single_row() {
        // 3x1 image — vertical flip changes nothing
        let mut data = vec![
            1, 2, 3, 4, //
            5, 6, 7, 8, //
            9, 10, 11, 12, //
        ];
        let original = data.clone();
        flip_vertical(&mut data, 3, 1);
        assert_eq!(data, original);
    }

    mod proptests {
        use super::*;
        use proptest::prelude::*;

        /// Generates a random RGBA image with dimensions
        /// in range [1, 64] and random pixel data.
        fn arbitrary_image() -> impl Strategy<Value = (usize, usize, Vec<u8>)> {
            (1..=64usize, 1..=64usize).prop_flat_map(|(w, h)| {
                let len = w * h * BYTES_PER_PIXEL;
                (Just(w), Just(h), proptest::collection::vec(any::<u8>(), len))
            })
        }

        proptest! {
            #[test]
            fn double_horizontal_flip_is_identity(
                (w, h, mut data) in arbitrary_image()
            ) {
                let original = data.clone();
                flip_horizontal(&mut data, w, h);
                flip_horizontal(&mut data, w, h);
                prop_assert_eq!(data, original);
            }

            #[test]
            fn double_vertical_flip_is_identity(
                (w, h, mut data) in arbitrary_image()
            ) {
                let original = data.clone();
                flip_vertical(&mut data, w, h);
                flip_vertical(&mut data, w, h);
                prop_assert_eq!(data, original);
            }
        }
    }
}
