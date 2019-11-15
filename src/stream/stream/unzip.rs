use std::future::Future;
use std::pin::Pin;

use pin_project_lite::pin_project;

use crate::stream::Stream;
use crate::task::{Context, Poll};

pin_project! {
    #[cfg(all(feature = "default", feature = "unstable"))]
    #[cfg_attr(feature = "docs", doc(cfg(unstable)))]
    pub struct UnzipFuture<S: Stream, FromA, FromB> {
        #[pin]
        stream: S,
        res: (FromA, FromB),
    }
}

impl<S: Stream, FromA, FromB> UnzipFuture<S, FromA, FromB>
where
    FromA: Default,
    FromB: Default,
{
    pub(super) fn new(stream: S) -> Self {
        UnzipFuture {
            stream,
            res: (FromA::default(), FromB::default()),
        }
    }
}

impl<S, A, B, FromA, FromB> Future for UnzipFuture<S, FromA, FromB>
where
    S: Stream<Item = (A, B)>,
    FromA: Default + Extend<A> + Copy,
    FromB: Default + Extend<B> + Copy,
{
    type Output = (FromA, FromB);

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut this = self.project();
        let next = futures_core::ready!(this.stream.as_mut().poll_next(cx));

        match next {
            Some((a, b)) => {
                this.res.0.extend(Some(a));
                this.res.1.extend(Some(b));
                Poll::Pending
            }
            None => Poll::Ready(*this.res),
        }
    }
}
