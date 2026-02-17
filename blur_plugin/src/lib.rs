use std::ffi::{CStr, c_char};

use serde::Deserialize;

const BYTES_PER_PIXEL: usize = 4;

/// Blur plugin parameters.
#[derive(Deserialize)]
#[serde(default)]
struct BlurParams {
    /// Blur radius in pixels.
    radius: u32,
    /// Number of blur iterations.
    iterations: u32,
}

impl Default for BlurParams {
    fn default() -> Self {
        Self {
            radius: 1,
            iterations: 1,
        }
    }
}

/// Plugin entry point — exported with C-compatible ABI.
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
) {
    if rgba_data.is_null() || params.is_null() {
        return;
    }

    let Some(w) = usize::try_from(width).ok().filter(|&v| v > 0) else {
        return;
    };
    let Some(h) = usize::try_from(height).ok().filter(|&v| v > 0) else {
        return;
    };
    let Some(buf_len) = w
        .checked_mul(h)
        .and_then(|v| v.checked_mul(BYTES_PER_PIXEL))
    else {
        return;
    };

    // SAFETY: we verified that rgba_data is non-null and buf_len does not overflow.
    // The actual buffer size behind the pointer is guaranteed by the caller (the host application).
    let data = unsafe { std::slice::from_raw_parts_mut(rgba_data, buf_len) };

    // SAFETY: we verified that params is non-null.
    // The caller guarantees it points to a valid null-terminated C string.
    let params_str = unsafe { CStr::from_ptr(params) }.to_str().unwrap_or("");

    let blur_params: BlurParams =
        serde_json::from_str(params_str).unwrap_or_default();

    weighted_blur(
        data,
        w,
        h,
        usize::try_from(blur_params.radius).unwrap_or(0),
        blur_params.iterations,
    );
}

/// Applies weighted blur to an RGBA buffer.
///
/// For each pixel, computes a weighted average of all pixels
/// within a square of side `2 * radius + 1`.
/// Weight = `1.0 / max(1.0, distance)`, so the center pixel
/// has weight 1.0.
///
/// Uses a temporary buffer to avoid reading already-modified
/// data.
fn weighted_blur(
    data: &mut [u8],
    width: usize,
    height: usize,
    radius: usize,
    iterations: u32,
) {
    let mut temp = vec![0u8; data.len()];

    for _ in 0..iterations {
        for y in 0..height {
            for x in 0..width {
                let (sr, sg, sb, sa, tw) =
                    accumulate_neighborhood(data, width, height, x, y, radius);

                let dst = (y * width + x) * BYTES_PER_PIXEL;

                // Values are guaranteed non-negative (sums of non-negative products),
                // and division by total_weight keeps them within [0, 255].
                #[allow(
                    clippy::cast_possible_truncation,
                    clippy::cast_sign_loss
                )]
                {
                    temp[dst] = (sr / tw).round() as u8;
                    temp[dst + 1] = (sg / tw).round() as u8;
                    temp[dst + 2] = (sb / tw).round() as u8;
                    temp[dst + 3] = (sa / tw).round() as u8;
                }
            }
        }
        data.copy_from_slice(&temp);
    }
}

/// Accumulates weighted channel values of all neighboring
/// pixels within radius `r` of pixel `(center_x, center_y)`.
///
/// Returns `(sum_r, sum_g, sum_b, sum_a, total_weight)`.
fn accumulate_neighborhood(
    data: &[u8],
    width: usize,
    height: usize,
    center_x: usize,
    center_y: usize,
    radius: usize,
) -> (f64, f64, f64, f64, f64) {
    let mut sum_r = 0.0_f64;
    let mut sum_g = 0.0_f64;
    let mut sum_b = 0.0_f64;
    let mut sum_a = 0.0_f64;
    let mut total_weight = 0.0_f64;

    let y_start = center_y.saturating_sub(radius);
    let y_end = (center_y + radius + 1).min(height);
    let x_start = center_x.saturating_sub(radius);
    let x_end = (center_x + radius + 1).min(width);

    for ny in y_start..y_end {
        for nx in x_start..x_end {
            let dx = center_x.abs_diff(nx);
            let dy = center_y.abs_diff(ny);

            #[allow(clippy::cast_precision_loss)]
            let distance = ((dx * dx + dy * dy) as f64).sqrt();
            let weight = 1.0 / distance.max(1.0);

            let src = (ny * width + nx) * BYTES_PER_PIXEL;

            sum_r += f64::from(data[src]) * weight;
            sum_g += f64::from(data[src + 1]) * weight;
            sum_b += f64::from(data[src + 2]) * weight;
            sum_a += f64::from(data[src + 3]) * weight;
            total_weight += weight;
        }
    }

    (sum_r, sum_g, sum_b, sum_a, total_weight)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blur_radius_zero_is_identity() {
        let mut data = vec![
            255, 0, 0, 255, // red
            0, 255, 0, 255, // green
            0, 0, 255, 255, // blue
            255, 255, 0, 255, // yellow
        ];
        let original = data.clone();
        weighted_blur(&mut data, 2, 2, 0, 1);
        assert_eq!(data, original);
    }

    #[test]
    fn blur_single_pixel() {
        let mut data = vec![100, 150, 200, 255];
        let original = data.clone();
        weighted_blur(&mut data, 1, 1, 5, 3);
        assert_eq!(data, original);
    }

    #[test]
    fn blur_uniform_image_unchanged() {
        let pixel = [128u8, 128, 128, 255];
        let mut data: Vec<u8> =
            pixel.iter().copied().cycle().take(9 * 4).collect();
        let original = data.clone();
        weighted_blur(&mut data, 3, 3, 1, 1);
        assert_eq!(data, original);
    }

    #[test]
    fn blur_reduces_contrast() {
        // 3x3 image: center is white, rest are black
        let mut data = vec![0u8; 3 * 3 * BYTES_PER_PIXEL];
        for i in 0..9 {
            data[i * BYTES_PER_PIXEL + 3] = 255;
        }
        // Center pixel (1,1) = white
        let center = 4 * BYTES_PER_PIXEL;
        data[center] = 255;
        data[center + 1] = 255;
        data[center + 2] = 255;

        weighted_blur(&mut data, 3, 3, 1, 1);

        // Center pixel should darken (< 255)
        let center_r = data[center];
        assert!(
            center_r < 255,
            "Center pixel should darken after blur, \
             but R={center_r}"
        );

        // Neighbor pixel (0,1) should brighten (> 0)
        let neighbor = BYTES_PER_PIXEL;
        assert!(data[neighbor] > 0, "Neighbor pixel should brighten");
    }

    #[test]
    fn blur_multiple_iterations() {
        let make_data = || {
            let mut d = vec![0u8; 5 * 5 * BYTES_PER_PIXEL];
            for i in 0..25 {
                d[i * BYTES_PER_PIXEL + 3] = 255;
            }
            // Center pixel (2,2) = white
            let c = 12 * BYTES_PER_PIXEL;
            d[c] = 255;
            d[c + 1] = 255;
            d[c + 2] = 255;
            d
        };

        let mut data1 = make_data();
        weighted_blur(&mut data1, 5, 5, 1, 1);
        let center1 = data1[12 * BYTES_PER_PIXEL];

        let mut data2 = make_data();
        weighted_blur(&mut data2, 5, 5, 1, 3);
        let center2 = data2[12 * BYTES_PER_PIXEL];

        assert!(
            center2 < center1,
            "More iterations should produce stronger blur: \
             1 iter R={center1}, 3 iter R={center2}"
        );
    }

    mod proptests {
        use super::*;
        use proptest::prelude::*;

        /// Generates a random RGBA image with dimensions
        /// in range [1, 32] and random pixel data.
        fn arbitrary_image()
        -> impl Strategy<Value = (usize, usize, Vec<u8>)>
        {
            (1..=32usize, 1..=32usize).prop_flat_map(
                |(w, h)| {
                    let len = w * h * BYTES_PER_PIXEL;
                    (
                        Just(w),
                        Just(h),
                        proptest::collection::vec(
                            any::<u8>(),
                            len,
                        ),
                    )
                },
            )
        }

        proptest! {
            #[test]
            fn radius_zero_is_identity(
                (w, h, mut data) in arbitrary_image()
            ) {
                let original = data.clone();
                weighted_blur(&mut data, w, h, 0, 1);
                prop_assert_eq!(data, original);
            }

            #[test]
            fn uniform_image_unchanged(
                w in 1..=16usize,
                h in 1..=16usize,
                pixel in any::<[u8; 4]>(),
                radius in 1..=5usize,
            ) {
                let mut data: Vec<u8> = pixel
                    .iter()
                    .copied()
                    .cycle()
                    .take(w * h * BYTES_PER_PIXEL)
                    .collect();
                let original = data.clone();
                weighted_blur(&mut data, w, h, radius, 1);
                prop_assert_eq!(data, original);
            }
        }
    }
}
