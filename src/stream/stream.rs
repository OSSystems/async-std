//! Asynchronous iteration.
//!
//! This module is an async version of [`std::iter`].
//!
//! [`std::iter`]: https://doc.rust-lang.org/std/iter/index.html
//!
//! # Examples
//!
//! ```
//! # fn main() { async_std::task::block_on(async {
//! #
//! use async_std::prelude::*;
//! use async_std::stream;
//!
//! let mut s = stream::repeat(9).take(3);
//!
//! while let Some(v) = s.next().await {
//!     assert_eq!(v, 9);
//! }
//! #
//! # }) }
//! ```

use std::pin::Pin;

use cfg_if::cfg_if;

use crate::future::Future;
use crate::task::{Context, Poll};
use std::marker::PhantomData;

cfg_if! {
    if #[cfg(feature = "docs")] {
        #[doc(hidden)]
        pub struct ImplFuture<'a, T>(std::marker::PhantomData<&'a T>);

        macro_rules! ret {
            ($a:lifetime, $f:tt, $o:ty) => (ImplFuture<$a, $o>);
        }
    } else {
        macro_rules! ret {
            ($a:lifetime, $f:tt, $o:ty) => ($f<$a, Self>);
        }
    }
}

/// An asynchronous stream of values.
///
/// This trait is an async version of [`std::iter::Iterator`].
///
/// While it is currently not possible to implement this trait directly, it gets implemented
/// automatically for all types that implement [`futures::stream::Stream`].
///
/// [`std::iter::Iterator`]: https://doc.rust-lang.org/std/iter/trait.Iterator.html
/// [`futures::stream::Stream`]:
/// https://docs.rs/futures-preview/0.3.0-alpha.17/futures/stream/trait.Stream.html
pub trait Stream {
    /// The type of items yielded by this stream.
    type Item;

    /// Advances the stream and returns the next value.
    ///
    /// Returns [`None`] when iteration is finished. Individual stream implementations may
    /// choose to resume iteration, and so calling `next()` again may or may not eventually
    /// start returning more values.
    ///
    /// [`None`]: https://doc.rust-lang.org/std/option/enum.Option.html#variant.None
    ///
    /// # Examples
    ///
    /// ```
    /// # fn main() { async_std::task::block_on(async {
    /// #
    /// use async_std::prelude::*;
    /// use async_std::stream;
    ///
    /// let mut s = stream::once(7);
    ///
    /// assert_eq!(s.next().await, Some(7));
    /// assert_eq!(s.next().await, None);
    /// #
    /// # }) }
    /// ```
    fn next<'a>(&'a mut self) -> ret!('a, NextFuture, Option<Self::Item>)
    where
        Self: Unpin;

    /// Creates a stream that yields its first `n` elements.
    ///
    /// # Examples
    ///
    /// ```
    /// # fn main() { async_std::task::block_on(async {
    /// #
    /// use async_std::prelude::*;
    /// use async_std::stream;
    ///
    /// let mut s = stream::repeat(9).take(3);
    ///
    /// while let Some(v) = s.next().await {
    ///     assert_eq!(v, 9);
    /// }
    /// #
    /// # }) }
    /// ```
    fn take(self, n: usize) -> Take<Self>
    where
        Self: Sized,
    {
        Take {
            stream: self,
            remaining: n,
        }
    }

    /// Tests if every element of the stream matches a predicate.
    ///
    /// `all()` takes a closure that returns `true` or `false`. It applies
    /// this closure to each element of the stream, and if they all return
    /// `true`, then so does `all()`. If any of them return `false`, it
    /// returns `false`.
    ///
    /// `all()` is short-circuiting; in other words, it will stop processing
    /// as soon as it finds a `false`, given that no matter what else happens,
    /// the result will also be `false`.
    ///
    /// An empty stream returns `true`.
    ///
    /// # Examples
    ///
    /// Basic usage:
    ///
    /// ```
    /// # fn main() { async_std::task::block_on(async {
    /// #
    /// use async_std::prelude::*;
    /// use async_std::stream;
    ///
    /// let mut s = stream::repeat::<u32>(42).take(3);
    /// assert!(s.all(|x| x ==  42).await);
    ///
    /// #
    /// # }) }
    /// ```
    ///
    /// Empty stream:
    ///
    /// ```
    /// # fn main() { async_std::task::block_on(async {
    /// #
    /// use async_std::prelude::*;
    /// use async_std::stream;
    ///
    /// let mut s = stream::empty::<u32>();
    /// assert!(s.all(|_| false).await);
    ///
    /// #
    /// # }) }
    /// ```
    /// Stopping at the first `false`:
    ///
    /// TODO: add example here
    /// TODO: add more examples
    #[inline]
    fn all<F>(&mut self, f: F) -> AllFuture<'_, Self, F, Self::Item>
    where
        Self: Sized,
        F: FnMut(Self::Item) -> bool,
    {
        AllFuture {
            stream: self,
            result: true,
            __item: PhantomData,
            f,
        }
    }
}

impl<T: futures::Stream + Unpin + ?Sized> Stream for T {
    type Item = <Self as futures::Stream>::Item;

    fn next<'a>(&'a mut self) -> ret!('a, NextFuture, Option<Self::Item>)
    where
        Self: Unpin,
    {
        NextFuture { stream: self }
    }
}

#[doc(hidden)]
#[allow(missing_debug_implementations)]
pub struct NextFuture<'a, T: Unpin + ?Sized> {
    stream: &'a mut T,
}

impl<T: futures::Stream + Unpin + ?Sized> Future for NextFuture<'_, T> {
    type Output = Option<T::Item>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        Pin::new(&mut *self.stream).poll_next(cx)
    }
}

/// A stream that yields the first `n` items of another stream.
#[derive(Clone, Debug)]
pub struct Take<S> {
    stream: S,
    remaining: usize,
}

impl<S: Unpin> Unpin for Take<S> {}

impl<S: futures::Stream> Take<S> {
    pin_utils::unsafe_pinned!(stream: S);
    pin_utils::unsafe_unpinned!(remaining: usize);
}

impl<S: futures::Stream> futures::Stream for Take<S> {
    type Item = S::Item;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<S::Item>> {
        if self.remaining == 0 {
            Poll::Ready(None)
        } else {
            let next = futures::ready!(self.as_mut().stream().poll_next(cx));
            match next {
                Some(_) => *self.as_mut().remaining() -= 1,
                None => *self.as_mut().remaining() = 0,
            }
            Poll::Ready(next)
        }
    }
}

#[derive(Debug)]
pub struct AllFuture<'a, S, F, T>
where
    F: FnMut(T) -> bool,
{
    stream: &'a mut S,
    f: F,
    result: bool,
    __item: PhantomData<T>,
}

impl<'a, S, F, T> AllFuture<'a, S, F, T>
where
    F: FnMut(T) -> bool,
{
    pin_utils::unsafe_pinned!(stream: &'a mut S);
    pin_utils::unsafe_unpinned!(result: bool);
    pin_utils::unsafe_unpinned!(f: F);
}

impl<S, F> Future for AllFuture<'_, S, F, S::Item>
where
    S: futures::Stream + Unpin + Sized,
    F: FnMut(S::Item) -> bool,
{
    type Output = bool;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        use futures::Stream;
        let next = futures::ready!(self.as_mut().stream().poll_next(cx));
        match next {
            Some(v) => {
                let result = (self.as_mut().f())(v);
                *self.as_mut().result() = result;
                if result {
                    // don't forget to wake this task again to pull the next item from stream
                    cx.waker().wake_by_ref();
                    Poll::Pending
                } else {
                    Poll::Ready(false)
                }
            }
            None => Poll::Ready(self.result),
        }
    }
}
