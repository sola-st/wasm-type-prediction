use indicatif::{ParProgressBarIter, ParallelProgressIterator};
use rayon::iter::IndexedParallelIterator;

pub mod handle_errors;
pub mod frequencies;
pub mod human_gnu_format;
pub mod into_seq_iter;
pub mod sample_writer;
pub mod percent;
pub mod file_error;
pub mod cmultimap;
// pub mod progress_bar_log;

// WORKAROUND IndexedParallelIterator know their length, so we can use that instead of 0 by default.
// See https://github.com/mitsuhiko/indicatif/issues/242
// Use progress_bar() instead of progress() to avoid ambiguous method name vs. upstream.
pub trait ParallelProgressBar: IndexedParallelIterator {
    fn progress_bar(self) -> ParProgressBarIter<Self> {
        let total = self.len() as u64;
        self.progress_count(total)
    }
}

impl<I: IndexedParallelIterator> ParallelProgressBar for I {}
