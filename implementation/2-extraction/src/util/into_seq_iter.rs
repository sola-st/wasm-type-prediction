use std::sync::mpsc;

use rayon::iter::ParallelIterator;

/// Convert a rayon ParallelIterator into a sequential Iterator that can, e.g., be consumed on the
/// main thread and further processed with the standard Iterator methods.
/// Allocates only a finite amount of memory and applies back-pressure from the sequential end to
/// the parallel iterator.
/// Items come in arbitrary order, as they are processed by the ParallelIterator.
/// (In general, order cannot be re-established unless potentially allocating enough space for all
/// elements, at which point you can just collect into a Vec or other collections.)
///
/// Uses a MPSC queue that is fed by multiple parallel producers (the input ParallelIterator) and
/// that can be emptied by consuming the returned standard Iterator.
/// The queue is bounded, such that if the parallel producers are "faster" than the sequential
/// consumer, there is not going to be memory exhaustion, but the parallel producers will slow down.
///
/// See https://users.rust-lang.org/t/is-there-some-way-to-convert-rayon-parallel-iterator-back-to-sequential-iterator/31827
/// and https://github.com/rayon-rs/rayon/issues/210 for inspiration
/// and https://github.com/nwtgck/rayon-seq-iter/blob/develop/src/lib.rs

pub struct SeqIter<'a, T> {
    receiver: mpsc::Receiver<T>,
    // Joins the scoped sender thread once this iterator is dropped (e.g., because all elements
    // have been consumed, or the receiver has hung up).
    _join_senders: thread_scoped::JoinGuard<'a, ()>,
}

impl<'a, T> Iterator for SeqIter<'a, T> {
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        self.receiver.recv().ok()
    }
}

pub trait IntoSeqIter<'a>: ParallelIterator + 'a {
    #[must_use = "to sequentially process the items, consume the result iterator in a for loop or with the regular iterator methods"]
    fn into_seq_iter(self) -> SeqIter<'a, Self::Item> {
        // Create a bounded sync_channel that buffers a limited number of items.
        //
        // There are two reasons for mpsc::sync_channel/bounded instead of mpsc::channel/unbounded:
        // 1. Memory exhaustion: If the parallel producers (senders of the MPSC) are "too quick" for
        // the sequential consumer (the returned receiver-iterator), then the channel buffer can
        // grow arbitrarily large and use up all memory.
        // 2. API limitation of std::mpsc: Only the SyncSender type (of sync_channel) is Sync, i.e.,
        // can be shared easily across all worker threads of the for_each below. (But that could be
        // alleviated with a different MPSC, e.g., crossbeam or using rayon's for_each_with...)
        //
        // Regarding buffer size: when the buffer is the same size as the number of rayon workers,
        // at least no thread has to wait for the sequential consumer in the beginning.
        // I am not sure if it makes sense to increase the buffer size further, that probably
        // depends on how different each work piece for each item is.
        let (sender, receiver) = mpsc::sync_channel(rayon::current_num_threads());

        // The call to sender.send() below will block if the channel buffer is full. In order to not
        // deadlock before starting the receiving end (in the last line), we must start the senders
        // delayed on a separate thread.
        //
        // Originally, we had executed self.try_for_each() in a rayon::spawn thread, but the problem
        // is that this enforces a 'static lifetime on self (i.e., the input ParallelIterator)
        // This makes into_seq_iter() very inflexible, because then no operation on the parallel
        // iterator can make any borrows to local variables, not even if they outlife the sequential
        // collector. In general, rayon::spawn is correct to enforce 'static lifetime, because the
        // spawned thread makes no guarantee to join before any particular lifetime (it could run 
        // forever). However, we know a special invariant here, namely that the returned SeqIter 
        // (which contains the mpsc::Receiver) will only finish executing once all items have been
        // received, which implies that all sender.send() calls have finished.
        // 
        // We thus now use the (formely in std) thread::scoped API, which lets us keep an explicit
        // "join guard" inside the sequential iterator. When the sequential iterator is dropped, it
        // blocks on this "sending thread" below.
        //
        // FIXME thread::scoped was removed from std because in general it is unsound, which is why
        // I am using this "backup crate" thread_scoped. I hope the usage here is safe?
        let _join_senders: thread_scoped::JoinGuard<'a, ()> = unsafe { 
                thread_scoped::scoped(move || {
                // ParallelIterator::for_each is a consuming/blocking operation, i.e., it will not 
                // return until all items of the parallel iterator are processed. Hence, it must be
                // on a separate thread.
                // We use try_ variant to stop early (and avoid needless work) if sending fails
                // because the receiver has already hung up.
                self.try_for_each(|item| {
                    // Sending fails if the receiver has already hung-up (e.g., the returned iterator
                    // was not fully consumed or dropped too early). This will cause computation 
                    // results to go unused, but there is not much we can do about it.
                    sender.send(item).ok()
                });
            })
        };

        // Since the receiver iterator will block until all senders have hung up, consuming this
        // iterator fully means all items of the parallel iterator were processed.
        SeqIter { receiver, _join_senders }
    }
}

impl<'a, P: ParallelIterator + 'a> IntoSeqIter<'a> for P {}
