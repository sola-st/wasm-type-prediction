
use std::convert::TryInto;
use std::fmt;

/// Easy formatting and alignment of percentages.
#[derive(Debug, Clone, Copy, PartialOrd, PartialEq)]
pub struct Percent(f64);

/// Display percentages with one decimal place and % sign.
/// Right-align to 100.0% with the alternate formatting flag `#`, e.g. `format!("{:#}", percent)`.
impl fmt::Display for Percent {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if f.alternate() {
            write!(f, "{:5.1}%", self.0)
        } else {
            write!(f, "{:.1}%", self.0)
        }
    }
}

impl Percent {
    // TryInto<u64> as a poor man's generic unsigned integer type.
    pub fn from_counts<T: TryInto<u64>, U: TryInto<u64>>(part: T, total: U) -> Self {
        // Since there are no generic lossy int-to-float conversion traits, go via u64 + as.
        let part: u64 = part.try_into().ok().unwrap();
        let total: u64 = total.try_into().ok().unwrap();
        Percent((part as f64 / total as f64) * 100.0)
    }

    pub fn from_ratio(ratio: f64) -> Self {
        Self(ratio * 100.0)
    }

    pub fn into_ratio(self) -> f64 {
        self.0 / 100.0
    }
}
