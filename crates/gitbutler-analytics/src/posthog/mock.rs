use async_trait::async_trait;
use tracing::instrument;

#[derive(Default)]
pub struct Client;

#[async_trait]
impl super::Client for Client {
    #[instrument(skip(self), level = "debug")]
    async fn capture(&self, _events: &[super::Event]) -> Result<(), super::Error> {
        Ok(())
    }
}
