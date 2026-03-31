//! Tile filename pattern resolution engine.
//!
//! Resolves configurable filename patterns for tile export.
//! Supported variables: `{col}` (or `{x}`), `{row}` (or `{y}`), `{seq}`.
//! Optional zero-padding: `{var:N}` pads to N digits with leading zeros.

use anyhow::{bail, Result};

/// Resolve a tile filename pattern by substituting variables.
///
/// Variables:
/// - `{col}` / `{x}` — Column index (0-based)
/// - `{row}` / `{y}` — Row index (0-based)
/// - `{seq}` — Sequential counter (1-based)
/// - `{var:N}` — Zero-padded to N digits
pub fn resolve_tile_pattern(pattern: &str, col: usize, row: usize, seq: usize) -> Result<String> {
    let mut result = String::with_capacity(pattern.len());
    let chars: Vec<char> = pattern.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        if chars[i] == '{' {
            // Find closing brace
            let start = i + 1;
            let end = chars[start..]
                .iter()
                .position(|&c| c == '}')
                .map(|pos| start + pos);

            let end = match end {
                Some(e) => e,
                None => bail!("Unclosed '{{' in pattern at position {}", i),
            };

            let placeholder: String = chars[start..end].iter().collect();
            let (var_name, width) = parse_placeholder(&placeholder)?;

            let value = match var_name {
                "col" | "x" => col,
                "row" | "y" => row,
                "seq" => seq,
                _ => bail!(
                    "Unknown pattern variable '{{{}}}'. Valid: col, row, seq (aliases: x, y)",
                    var_name
                ),
            };

            if let Some(w) = width {
                result.push_str(&format!("{:0>width$}", value, width = w));
            } else {
                result.push_str(&value.to_string());
            }

            i = end + 1;
        } else {
            result.push(chars[i]);
            i += 1;
        }
    }

    Ok(result)
}

/// Validate a tile pattern without resolving it.
/// Checks that all `{placeholders}` use known variables and valid format specifiers.
/// Delegates to `resolve_tile_pattern` with dummy values to avoid duplicating parsing logic.
pub fn validate_tile_pattern(pattern: &str) -> Result<()> {
    resolve_tile_pattern(pattern, 0, 0, 0).map(|_| ())
}

/// Parse a placeholder like "col" or "col:03" into (name, optional_width).
fn parse_placeholder(placeholder: &str) -> Result<(&str, Option<usize>)> {
    if let Some(colon_pos) = placeholder.find(':') {
        let name = &placeholder[..colon_pos];
        let width_str = &placeholder[colon_pos + 1..];
        let width: usize = width_str.parse().map_err(|_| {
            anyhow::anyhow!(
                "Invalid width '{}' in pattern variable '{{{}}}'. Expected positive integer",
                width_str,
                placeholder
            )
        })?;
        if width == 0 {
            bail!(
                "Width must be positive in pattern variable '{{{}}}'",
                placeholder
            );
        }
        Ok((name, Some(width)))
    } else {
        Ok((placeholder, None))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // === resolve_tile_pattern tests ===

    #[test]
    fn test_basic_col_row() {
        let result = resolve_tile_pattern("{col}_{row}.mp", 15, 42, 1).unwrap();
        assert_eq!(result, "15_42.mp");
    }

    #[test]
    fn test_zero_padding() {
        let result = resolve_tile_pattern("{col:03}_{row:03}.mp", 5, 42, 1).unwrap();
        assert_eq!(result, "005_042.mp");
    }

    #[test]
    fn test_seq_basic() {
        let result = resolve_tile_pattern("{seq}.mp", 0, 0, 157).unwrap();
        assert_eq!(result, "157.mp");
    }

    #[test]
    fn test_seq_zero_padded() {
        let result = resolve_tile_pattern("{seq:04}.mp", 0, 0, 157).unwrap();
        assert_eq!(result, "0157.mp");
    }

    #[test]
    fn test_alias_x_y() {
        let result = resolve_tile_pattern("{x}_{y}.mp", 15, 42, 1).unwrap();
        assert_eq!(result, "15_42.mp");
    }

    #[test]
    fn test_alias_x_y_with_padding() {
        let result = resolve_tile_pattern("{x:03}_{y:03}.mp", 5, 42, 1).unwrap();
        assert_eq!(result, "005_042.mp");
    }

    #[test]
    fn test_subdirectory_pattern() {
        let result = resolve_tile_pattern("{col}/{row}.mp", 15, 42, 1).unwrap();
        assert_eq!(result, "15/42.mp");
    }

    #[test]
    fn test_prefix_pattern() {
        let result = resolve_tile_pattern("tile_{col}_{row}.mp", 15, 42, 1).unwrap();
        assert_eq!(result, "tile_15_42.mp");
    }

    #[test]
    fn test_all_variables() {
        let result = resolve_tile_pattern("{col}_{row}_{seq}.mp", 15, 42, 157).unwrap();
        assert_eq!(result, "15_42_157.mp");
    }

    #[test]
    fn test_large_padding() {
        let result = resolve_tile_pattern("{seq:06}.mp", 0, 0, 42).unwrap();
        assert_eq!(result, "000042.mp");
    }

    #[test]
    fn test_value_wider_than_padding() {
        // Value 1234 is wider than padding 03 — no truncation
        let result = resolve_tile_pattern("{col:03}.mp", 1234, 0, 1).unwrap();
        assert_eq!(result, "1234.mp");
    }

    #[test]
    fn test_zero_values() {
        let result = resolve_tile_pattern("{col}_{row}.mp", 0, 0, 1).unwrap();
        assert_eq!(result, "0_0.mp");
    }

    #[test]
    fn test_unknown_variable_error() {
        let result = resolve_tile_pattern("{invalid_var}.mp", 0, 0, 1);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("Unknown pattern variable"));
        assert!(err.contains("invalid_var"));
        assert!(err.contains("Valid: col, row, seq"));
    }

    #[test]
    fn test_invalid_width_error() {
        let result = resolve_tile_pattern("{col:abc}.mp", 0, 0, 1);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Invalid width"));
    }

    #[test]
    fn test_unclosed_brace_error() {
        let result = resolve_tile_pattern("{col.mp", 0, 0, 1);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Unclosed"));
    }

    #[test]
    fn test_no_placeholders() {
        let result = resolve_tile_pattern("fixed_name.mp", 15, 42, 1).unwrap();
        assert_eq!(result, "fixed_name.mp");
    }

    // === validate_tile_pattern tests ===

    #[test]
    fn test_validate_valid_patterns() {
        assert!(validate_tile_pattern("{col}_{row}.mp").is_ok());
        assert!(validate_tile_pattern("{col:03}_{row:03}.mp").is_ok());
        assert!(validate_tile_pattern("{seq:04}.mp").is_ok());
        assert!(validate_tile_pattern("{x}_{y}.mp").is_ok());
        assert!(validate_tile_pattern("{col}/{row}.mp").is_ok());
        assert!(validate_tile_pattern("tile_{col}_{row}.mp").is_ok());
    }

    #[test]
    fn test_validate_unknown_variable() {
        let result = validate_tile_pattern("{invalid_var}.mp");
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("Unknown pattern variable"));
        assert!(err.contains("invalid_var"));
    }

    #[test]
    fn test_validate_invalid_width() {
        let result = validate_tile_pattern("{col:abc}.mp");
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_zero_width() {
        let result = validate_tile_pattern("{col:0}.mp");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Width must be positive"));
    }

    #[test]
    fn test_validate_unclosed_brace() {
        let result = validate_tile_pattern("{col.mp");
        assert!(result.is_err());
    }
}
