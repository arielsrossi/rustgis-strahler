//! Colors and classes for the Strahler orders of a layer. Plain functions, tested without RustGIS.

/// The colors used when none are given: the same four as the palette in the examples.
pub const DEFAULT_COLORS: [&str; 4] = ["#1f77b4", "#ff7f0e", "#2ca02c", "#d62728"];

/// RustGIS shows at most this many classes in a graduated symbology.
pub const MAX_CLASSES: usize = 12;

/// The distinct orders, in ascending order.
pub fn distinct_orders(orders: impl IntoIterator<Item = u64>) -> Vec<u64> {
    let mut all: Vec<u64> = orders.into_iter().collect();
    all.sort_unstable();
    all.dedup();
    all
}

/// The colors of a comma-separated `#RRGGBB` list, or the defaults when the list is blank.
pub fn parse_colors(text: &str) -> Result<Vec<String>, String> {
    let list: Vec<String> = text.split(',').map(str::trim).filter(|c| !c.is_empty()).map(str::to_string).collect();
    if list.is_empty() {
        return Ok(DEFAULT_COLORS.iter().map(|c| c.to_string()).collect());
    }
    match list.iter().find(|c| !is_hex_color(c)) {
        Some(bad) => Err(format!("'{bad}' is not a #RRGGBB color.")),
        None => Ok(list),
    }
}

fn is_hex_color(color: &str) -> bool {
    color.len() == 7 && color.starts_with('#') && color[1..].chars().all(|c| c.is_ascii_hexdigit())
}

/// The color of class `index`: the list's colors in turn, the last one repeated after that.
pub fn color_for(index: usize, colors: &[String]) -> &str {
    &colors[index.min(colors.len() - 1)]
}

/// The upper bounds of the classes for manual breaks: one per order but the largest, which
/// closes the last class. With orders 1 to 4 this gives classes `<=1`, `<=2`, `<=3` and `<=4`.
pub fn breaks(orders: &[u64]) -> Vec<f64> {
    orders[..orders.len().saturating_sub(1)].iter().map(|&o| o as f64).collect()
}

/// The width of a line of order `order`, in points: order 1 is 2 pt, order 4 is 5 pt.
pub fn width_for(order: u64) -> f64 {
    (order + 1) as f64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn higher_orders_are_thicker() {
        assert!(width_for(1) < width_for(2) && width_for(2) < width_for(4));
        assert_eq!(width_for(1), 2.0);
        assert_eq!(width_for(4), 5.0);
    }

    #[test]
    fn orders_are_distinct_and_sorted() {
        assert_eq!(distinct_orders([3, 1, 3, 2, 1]), vec![1, 2, 3]);
        assert!(distinct_orders([]).is_empty());
    }

    #[test]
    fn colors_are_parsed_or_defaulted() {
        assert_eq!(parse_colors("  ").unwrap().len(), 4);
        assert_eq!(parse_colors("#000000, #FFFFFF").unwrap(), vec!["#000000", "#FFFFFF"]);
        assert!(parse_colors("red").is_err());
        assert!(parse_colors("#12345").is_err());
    }

    #[test]
    fn the_last_color_repeats() {
        let colors = vec!["#000000".to_string(), "#ffffff".to_string()];
        assert_eq!(color_for(0, &colors), "#000000");
        assert_eq!(color_for(5, &colors), "#ffffff");
    }

    #[test]
    fn breaks_close_on_the_largest_order() {
        assert_eq!(breaks(&[1, 2, 3, 4]), vec![1.0, 2.0, 3.0]);
        assert!(breaks(&[2]).is_empty());
    }
}
