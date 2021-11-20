// TODO publish on crates.io

use std::collections::HashSet;
use std::convert::TryInto;
use std::hash::Hash;
use std::iter::Sum;

use crate::util::percent::Percent;

/// Extension trait for iterators over items and their counts. Roughly inspired by Python's
/// [https://docs.python.org/3/library/collections.html#collections.Counter](Counter) collection.
pub trait Frequencies<T, U>: Iterator<Item = (T, U)>
where
    // Poor man's generic constraint for unsigned integers: Sum for the total count, Ord for sorting
    U: TryInto<u64> + Copy + Sum + Ord,
    Self: Sized,
{
    fn unique_count(self) -> usize
    where
        T: Hash + Eq
    {
        let unique_items: HashSet<T> = self.map(|(item, _count)| item).collect();
        unique_items.len()
    }

    fn total_count(self) -> U {
        self.map(|(_, count)| count).sum()
    }

    fn with_percent(self) -> WithPercent<U, Self> 
    where 
        // Need to iterate over the counts twice, once for the total, once for adding the percentages.
        Self: Clone,
    {
        let total = self.clone().total_count();
        WithPercent { total, iter: self.into_iter() }
    }

    fn most_common(self, n: usize) -> std::iter::Take<std::vec::IntoIter<(T, U)>> {
        self.sorted_counts_desc().take(n)
    }

    fn sorted_counts_asc(self) -> std::vec::IntoIter<(T, U)> {
        let mut counts: Vec<_> = self.collect();
        counts.sort_by_key(|(_, count)| *count);
        counts.into_iter()
    }

    fn sorted_counts_desc(self) -> std::vec::IntoIter<(T, U)> {
        let mut counts: Vec<_> = self.collect();
        counts.sort_by_key(|(_, count)| std::cmp::Reverse(*count));
        counts.into_iter()
    }

    fn sorted_items(self) -> std::vec::IntoIter<(T, U)>
    where
        T: Ord
    {
        let mut counts: Vec<_> = self.collect();
        counts.sort_by(|a, b| a.0.cmp(&b.0));
        let counts = counts;
        counts.into_iter()
    }
}

impl<T, U, I> Frequencies<T, U> for I
where
    U: TryInto<u64> + Copy + Sum + Ord,
    I: Iterator<Item = (T, U)>,
{}

/// Return type of `with_percent` method.
pub struct WithPercent<U, I> {
    total: U,
    iter: I,
}

impl<T, U, I> Iterator for WithPercent<U, I>
where
    U: TryInto<u64> + Copy,
    I: Iterator<Item = (T, U)>,
{
    // Make the result type a pair (where the second element is a count), such that this iterator
    // is still compatible with the extension methods above. This way, one can chain e.g.
    // `some_iter.with_percent().most_common()`.
    type Item = ((T, Percent), U);

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next().map(|(item, count): I::Item| -> Self::Item {
            let percent = Percent::from_counts(count, self.total);
            ((item, percent), count)
        })
    }
}
