use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::Mutex;
use tracing::instrument;

#[derive(Clone)]
pub struct Client<T: super::Client + Sync> {
    inner: T,

    /// Events that failed to be sent
    /// and are waiting to be retried.
    batch: Arc<Mutex<Vec<super::Event>>>,
}

impl<T: super::Client + Sync> Client<T> {
    pub fn new(inner: T) -> Self {
        Client {
            inner,

            batch: Arc::new(Mutex::new(Vec::new())),
        }
    }
}

#[async_trait]
impl<T: super::Client + Sync> super::Client for Client<T> {
    #[instrument(skip(self), level = "debug")]
    async fn capture(&self, events: &[super::Event]) -> Result<(), super::Error> {
        let mut batch = self.batch.lock().await;
        batch.extend_from_slice(events);
        if let Err(error) = self.inner.capture(&batch).await {
            tracing::warn!("Failed to send analytics: {}", error);
        } else {
            batch.clear();
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

    use super::super::Client;

    #[derive(Clone)]
    struct MockClient {
        sent: Arc<AtomicUsize>,
        is_failing: Arc<AtomicBool>,
    }

    impl MockClient {
        fn new() -> Self {
            MockClient {
                sent: Arc::new(AtomicUsize::new(0)),
                is_failing: Arc::new(AtomicBool::new(false)),
            }
        }

        fn set_failing(&self, is_failing: bool) {
            self.is_failing.store(is_failing, Ordering::SeqCst);
        }

        fn get_sent(&self) -> usize {
            self.sent.load(Ordering::SeqCst)
        }
    }

    #[async_trait]
    impl super::super::Client for MockClient {
        async fn capture(&self, events: &[super::super::Event]) -> Result<(), super::super::Error> {
            if self.is_failing.load(Ordering::SeqCst) {
                Err(super::super::Error::BadRequest {
                    code: 400,
                    message: "Bad request".to_string(),
                })
            } else {
                self.sent.fetch_add(events.len(), Ordering::SeqCst);
                Ok(())
            }
        }
    }

    #[tokio::test]
    async fn retry() {
        let inner_client = MockClient::new();
        let retry_client = super::Client::new(inner_client.clone());

        inner_client.set_failing(true);

        retry_client
            .capture(&[super::super::Event::new("test", "test")])
            .await
            .unwrap();
        assert_eq!(inner_client.get_sent(), 0);

        retry_client
            .capture(&[super::super::Event::new("test", "test")])
            .await
            .unwrap();
        assert_eq!(inner_client.get_sent(), 0);

        inner_client.set_failing(false);

        retry_client
            .capture(&[super::super::Event::new("test", "test")])
            .await
            .unwrap();
        assert_eq!(inner_client.get_sent(), 3);

        retry_client
            .capture(&[super::super::Event::new("test", "test")])
            .await
            .unwrap();
        assert_eq!(inner_client.get_sent(), 4);
    }
}
