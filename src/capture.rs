use scap::frame::RGBFrame;
use rayon::prelude::*;

/// Calculates the average color of a specified region of the frame, ignoring black pixels.
pub fn calculate_average_color(frame: &RGBFrame, position: &str) -> [u32; 3] {
    let (x_start, x_end, y_start, y_end) = match position {
        "top" => (0, frame.width, 0, frame.height / 3),
        "bottom" => (0, frame.width, 2 * frame.height / 3, frame.height),
        "left" => (0, frame.width / 3, 0, frame.height),
        "right" => (2 * frame.width / 3, frame.width, 0, frame.height),
        _ => (0, frame.width, 0, frame.height), // Default is full frame
    };

    let (color_sum, count) = (y_start..y_end).into_par_iter().map(|y| {
        let mut local_color_sum = (0u64, 0u64, 0u64); // Local sums for R, G, B
        let mut local_count = 0u64; // Local pixel count for averaging

        for x in x_start..x_end {
            let index = (y * frame.width + x) as usize * 3; // Calculate buffer index (RGB)
            if index + 2 < frame.data.len() { // Ensure index does not exceed buffer length
                let r = frame.data[index];
                let g = frame.data[index + 1];
                let b = frame.data[index + 2];

                // Check if the pixel is black; if not, include it in the averaging
                if !(r == 0 && g == 0 && b == 0) {
                    local_color_sum.0 += r as u64; // Red
                    local_color_sum.1 += g as u64; // Green
                    local_color_sum.2 += b as u64; // Blue
                    local_count += 1; // Increase count for each pixel processed
                }
            }
        }

        (local_color_sum, local_count)
    }).reduce(|| ((0u64, 0u64, 0u64), 0u64), |acc, local| {
        (
            (acc.0 .0 + local.0 .0, acc.0 .1 + local.0 .1, acc.0 .2 + local.0 .2),
            acc.1 + local.1,
        )
    });

    if count == 0 { // Avoid division by zero and handle case where all pixels might be black
        return [0, 0, 0];
    }

    // Calculate averages for each color
    [
        (color_sum.0 / count) as u32, // Average Red
        (color_sum.1 / count) as u32, // Average Green
        (color_sum.2 / count) as u32, // Average Blue
    ]
}

/// Smooths the color transitions between frames.
pub fn smooth_colors(current_avg: (u32, u32, u32), new_avg: [u32; 3], smoothing_factor: f32) -> (u32, u32, u32) {
    let (current_r, current_g, current_b) = current_avg;
    let (new_r, new_g, new_b) = (new_avg[0], new_avg[1], new_avg[2]);

    let sm_r = (smoothing_factor * current_r as f32 + (1.0 - smoothing_factor) * new_r as f32) as u32;
    let sm_g = (smoothing_factor * current_g as f32 + (1.0 - smoothing_factor) * new_g as f32) as u32;
    let sm_b = (smoothing_factor * current_b as f32 + (1.0 - smoothing_factor) * new_b as f32) as u32;

    (sm_r, sm_g, sm_b)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::iter;

    // Helper function to create a mock frame
    fn mock_frame(width: i32, height: i32, fill: u8) -> RGBFrame {
        let size = (width * height * 3) as usize; // RGB
        let data: Vec<u8> = iter::repeat(fill).take(size).collect();
        RGBFrame {
            display_time: 0,
            width,
            height,
            data,
        }
    }

    #[test]
    fn test_calculate_average_color() {
        let frame = mock_frame(100, 100, 255);  // Create a white frame
        let avg_color = calculate_average_color(&frame, "top");
        assert_eq!(avg_color, [255, 255, 255]);

        let frame = mock_frame(100, 100, 0);  // Create a black frame
        let avg_color = calculate_average_color(&frame, "bottom");
        assert_eq!(avg_color, [0, 0, 0]);

        // Test with skipping pixels
        let avg_color = calculate_average_color(&frame, "left");
        assert_eq!(avg_color, [0, 0, 0]);
    }

    #[test]
    fn test_smooth_colors() {
        let current_colors = (100, 150, 200);
        let new_colors = [110, 160, 210];
        let smoothed = smooth_colors(current_colors, new_colors, 0.5);
        assert_eq!(smoothed, (105, 155, 205));

        let smoothed = smooth_colors(current_colors, new_colors, 0.0);
        assert_eq!(smoothed, (110, 160, 210));

        let smoothed = smooth_colors(current_colors, new_colors, 1.0);
        assert_eq!(smoothed, (100, 150, 200));
    }
}
