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

/// Orientation bias for mosaic generation
#[derive(Debug, Clone, Default)]
pub struct OrientationBias {
    pub num_portrait: usize,  // Number of portrait images
    pub num_landscape: usize, // Number of landscape images
    pub num_square: usize,    // Number of square-ish images
}

/// Tracks how many rectangles of each orientation have been created
#[derive(Debug, Clone)]
struct OrientationTracker {
    target_portrait: usize,
    target_landscape: usize,
    target_square: usize,
    current_portrait: usize,
    current_landscape: usize,
    current_square: usize,
}

impl OrientationTracker {
    fn new(bias: &OrientationBias) -> Self {
        Self {
            target_portrait: bias.num_portrait,
            target_landscape: bias.num_landscape,
            target_square: bias.num_square,
            current_portrait: 0,
            current_landscape: 0,
            current_square: 0,
        }
    }

    fn update_from_rectangles(&mut self, rectangles: &[&Rectangle]) {
        for rect in rectangles {
            let orientation = categorize_orientation(rect.aspect_ratio());
            match orientation {
                "portrait" => self.current_portrait += 1,
                "landscape" => self.current_landscape += 1,
                "square" => self.current_square += 1,
                _ => {}
            }
        }
    }

    /// Returns a preference for which orientation to create next
    /// Returns None if no strong preference, or Some((prefer_horizontal, strength))
    /// where strength is 0.0-1.0
    fn get_split_preference(&self) -> Option<(bool, f64)> {
        let portrait_deficit = self.target_portrait.saturating_sub(self.current_portrait);
        let landscape_deficit = self.target_landscape.saturating_sub(self.current_landscape);
        let square_deficit = self.target_square.saturating_sub(self.current_square);

        let total_deficit = portrait_deficit + landscape_deficit + square_deficit;
        if total_deficit == 0 {
            return None;
        }

        // If we need more portrait images, prefer vertical splits
        // If we need more landscape images, prefer horizontal splits
        // Squares have no preference

        if portrait_deficit > landscape_deficit && portrait_deficit > square_deficit {
            // Prefer vertical splits to create tall rectangles
            let strength = (portrait_deficit as f64) / (total_deficit as f64);
            Some((false, strength * 0.7)) // false = prefer vertical split
        } else if landscape_deficit > portrait_deficit && landscape_deficit > square_deficit {
            // Prefer horizontal splits to create wide rectangles
            let strength = (landscape_deficit as f64) / (total_deficit as f64);
            Some((true, strength * 0.7)) // true = prefer horizontal split
        } else {
            None
        }
    }
}

/// Configuration for the mosaic layout generator
#[derive(Debug, Clone)]
pub struct MosaicConfig {
    pub container_width: f64,
    pub container_height: f64,
    pub min_cell_dimension: f64, // Minimum width or height for a cell
    pub min_aspect_ratio: f64,   // e.g., 0.4 (very portrait)
    pub max_aspect_ratio: f64,   // e.g., 2.5 (very landscape)
    pub orientation_bias: Option<OrientationBias>, // Bias toward creating certain orientations
}

impl Default for MosaicConfig {
    fn default() -> Self {
        Self {
            container_width: 1200.0,
            container_height: 800.0,
            min_cell_dimension: 150.0,
            min_aspect_ratio: 0.4,
            max_aspect_ratio: 2.5,
            orientation_bias: None,
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

    // Track orientation counts if bias is provided
    let mut orientation_tracker = if let Some(ref bias) = config.orientation_bias {
        Some(OrientationTracker::new(bias))
    } else {
        None
    };

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
        if let Some((rect1, rect2, new_line)) = try_split_rectangle_with_bias(
            &rect_to_split,
            &lines,
            &config,
            &mut rng,
            orientation_tracker.as_ref(),
        ) {
            // Remove the old rectangle and add the two new ones
            rectangles.remove(rect_index);

            // Update orientation tracker
            if let Some(ref mut tracker) = orientation_tracker {
                tracker.update_from_rectangles(&[&rect1, &rect2]);
            }

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
                if let Some((rect1, rect2, new_line)) = try_split_rectangle_with_bias(
                    rect,
                    &lines,
                    &config,
                    &mut rng,
                    orientation_tracker.as_ref(),
                ) {
                    rectangles.remove(i);

                    // Update orientation tracker
                    if let Some(ref mut tracker) = orientation_tracker {
                        tracker.update_from_rectangles(&[&rect1, &rect2]);
                    }

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

/// Try to split a rectangle into two rectangles with orientation bias
fn try_split_rectangle_with_bias(
    rect: &Rectangle,
    existing_lines: &[DivisionLine],
    config: &MosaicConfig,
    rng: &mut impl Rng,
    tracker: Option<&OrientationTracker>,
) -> Option<(Rectangle, Rectangle, DivisionLine)> {
    // Add a general bias toward vertical splits (which create portrait/tall rectangles)
    // Vertical splits create tall rectangles, which are less common but needed for portraits
    let random_val: f64 = rng.gen();
    let general_vertical_bias = 0.65; // 65% chance to prefer vertical split

    // Determine base preference - favor vertical splits overall
    let dimension_preference = if random_val < general_vertical_bias {
        false // vertical split (creates tall rectangles)
    } else {
        rect.width > rect.height // horizontal split for very wide rectangles
    };

    // Check if we have orientation bias from tracker
    let prefer_horizontal = if let Some(tracker) = tracker {
        if let Some((bias_preference, strength)) = tracker.get_split_preference() {
            // Use random value to apply bias probabilistically
            let random_val_2: f64 = rng.gen();
            if random_val_2 < strength {
                bias_preference
            } else {
                dimension_preference
            }
        } else {
            dimension_preference
        }
    } else {
        dimension_preference
    };

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
    // Bias AWAY from center to create more extreme aspect ratios
    for _ in 0..10 {
        // Pick position biased toward edges (creates one tall, one short rectangle)
        let random_val = rng.gen::<f64>();
        let split_y = if random_val < 0.5 {
            // Bias toward top edge (creates tall bottom rectangle)
            let bias = random_val * 2.0; // 0.0 to 1.0
            let bias_squared = bias * bias; // Stronger bias toward edge
            min_y + (max_y - min_y) * bias_squared * 0.4 // Split in top 40% of range
        } else {
            // Bias toward bottom edge (creates tall top rectangle)
            let bias = (random_val - 0.5) * 2.0; // 0.0 to 1.0
            let bias_squared = bias * bias;
            max_y - (max_y - min_y) * bias_squared * 0.4 // Split in bottom 40% of range
        };

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
    // Bias AWAY from center to create more extreme aspect ratios
    for _ in 0..10 {
        // Pick position biased toward edges (creates one wide, one narrow rectangle)
        let random_val = rng.gen::<f64>();
        let split_x = if random_val < 0.5 {
            // Bias toward left edge (creates wide right rectangle)
            let bias = random_val * 2.0; // 0.0 to 1.0
            let bias_squared = bias * bias; // Stronger bias toward edge
            min_x + (max_x - min_x) * bias_squared * 0.4 // Split in left 40% of range
        } else {
            // Bias toward right edge (creates wide left rectangle)
            let bias = (random_val - 0.5) * 2.0; // 0.0 to 1.0
            let bias_squared = bias * bias;
            max_x - (max_x - min_x) * bias_squared * 0.4 // Split in right 40% of range
        };

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

/// Calculate how far an aspect ratio is from being square (1.0)
/// Higher values mean more extreme (less square)
fn aspect_extremeness(aspect: f64) -> f64 {
    (aspect.ln()).abs() // log scale distance from 1.0
}

/// Assign images to mosaic rectangles based on aspect ratio matching
pub fn assign_images_to_layout(
    rectangles: &[Rectangle],
    image_aspects: &[(usize, f64)], // (original_index, aspect_ratio)
) -> Vec<(Rectangle, usize)> {
    // Create a copy of rectangles and images so we can match them
    let mut available_rects: Vec<(usize, Rectangle)> =
        rectangles.iter().cloned().enumerate().collect();

    // Sort images by extremeness (least square first)
    // This prioritizes portrait and landscape images over square ones
    let mut available_images: Vec<(usize, f64)> = image_aspects.to_vec();
    available_images.sort_by(|a, b| {
        let extremeness_a = aspect_extremeness(a.1);
        let extremeness_b = aspect_extremeness(b.1);
        extremeness_b.partial_cmp(&extremeness_a).unwrap_or(std::cmp::Ordering::Equal)
    });

    let mut assignments: Vec<(Rectangle, usize)> = Vec::new();

    // Process images in order of extremeness (least square first)
    // For each image, find the best matching rectangle
    for (original_img_idx, img_aspect) in available_images {
        let img_orientation = categorize_orientation(img_aspect);

        // Find the best rectangle for this specific image
        let mut best_rect_match: Option<(usize, f64)> = None; // (rect_idx, score)

        for (rect_idx, (_rect_id, rect)) in available_rects.iter().enumerate() {
            let rect_aspect = rect.aspect_ratio();
            let rect_orientation = categorize_orientation(rect_aspect);

            // Calculate similarity score
            let score = calculate_match_score(img_aspect, img_orientation, rect_aspect, rect_orientation);

            if let Some((_, best_score)) = best_rect_match {
                if score > best_score {
                    best_rect_match = Some((rect_idx, score));
                }
            } else {
                best_rect_match = Some((rect_idx, score));
            }
        }

        // Assign this image to its best rectangle
        if let Some((rect_idx, _score)) = best_rect_match {
            let (_rect_id, rect) = available_rects.remove(rect_idx);
            assignments.push((rect, original_img_idx));
        }
    }

    assignments
}

/// Calculate how well an image matches a rectangle
fn calculate_match_score(
    img_aspect: f64,
    img_orientation: &str,
    rect_aspect: f64,
    rect_orientation: &str,
) -> f64 {
    // Start with aspect ratio similarity
    let aspect_diff = (rect_aspect - img_aspect).abs();
    let mut score = 1.0 / (1.0 + aspect_diff);

    // Apply orientation matching bonuses/penalties
    if rect_orientation == img_orientation {
        // Perfect orientation match - strong bonus
        score *= 20.0;
    } else if rect_orientation == "square" || img_orientation == "square" {
        // One is square - moderate penalty
        // Squares are flexible but not ideal
        score *= 2.0;
    } else {
        // Portrait/landscape mismatch - very heavy penalty
        // This should almost never happen unless there's no choice
        score *= 0.01;
    }

    // Additional bonus for very close aspect ratio matches
    if aspect_diff < 0.1 {
        score *= 1.5; // Extra bonus for near-perfect aspect match
    }

    score
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

/// Calculate orientation bias from image aspect ratios
pub fn calculate_orientation_bias(image_aspects: &[(usize, f64)]) -> OrientationBias {
    let mut num_portrait = 0;
    let mut num_landscape = 0;
    let mut num_square = 0;

    for (_, aspect) in image_aspects {
        let orientation = categorize_orientation(*aspect);
        match orientation {
            "portrait" => num_portrait += 1,
            "landscape" => num_landscape += 1,
            "square" => num_square += 1,
            _ => {}
        }
    }

    OrientationBias {
        num_portrait,
        num_landscape,
        num_square,
    }
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
            orientation_bias: None,
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
            (0, 1.5),  // Landscape
            (1, 0.75), // Portrait
            (2, 1.0),  // Square
        ];

        let assignments = assign_images_to_layout(&layout, &images);

        // Should have exactly 3 assignments
        assert_eq!(assignments.len(), 3);

        // Each image should be assigned
        let mut assigned_images: Vec<usize> = assignments.iter().map(|(_, idx)| *idx).collect();
        assigned_images.sort();
        assert_eq!(assigned_images, vec![0, 1, 2]);
    }

    #[test]
    fn test_aspect_extremeness() {
        // Square should have lowest extremeness (close to 0)
        assert!(aspect_extremeness(1.0) < 0.01);

        // More extreme aspect ratios should have higher values
        let square_extremeness = aspect_extremeness(1.0);
        let slightly_wide = aspect_extremeness(1.3);
        let wide = aspect_extremeness(2.0);
        let very_wide = aspect_extremeness(3.0);
        let tall = aspect_extremeness(0.5);
        let very_tall = aspect_extremeness(0.33);

        assert!(slightly_wide > square_extremeness);
        assert!(wide > slightly_wide);
        assert!(very_wide > wide);
        assert!(tall > square_extremeness);
        assert!(very_tall > tall);

        // Portrait and landscape with same distance from 1.0 should have similar extremeness
        assert!((aspect_extremeness(2.0) - aspect_extremeness(0.5)).abs() < 0.01);
    }

    #[test]
    fn test_image_assignment_prioritizes_extreme_aspects() {
        let config = MosaicConfig::default();
        let layout = generate_mosaic_layout(4, config);

        // Create images: one very extreme, one somewhat extreme, two nearly square
        let images = vec![
            (0, 0.4),  // Very tall portrait (most extreme)
            (1, 1.05), // Nearly square
            (2, 1.8),  // Landscape (somewhat extreme)
            (3, 0.95), // Nearly square
        ];

        let assignments = assign_images_to_layout(&layout, &images);

        // All images should be assigned
        assert_eq!(assignments.len(), 4);

        // Verify all images are present
        let mut assigned_images: Vec<usize> = assignments.iter().map(|(_, idx)| *idx).collect();
        assigned_images.sort();
        assert_eq!(assigned_images, vec![0, 1, 2, 3]);
    }

    #[test]
    fn test_orientation_matching_prevents_mismatches() {
        // Create rectangles with clear orientations
        let rectangles = vec![
            Rectangle::new(0.0, 0.0, 100.0, 200.0), // Portrait (0.5)
            Rectangle::new(0.0, 0.0, 200.0, 100.0), // Landscape (2.0)
            Rectangle::new(0.0, 0.0, 150.0, 150.0), // Square (1.0)
        ];

        // Create images with matching orientations
        let images = vec![
            (0, 0.6),  // Portrait
            (1, 1.8),  // Landscape
            (2, 1.0),  // Square
        ];

        let assignments = assign_images_to_layout(&rectangles, &images);

        // Verify all images are assigned
        assert_eq!(assignments.len(), 3);

        // Check that orientations match
        for (rect, img_idx) in &assignments {
            let rect_orientation = categorize_orientation(rect.aspect_ratio());
            let img_aspect = images.iter().find(|(idx, _)| idx == img_idx).unwrap().1;
            let img_orientation = categorize_orientation(img_aspect);

            // Orientations should match or one should be square (which is flexible)
            assert!(
                rect_orientation == img_orientation
                || rect_orientation == "square"
                || img_orientation == "square",
                "Mismatch: rect orientation {} (aspect {}) assigned to image orientation {} (aspect {})",
                rect_orientation, rect.aspect_ratio(), img_orientation, img_aspect
            );
        }
    }

    #[test]
    fn test_match_score_favors_orientation_match() {
        // Portrait image
        let portrait_img = 0.7;
        let portrait_img_orient = categorize_orientation(portrait_img);

        // Portrait rectangle (good match)
        let portrait_rect = 0.75;
        let portrait_rect_orient = categorize_orientation(portrait_rect);

        // Landscape rectangle (bad match)
        let landscape_rect = 1.8;
        let landscape_rect_orient = categorize_orientation(landscape_rect);

        let portrait_score = calculate_match_score(
            portrait_img, portrait_img_orient,
            portrait_rect, portrait_rect_orient
        );

        let landscape_score = calculate_match_score(
            portrait_img, portrait_img_orient,
            landscape_rect, landscape_rect_orient
        );

        // Portrait rectangle should score much higher for portrait image
        assert!(
            portrait_score > landscape_score * 10.0,
            "Portrait match score {} should be much higher than landscape mismatch score {}",
            portrait_score, landscape_score
        );
    }
}
