use crate::{GeneratedMedia, MediaProvider, MediaProviderError, MediaRequest, ProviderFuture};
use story_provider::{MediaGatewayClient, MediaGatewayRoute};

pub struct GatewayMediaProvider {
    client: MediaGatewayClient,
    route: MediaGatewayRoute,
}

impl GatewayMediaProvider {
    pub fn new(client: MediaGatewayClient, route: MediaGatewayRoute) -> Self {
        Self { client, route }
    }
}

impl MediaProvider for GatewayMediaProvider {
    fn generate<'a>(&'a self, request: &'a MediaRequest) -> ProviderFuture<'a> {
        Box::pin(async move {
            let request = match request {
                MediaRequest::Image(value) => serde_json::to_value(value),
                MediaRequest::Video(value) => serde_json::to_value(value),
            }
            .map_err(|_| MediaProviderError::Failed)?;
            let output = self
                .client
                .generate(&self.route, request)
                .await
                .map_err(|_| MediaProviderError::Failed)?;
            Ok(GeneratedMedia {
                mime_type: output.mime_type,
                bytes: output.bytes,
                provider: output.provider,
                model: output.model,
                cost_cny_fen: output.cost_cny_fen,
                pricing_catalog_id: output.pricing_catalog_id,
            })
        })
    }
}
