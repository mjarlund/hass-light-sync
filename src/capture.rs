use captrs::Capturer;
use crate::models::Frame;

/// Captures a single frame from the screen capturer.
pub fn capture_frame(capturer: &mut Capturer) -> Option<Frame> {
    let (width, height) = capturer.geometry();  // Get the dimensions directly from capturer
    capturer.capture_frame().ok().map(|frame| {
        Frame {
            width: width as u32,
            height: height as u32,
            buffer: frame.into_iter().flat_map(|pixel| vec![pixel.r, pixel.g, pixel.b]).collect(),
        }
    })
}

/// Calculates the average color of a specified region of the frame.
pub fn calculate_average_color(frame: &Frame, position: &str, skip_pixels: i16) -> Vec<u32> {
    let (x_start, x_end, y_start, y_end) = match position {
        "top" => (0, frame.width, 0, frame.height / 3),
        "bottom" => (0, frame.width, 2 * frame.height / 3, frame.height),
        "left" => (0, frame.width / 3, 0, frame.height),
        "right" => (2 * frame.width / 3, frame.width, 0, frame.height),
        _ => (0, frame.width, 0, frame.height),
    };

    let mut color_sum = (0u64, 0u64, 0u64);
    let mut count = 0u64;
    for y in (y_start..y_end).step_by(skip_pixels as usize) {
        for x in (x_start..x_end).step_by(3 * skip_pixels as usize) {
            let offset = (y * frame.width + x) * 3;
            color_sum.0 += frame.buffer[offset as usize] as u64;
            color_sum.1 += frame.buffer[offset as usize + 1] as u64;
            color_sum.2 += frame.buffer[offset as usize + 2] as u64;
            count += 1;
        }
    }

    vec![(color_sum.0 / count) as u32, (color_sum.1 / count) as u32, (color_sum.2 / count) as u32]
}
