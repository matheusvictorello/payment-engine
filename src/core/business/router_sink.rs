use futures::Sink;
use std::pin::Pin;
use std::task::{Context, Poll};

#[derive(Clone)]
pub struct RouterSink<K, F> {
    sinks: Vec<K>,
    selector: F,
}

impl<K, F> RouterSink<K, F> {
    pub fn new(sinks: Vec<K>, selector: F) -> Self {
        Self { sinks, selector }
    }
}

impl<K, I, F> Sink<I> for RouterSink<K, F>
where
    K: Sink<I> + Unpin,
    F: Fn(&I) -> usize + Unpin,
{
    type Error = K::Error;

    fn poll_ready(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        // Ensure all sinks are ready
        let this = self.get_mut();
        for sink in this.sinks.iter_mut() {
            match Pin::new(sink).poll_ready(cx) {
                Poll::Ready(Ok(())) => continue,
                Poll::Ready(Err(e)) => return Poll::Ready(Err(e)),
                Poll::Pending => return Poll::Pending,
            }
        }
        Poll::Ready(Ok(()))
    }

    fn start_send(self: Pin<&mut Self>, item: I) -> Result<(), Self::Error> {
        let this = self.get_mut();
        let idx = (this.selector)(&item);
        let sink = this
            .sinks
            .get_mut(idx)
            .expect("selector returned out-of-range index");
        Pin::new(sink).start_send(item)
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        let this = self.get_mut();
        for sink in this.sinks.iter_mut() {
            match Pin::new(sink).poll_flush(cx) {
                Poll::Ready(Ok(())) => continue,
                Poll::Ready(Err(e)) => return Poll::Ready(Err(e)),
                Poll::Pending => return Poll::Pending,
            }
        }
        Poll::Ready(Ok(()))
    }

    fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        let this = self.get_mut();
        for sink in this.sinks.iter_mut() {
            match Pin::new(sink).poll_close(cx) {
                Poll::Ready(Ok(())) => continue,
                Poll::Ready(Err(e)) => return Poll::Ready(Err(e)),
                Poll::Pending => return Poll::Pending,
            }
        }
        Poll::Ready(Ok(()))
    }
}
