use chrono::{DateTime};
use sha2::Sha256;


trait Stream {
    /// The type of the value yielded by the stream.
    type Item;

    /// Attempt to resolve the next item in the stream.
    /// Returns `Poll::Pending` if not ready, `Poll::Ready(Some(x))` if a value
    /// is ready, and `Poll::Ready(None)` if the stream has completed.
    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>)
        -> Poll<Option<Self::Item>>;
}

struct DataFragment {
    v: u32, // Defaults to 0
    timestamp: DateTime,
    hash: Sha256,
    live: bool,
    data: Option<>,
}