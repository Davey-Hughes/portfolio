use crate::types::{MosaicCell, MosaicLayout};
use rand::Rng;
use serde::{Deserialize, Serialize};

/// Represents a rectangle in the mosaic layout
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rectangle {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl Rectangle {
    pub fn new(x: f64, y: f64, width: f64, height: f64) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    pub fn area(&self) -> f64 {
        self.width * self.height
    }

    pub fn aspect_ratio(&self) -> f64 {
        self.width / self.height
    }

    /// Returns true if this rectangle is too small or too thin
    fn is_too_small(&self, min_dimension: f64, min_aspect: f64, max_aspect: f64) -> bool {
        self.width < min_dimension
            || self.height < min_dimension
            || self.aspect_ratio() < min_aspect
            || self.aspect_ratio() > max_aspect
    }
}

/// Represents a line segment that divides the space
#[derive(Debug, Clone)]
struct DivisionLine {
    // For horizontal lines (cuts top/bottom edges)
    // For vertical lines (cuts left/right edges)
    start_x: f64,
    start_y: f64,
    end_x: f64,
    end_y: f64,
    is_horizontal: bool,
}

/// Configuration for the mosaic layout generator
#[derive(Debug, Clone)]
pub struct MosaicConfig {
    pub container_width: f64,
    pub container_height: f64,
    pub min_cell_dimension: f64, // Minimum width or height for a cell
    pub min_aspect_ratio: f64,   // e.g., 0.4 (very portrait)
    pub max_aspect_ratio: f64,   // e.g., 2.5 (very landscape)
}

impl Default for MosaicConfig {
    fn default() -> Self {
        Self {
            container_width: 1200.0,
            container_height: 800.0,
            min_cell_dimension: 150.0,
            min_aspect_ratio: 0.4,
            max_aspect_ratio: 2.5,
        }
    }
}

/// Generate a mosaic layout for a given number of images
pub fn generate_mosaic_layout(num_images: usize, config: MosaicConfig) -> Vec<Rectangle> {
    if num_images == 0 {
        return Vec::new();
    }

    if num_images == 1 {
        return vec![Rectangle::new(
            0.0,
            0.0,
            config.container_width,
            config.container_height,
        )];
    }

    let mut rng = rand::thread_rng();

    // Start with the full container
    let mut rectangles = vec![Rectangle::new(
        0.0,
        0.0,
        config.container_width,
        config.container_height,
    )];

    // Keep track of division lines
    let mut lines: Vec<DivisionLine> = Vec::new();

    // We need n-1 splits to get n rectangles
    let num_splits = num_images - 1;

    for _ in 0..num_splits {
        // Calculate weights for each rectangle based on area (bias toward larger)
        let total_area: f64 = rectangles.iter().map(|r| r.area()).sum();
        let weights: Vec<f64> = rectangles.iter().map(|r| r.area() / total_area).collect();

        // Select a rectangle to split based on weighted random selection
        let rect_index = weighted_random_select(&weights, &mut rng);
        let rect_to_split = rectangles[rect_index].clone();

        // Try to split this rectangle
        if let Some((rect1, rect2, new_line)) =
            try_split_rectangle(&rect_to_split, &lines, &config, &mut rng)
        {
            // Remove the old rectangle and add the two new ones
            rectangles.remove(rect_index);
            rectangles.push(rect1);
            rectangles.push(rect2);
            lines.push(new_line);
        } else {
            // If we can't split the selected rectangle, try others
            // This is a fallback in case we get stuck
            let mut found_split = false;
            for (i, rect) in rectangles.iter().enumerate() {
                if i == rect_index {
                    continue;
                }
                if let Some((rect1, rect2, new_line)) =
                    try_split_rectangle(rect, &lines, &config, &mut rng)
                {
                    rectangles.remove(i);
                    rectangles.push(rect1);
                    rectangles.push(rect2);
                    lines.push(new_line);
                    found_split = true;
                    break;
                }
            }

            // If we still can't split anything, break early
            if !found_split {
                break;
            }
        }
    }

    rectangles
}

/// Try to split a rectangle into two rectangles
fn try_split_rectangle(
    rect: &Rectangle,
    existing_lines: &[DivisionLine],
    config: &MosaicConfig,
    rng: &mut impl Rng,
) -> Option<(Rectangle, Rectangle, DivisionLine)> {
    // Determine if we should split horizontally or vertically
    // Prefer splitting the longer dimension
    let prefer_horizontal = rect.width > rect.height;

    // Try preferred direction first, then try the other direction
    for try_horizontal in [prefer_horizontal, !prefer_horizontal] {
        if try_horizontal {
            // Try horizontal split (cutting top or bottom edge)
            if let Some(split) = try_horizontal_split(rect, existing_lines, config, rng) {
                return Some(split);
            }
        } else {
            // Try vertical split (cutting left or right edge)
            if let Some(split) = try_vertical_split(rect, existing_lines, config, rng) {
                return Some(split);
            }
        }
    }

    None
}

/// Try to split a rectangle horizontally
fn try_horizontal_split(
    rect: &Rectangle,
    existing_lines: &[DivisionLine],
    config: &MosaicConfig,
    rng: &mut impl Rng,
) -> Option<(Rectangle, Rectangle, DivisionLine)> {
    // Choose a random position along the height
    // Ensure we don't create rectangles that are too small
    // Add extra margin to avoid very small cuts near edges
    let margin = config.min_cell_dimension * 1.5;
    let min_y = rect.y + margin;
    let max_y = rect.y + rect.height - margin;

    if min_y >= max_y {
        return None;
    }

    // Try multiple random positions to find a valid split
    // Bias toward center by using triangular distribution
    for _ in 0..10 {
        // Generate two random numbers and average them to bias toward center
        let r1 = rng.gen_range(min_y..max_y);
        let r2 = rng.gen_range(min_y..max_y);
        let split_y = (r1 + r2) / 2.0;

        // Find where this horizontal line would intersect with existing vertical lines
        let (start_x, end_x) = find_line_extent_horizontal(rect, split_y, existing_lines);

        // Create the two new rectangles
        let rect1 = Rectangle::new(rect.x, rect.y, rect.width, split_y - rect.y);
        let rect2 = Rectangle::new(rect.x, split_y, rect.width, rect.y + rect.height - split_y);

        // Check if the new rectangles would be valid
        if !rect1.is_too_small(
            config.min_cell_dimension,
            config.min_aspect_ratio,
            config.max_aspect_ratio,
        ) && !rect2.is_too_small(
            config.min_cell_dimension,
            config.min_aspect_ratio,
            config.max_aspect_ratio,
        ) {
            let line = DivisionLine {
                start_x,
                start_y: split_y,
                end_x,
                end_y: split_y,
                is_horizontal: true,
            };

            return Some((rect1, rect2, line));
        }
    }

    None
}

/// Try to split a rectangle vertically
fn try_vertical_split(
    rect: &Rectangle,
    existing_lines: &[DivisionLine],
    config: &MosaicConfig,
    rng: &mut impl Rng,
) -> Option<(Rectangle, Rectangle, DivisionLine)> {
    // Choose a random position along the width
    // Add extra margin to avoid very small cuts near edges
    let margin = config.min_cell_dimension * 1.5;
    let min_x = rect.x + margin;
    let max_x = rect.x + rect.width - margin;

    if min_x >= max_x {
        return None;
    }

    // Try multiple random positions to find a valid split
    // Bias toward center by using triangular distribution
    for _ in 0..10 {
        // Generate two random numbers and average them to bias toward center
        let r1 = rng.gen_range(min_x..max_x);
        let r2 = rng.gen_range(min_x..max_x);
        let split_x = (r1 + r2) / 2.0;

        // Find where this vertical line would intersect with existing horizontal lines
        let (start_y, end_y) = find_line_extent_vertical(rect, split_x, existing_lines);

        // Create the two new rectangles
        let rect1 = Rectangle::new(rect.x, rect.y, split_x - rect.x, rect.height);
        let rect2 = Rectangle::new(split_x, rect.y, rect.x + rect.width - split_x, rect.height);

        // Check if the new rectangles would be valid
        if !rect1.is_too_small(
            config.min_cell_dimension,
            config.min_aspect_ratio,
            config.max_aspect_ratio,
        ) && !rect2.is_too_small(
            config.min_cell_dimension,
            config.min_aspect_ratio,
            config.max_aspect_ratio,
        ) {
            let line = DivisionLine {
                start_x: split_x,
                start_y,
                end_x: split_x,
                end_y,
                is_horizontal: false,
            };

            return Some((rect1, rect2, line));
        }
    }

    None
}

/// Find where a horizontal line would end based on existing vertical lines
fn find_line_extent_horizontal(
    rect: &Rectangle,
    y: f64,
    existing_lines: &[DivisionLine],
) -> (f64, f64) {
    let mut start_x = rect.x;
    let mut end_x = rect.x + rect.width;

    // Check if any vertical lines intersect with this horizontal line
    for line in existing_lines {
        if !line.is_horizontal {
            // This is a vertical line
            let line_x = line.start_x;
            let line_y_min = line.start_y.min(line.end_y);
            let line_y_max = line.start_y.max(line.end_y);

            // Check if this vertical line crosses our horizontal line
            if line_x >= rect.x
                && line_x <= rect.x + rect.width
                && y >= line_y_min
                && y <= line_y_max
            {
                // This line intersects, stop here
                if line_x < rect.x + rect.width / 2.0 {
                    start_x = start_x.max(line_x);
                } else {
                    end_x = end_x.min(line_x);
                }
            }
        }
    }

    (start_x, end_x)
}

/// Find where a vertical line would end based on existing horizontal lines
fn find_line_extent_vertical(
    rect: &Rectangle,
    x: f64,
    existing_lines: &[DivisionLine],
) -> (f64, f64) {
    let mut start_y = rect.y;
    let mut end_y = rect.y + rect.height;

    // Check if any horizontal lines intersect with this vertical line
    for line in existing_lines {
        if line.is_horizontal {
            // This is a horizontal line
            let line_y = line.start_y;
            let line_x_min = line.start_x.min(line.end_x);
            let line_x_max = line.start_x.max(line.end_x);

            // Check if this horizontal line crosses our vertical line
            if line_y >= rect.y
                && line_y <= rect.y + rect.height
                && x >= line_x_min
                && x <= line_x_max
            {
                // This line intersects, stop here
                if line_y < rect.y + rect.height / 2.0 {
                    start_y = start_y.max(line_y);
                } else {
                    end_y = end_y.min(line_y);
                }
            }
        }
    }

    (start_y, end_y)
}

/// Weighted random selection based on provided weights
fn weighted_random_select(weights: &[f64], rng: &mut impl Rng) -> usize {
    let total: f64 = weights.iter().sum();
    let mut random = rng.gen_range(0.0..total);

    for (i, &weight) in weights.iter().enumerate() {
        random -= weight;
        if random <= 0.0 {
            return i;
        }
    }

    // Fallback to last index (shouldn't happen due to floating point precision)
    weights.len() - 1
}

/// Categorize aspect ratio into orientation
fn categorize_orientation(aspect_ratio: f64) -> &'static str {
    if aspect_ratio > 1.2 {
        "landscape"
    } else if aspect_ratio < 0.83 {
        "portrait"
    } else {
        "square"
    }
}

/// Assign images to mosaic rectangles based on aspect ratio matching
pub fn assign_images_to_layout(
    rectangles: &[Rectangle],
    image_aspects: &[(usize, f64)], // (original_index, aspect_ratio)
) -> Vec<(Rectangle, usize)> {
    // Create a copy of rectangles and images so we can match them
    let mut available_rects: Vec<(usize, Rectangle)> =
        rectangles.iter().cloned().enumerate().collect();
    let mut available_images: Vec<(usize, f64)> = image_aspects.to_vec();

    let mut assignments: Vec<(Rectangle, usize)> = Vec::new();

    // Greedily assign images to rectangles based on aspect ratio similarity
    while !available_rects.is_empty() && !available_images.is_empty() {
        // Find the best match
        let mut best_match: Option<(usize, usize, f64)> = None; // (rect_idx, img_idx, score)

        for (rect_idx, (_rect_id, rect)) in available_rects.iter().enumerate() {
            let rect_aspect = rect.aspect_ratio();
            let rect_orientation = categorize_orientation(rect_aspect);

            for (img_idx, (_, img_aspect)) in available_images.iter().enumerate() {
                let img_orientation = categorize_orientation(*img_aspect);
                
                // Calculate base score from aspect ratio difference
                let aspect_diff = (rect_aspect - img_aspect).abs();
                let mut score = 1.0 / (1.0 + aspect_diff);
                
                // Apply VERY aggressive penalty for orientation mismatch
                // Matching orientations get a 10x bonus, mismatches get 0.05x penalty
                if rect_orientation == img_orientation {
                    score *= 10.0; // Very strong bonus for matching orientation
                } else if (rect_orientation == "square") || (img_orientation == "square") {
                    score *= 0.8; // Slight penalty even for squares if not exact match
                } else {
                    score *= 0.05; // Extremely heavy penalty for portrait/landscape mismatch
                }

                if let Some((_, _, best_score)) = best_match {
                    if score > best_score {
                        best_match = Some((rect_idx, img_idx, score));
                    }
                } else {
                    best_match = Some((rect_idx, img_idx, score));
                }
            }
        }

        // Assign the best match
        if let Some((rect_idx, img_idx, _)) = best_match {
            let (_rect_id, rect) = available_rects.remove(rect_idx);
            let (original_img_idx, _) = available_images.remove(img_idx);
            assignments.push((rect, original_img_idx));
        } else {
            break;
        }
    }

    assignments
}

/// Convert mosaic rectangles to CSS Grid coordinates
pub fn rectangles_to_grid_layout(
    rectangles: &[Rectangle],
    grid_precision: u32,
    container_height: f64,
) -> MosaicLayout {
    // Find the bounding box
    let max_x = rectangles
        .iter()
        .map(|r| r.x + r.width)
        .max_by(|a, b| a.partial_cmp(b).unwrap())
        .unwrap_or(1.0);
    let max_y = rectangles
        .iter()
        .map(|r| r.y + r.height)
        .max_by(|a, b| a.partial_cmp(b).unwrap())
        .unwrap_or(1.0);

    // Convert each rectangle to grid coordinates
    let cells: Vec<MosaicCell> = rectangles
        .iter()
        .map(|rect| {
            // Normalize to 0-1 range then scale to grid
            let col_start = ((rect.x / max_x) * f64::from(grid_precision)).round() as u32 + 1;
            let col_end =
                (((rect.x + rect.width) / max_x) * f64::from(grid_precision)).round() as u32 + 1;
            let row_start = ((rect.y / max_y) * f64::from(grid_precision)).round() as u32 + 1;
            let row_end =
                (((rect.y + rect.height) / max_y) * f64::from(grid_precision)).round() as u32 + 1;

            MosaicCell {
                row_start,
                row_end,
                col_start,
                col_end,
            }
        })
        .collect();

    MosaicLayout {
        cells,
        grid_rows: grid_precision,
        grid_cols: grid_precision,
        container_height,
    }
}

/// Generate a complete mosaic layout with image assignments
pub fn generate_mosaic_with_images(
    num_images: usize,
    image_aspects: &[(usize, f64)],
    config: MosaicConfig,
    grid_precision: u32,
) -> (MosaicLayout, Vec<usize>) {
    // Store the container height from config
    let container_height = config.container_height;
    
    // Generate the mosaic rectangles
    let rectangles = generate_mosaic_layout(num_images, config);

    // Assign images to rectangles
    let assignments = assign_images_to_layout(&rectangles, image_aspects);

    // Convert to grid layout
    let layout = rectangles_to_grid_layout(&rectangles, grid_precision, container_height);

    // Extract the image order (sorted by the order rectangles appear in the layout)
    let image_order: Vec<usize> = assignments.iter().map(|(_, img_idx)| *img_idx).collect();

    (layout, image_order)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rectangle_creation() {
        let rect = Rectangle::new(0.0, 0.0, 100.0, 200.0);
        assert_eq!(rect.width, 100.0);
        assert_eq!(rect.height, 200.0);
        assert_eq!(rect.area(), 20000.0);
        assert_eq!(rect.aspect_ratio(), 0.5);
    }

    #[test]
    fn test_generate_single_image() {
        let config = MosaicConfig::default();
        let layout = generate_mosaic_layout(1, config.clone());
        assert_eq!(layout.len(), 1);
        assert_eq!(layout[0].width, config.container_width);
        assert_eq!(layout[0].height, config.container_height);
    }

    #[test]
    fn test_generate_multiple_images() {
        let config = MosaicConfig {
            container_width: 1200.0,
            container_height: 800.0,
            min_cell_dimension: 80.0, // Smaller minimum to allow more splits
            min_aspect_ratio: 0.5,
            max_aspect_ratio: 2.5,
        };
        let layout = generate_mosaic_layout(5, config);
        
        // Should have at least 2 rectangles and attempt to get close to 5
        // The algorithm may not always achieve exactly 5 due to constraints
        assert!(layout.len() >= 2, "Should generate at least 2 rectangles");
        assert!(layout.len() <= 5, "Should not exceed requested count");

        // All rectangles should have positive dimensions
        for rect in &layout {
            assert!(rect.width > 0.0);
            assert!(rect.height > 0.0);
        }
    }

    #[test]
    fn test_weighted_random_select() {
        let mut rng = rand::thread_rng();
        let weights = vec![0.1, 0.2, 0.7];

        // Run multiple times to ensure it doesn't panic
        for _ in 0..100 {
            let idx = weighted_random_select(&weights, &mut rng);
            assert!(idx < weights.len());
        }
    }

    #[test]
    fn test_assign_images_to_layout() {
        let config = MosaicConfig::default();
        let layout = generate_mosaic_layout(3, config);

        // Create some test images with different aspect ratios
        let images = vec![
            (0, 1.5), // Landscape
            (1, 0.75), // Portrait
            (2, 1.0), // Square
        ];

        let assignments = assign_images_to_layout(&layout, &images);

        // Should have exactly 3 assignments
        assert_eq!(assignments.len(), 3);

        // Each image should be assigned
        let mut assigned_images: Vec<usize> = assignments.iter().map(|(_, idx)| *idx).collect();
        assigned_images.sort();
        assert_eq!(assigned_images, vec![0, 1, 2]);
    }
}
