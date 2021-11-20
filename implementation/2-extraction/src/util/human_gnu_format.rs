/// Print a number in short human format, similar to GNU tools, e.g., 10M for 10 million.
/// The number including the suffix will be at most 4 characters long, e.g., 1.3M or 400K.
pub fn format_integer(uint: u64) -> String {
    format(uint, 1000.0, &["k", "M", "B", "T", "P"], "")
}

/// Similar to format_integer, except that it uses binary suffixes (Kibibyte etc.) and B as a unit.
pub fn format_file_size_binary(uint: u64) -> String {
    format(uint, 1024.0, &["Ki", "Mi", "Gi", "Ti", "Pi"], "B")
}

fn format(uint: u64, base: f64, suffixes: &[&str], unit: &str) -> String {
    // Scale down number, iteratively going through the suffixes.
    let mut scaled_value = uint as f64;
    let mut suffix = None;
    for s in suffixes {
        if scaled_value < base {
            break;
        }
        scaled_value = scaled_value / base;
        suffix = Some(s);
    }

    // Format the number such that it takes at most 4 characters (e.g., "0", "10K", "100K", or "1.5K").
    match suffix {
        // Whole number fits, no fractional part, no suffix.
        None => format!("{}{}", uint, unit),
        // Integral part has only a single digit, use additional space for fractional part.
        Some(suffix) if scaled_value < 10.0 => format!("{:.1}{}{}", scaled_value, suffix, unit),
        Some(suffix) => format!("{:.0}{}{}", scaled_value, suffix, unit),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_integer() {
        assert_eq!(format_integer(0), "0");
        assert_eq!(format_integer(1), "1");
        assert_eq!(format_integer(10), "10");
        assert_eq!(format_integer(100), "100");
        assert_eq!(format_integer(1000), "1.0k");
        assert_eq!(format_integer(1100), "1.1k");
        assert_eq!(format_integer(1500), "1.5k");
        assert_eq!(format_integer(2000), "2.0k");
        assert_eq!(format_integer(10_000), "10k");
        assert_eq!(format_integer(100_000), "100k");
        assert_eq!(format_integer(1000_000), "1.0M");
        assert_eq!(format_integer(1500_000), "1.5M");
        assert_eq!(format_integer(10_000_000), "10M");
        assert_eq!(format_integer(150_000_000), "150M");
    }

    #[test]
    fn test_file_size() {
        assert_eq!(format_file_size_binary(0), "0B");
        assert_eq!(format_file_size_binary(1), "1B");
        assert_eq!(format_file_size_binary(10), "10B");
        assert_eq!(format_file_size_binary(100), "100B");
        // FIXME Output is wider than 3 digits, because 1000<1024 and thus output is 1000B.
        assert_eq!(format_file_size_binary(1000), "1.0KiB");
        assert_eq!(format_file_size_binary(1100), "1.1KiB");
    }
}
