use std::fmt::Display;
use std::sync::Mutex;

use rayon::iter::ParallelIterator;

/// An iterator and parallel iterator, that handles `Result::Err` variants with `error_handler` and
/// produces only the `Result::Ok` elements for downstream consumption.
pub struct HandleErrors<I, F> {
    iter: I,
    error_handler: F,
}

// Sequential iterator implementation.
impl<I, F, T, E> Iterator for HandleErrors<I, F>
where
    I: Iterator<Item = Result<T, E>>,
    F: FnMut(E),
{
    type Item = T;

    fn next(&mut self) -> Option<T> {
        // Handle all errors until we get to the first non-error item.
        loop {
            let result = self.iter.next();
            match result {
                Some(Err(err)) => (self.error_handler)(err),
                Some(Ok(item)) => return Some(item),
                None => return None
            }
        }
    }
}

// Parallel iterator implementation.
impl<I, F, T, E> ParallelIterator for HandleErrors<I, F>
where
    I: ParallelIterator<Item = Result<T, E>>,
    F: Fn(E) + Send + Sync,
    T: Send,
{
    type Item = T;

    fn drive_unindexed<C: rayon::iter::plumbing::UnindexedConsumer<T>>(self, consumer: C) -> C::Result {
        // See https://github.com/rayon-rs/rayon/issues/643 for a good example.
        let error_handler = self.error_handler;
        self.iter
            .filter_map(|result| 
                match result {
                    Ok(item) => Some(item),
                    Err(err) => { 
                        error_handler(err);
                        None
                    }
                }
            )
            .drive_unindexed(consumer)
    }
}

/// Extension trait for sequential `Iterator`s.
pub trait HandleErrorsIterExt<E>: Sized {
    fn handle_errors<F: FnMut(E)>(self, error_handler: F) -> HandleErrors<Self, F> {
        HandleErrors { iter: self, error_handler }
    }

    /// Convenience method to log errors with `log::error!`.
    fn log_errors(self) -> HandleErrors<Self, fn(E)> where E: Display {
        HandleErrors { iter: self, error_handler: |err| log::error!("{}", err) }
    }

    /// Pushes all `Result::Err` encountered while iterating into the provided `errors` vector.
    ///
    /// The lifetime constraint 'a on the error_handler (and thus on the HandleErrors iterator)
    /// ensures that `errors` can only be accessed (read from) once the iterator is consumed.
    /// That is, `errors` remains borrowed until the end of the lifetime of the return value.
    fn collect_errors<'a>(self, errors: &'a mut Vec<E>) -> HandleErrors<Self, Box<dyn FnMut(E) + 'a>> {
        let error_handler = Box::new(move |err| errors.push(err));
        HandleErrors { iter: self, error_handler }
    }
}

impl<I: Iterator<Item = Result<T, E>>, T, E> HandleErrorsIterExt<E> for I {}

/// Extension trait for `ParallelIterator`s.
// This is almost exactly the same as HandleErrorsIterExt, except for some changes due to concurrency:
// 1. `error_handler` is an `Fn` closure, not `FnMut`, i.e., it cannot mutably access its environment
// (since it might access it from different worker threads).
// 2. `collect_errors` synchronizes the access to `errors` with a `Mutex`, in order to ensure that
// only one thread can append errors at the same time. (And thus fulfilling the `Fn` bound, see 1.)
// Also, the returned closure must be Send + Sync for HandleErrors to be a valid `ParallelIterator`.
//
// (Another reason for duplicating the trait, is that one cannot implement generically both traits
// at the same type, because a third-party type could implement both Iterator AND ParallelIterator
// and then we would have a conflicting implementation.)
pub trait HandleErrorsParIterExt<E>: Sized {
    fn handle_errors<F: Fn(E)>(self, error_handler: F) -> HandleErrors<Self, F> {
        HandleErrors { iter: self, error_handler }
    }

    // See `HandleErrorsIterExt`.
    fn log_errors(self) -> HandleErrors<Self, fn(E)> where E: Display {
        HandleErrors { iter: self, error_handler: |err| log::error!("{}", err)}
    }

    // See `HandleErrorsIterExt`.
    fn collect_errors<'a>(self, errors: &'a mut Vec<E>) -> HandleErrors<Self, Box<dyn Fn(E) + 'a + Send + Sync>> where E: Send {
        let errors = Mutex::new(errors);
        let error_handler = Box::new(move |err| errors.lock().unwrap().push(err));
        HandleErrors { iter: self, error_handler }
    }
}

impl<I: ParallelIterator<Item = Result<T, E>>, T, E> HandleErrorsParIterExt<E> for I {}
