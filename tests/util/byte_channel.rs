use std::cmp::min;
use std::collections::VecDeque;
use std::pin::Pin;
use std::task::{Context, Poll, Waker};

use futures::{AsyncRead, AsyncWrite};

#[derive(Debug, Default)]
pub struct ByteChannel {
    queue: VecDeque<u8>,
    waker: Option<Waker>,
    closed: bool,
}

impl ByteChannel {
    pub fn new() -> Self {
        Self::default()
    }
}

impl AsyncRead for ByteChannel {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<std::io::Result<usize>> {
        if self.queue.is_empty() && self.closed {
            return Poll::Ready(Ok(0));
        }
        let num_read = min(self.queue.len(), buf.len());
        if num_read == 0 {
            self.waker = Some(cx.waker().clone());
            return Poll::Pending;
        }
        buf.iter_mut()
            .zip(self.queue.drain(0..num_read))
            .for_each(|(dst, src)| *dst = src);
        Poll::Ready(Ok(num_read))
    }
}

impl AsyncWrite for ByteChannel {
    fn poll_write(
        mut self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        self.queue.extend(buf.iter());
        if let Some(waker) = self.waker.take() {
            waker.wake();
        }
        Poll::Ready(Ok(buf.len()))
    }

    fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Poll::Ready(Ok(()))
    }

    fn poll_close(mut self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        self.closed = true;
        if let Some(waker) = self.waker.take() {
            waker.wake();
        }
        Poll::Ready(Ok(()))
    }
}

#[cfg(test)]
mod tests {
    use futures::future::join;
    use futures::io::{AsyncReadExt, AsyncWriteExt};

    use super::*;

    #[tokio::test]
    async fn test_write_then_read() {
        let channel = ByteChannel::new();
        let (mut reader, mut writer) = channel.split();

        let mut read_buf = [0u8; 3];
        writer.write_all(&[1, 2, 3, 4]).await.unwrap();
        assert_eq!(reader.read(&mut read_buf).await.unwrap(), 3);
        assert_eq!(&read_buf, &[1, 2, 3]);

        writer.write_all(&[5, 6]).await.unwrap();
        assert_eq!(reader.read(&mut read_buf).await.unwrap(), 3);
        assert_eq!(&read_buf, &[4, 5, 6]);

        writer.close().await.unwrap();
        assert_eq!(reader.read(&mut read_buf).await.unwrap(), 0);
    }

    #[tokio::test]
    async fn test_read_then_write() {
        let channel = ByteChannel::new();
        let (mut reader, mut writer) = channel.split();

        join(
            async {
                let mut read_buf = [0u8; 3];
                assert_eq!(reader.read(&mut read_buf).await.unwrap(), 3);
                assert_eq!(&read_buf, &[1, 2, 3]);
            },
            async {
                writer.write_all(&[1, 2, 3, 4]).await.unwrap();
            },
        )
        .await;
    }

    #[tokio::test]
    async fn test_read_then_close() {
        let channel = ByteChannel::new();
        let (mut reader, mut writer) = channel.split();

        join(
            async {
                let mut read_buf = [0u8; 3];
                assert_eq!(reader.read(&mut read_buf).await.unwrap(), 0);
                assert_eq!(&read_buf, &[0, 0, 0]);
            },
            async {
                writer.close().await.unwrap();
            },
        )
        .await;
    }

    #[tokio::test]
    async fn test_close_then_read() {
        let channel = ByteChannel::new();
        let (mut reader, mut writer) = channel.split();

        writer.write_all(&[1, 2, 3]).await.unwrap();
        writer.close().await.unwrap();

        // should still read bytes from queue
        let mut read_buf = [0u8; 3];
        assert_eq!(reader.read(&mut read_buf).await.unwrap(), 3);
        assert_eq!(&read_buf, &[1, 2, 3]);
        // should read EOF
        assert_eq!(reader.read(&mut read_buf).await.unwrap(), 0);
    }
}
