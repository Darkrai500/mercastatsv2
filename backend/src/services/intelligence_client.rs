use std::time::Duration;

use reqwest::{Client, RequestBuilder, StatusCode};
use serde::de::DeserializeOwned;
use serde::Serialize;
use thiserror::Error;
use tokio::time::sleep;

use super::ocr::{OcrProcessTicketRequest, OcrProcessTicketResponse};

#[derive(Clone)]
pub struct IntelligenceClient {
    http: Client,
    base_url: String,
    api_key: Option<String>,
    max_retries: u32,
}

#[derive(Debug, Error)]
pub enum IntelligenceClientError {
    #[error("la peticion al servicio de inteligencia supero el timeout")]
    Timeout,
    #[error("servicio de inteligencia no disponible despues de reintentos")]
    ServiceUnavailable,
    #[error("respuesta inesperada {status}: {body}")]
    UnexpectedStatus { status: StatusCode, body: String },
    #[error("no se pudo deserializar la respuesta: {0}")]
    Deserialize(String),
    #[error(transparent)]
    Request(#[from] reqwest::Error),
}

impl IntelligenceClient {
    pub fn new(
        base_url: String,
        api_key: Option<String>,
        timeout_secs: u64,
        max_retries: u32,
    ) -> Result<Self, reqwest::Error> {
        let http = Client::builder()
            .timeout(Duration::from_secs(timeout_secs))
            .build()?;

        Ok(Self {
            http,
            base_url: base_url.trim_end_matches('/').to_string(),
            api_key,
            max_retries,
        })
    }

    pub async fn process_ticket(
        &self,
        request: OcrProcessTicketRequest,
    ) -> Result<OcrProcessTicketResponse, IntelligenceClientError> {
        self.post("/ocr/process", &request).await
    }

    pub async fn health(&self) -> Result<(), IntelligenceClientError> {
        let url = self.url("/health");
        let request = self.http.get(url);
        let request = self.apply_headers(request);

        let response = request.send().await.map_err(|err| {
            if err.is_timeout() {
                IntelligenceClientError::Timeout
            } else {
                IntelligenceClientError::Request(err)
            }
        })?;

        if response.status().is_success() {
            return Ok(());
        }

        Err(IntelligenceClientError::UnexpectedStatus {
            status: response.status(),
            body: response.text().await.unwrap_or_default(),
        })
    }

    async fn post<TRequest, TResponse>(
        &self,
        path: &str,
        body: &TRequest,
    ) -> Result<TResponse, IntelligenceClientError>
    where
        TRequest: Serialize + ?Sized,
        TResponse: DeserializeOwned,
    {
        let url = self.url(path);
        let mut attempt = 0;

        loop {
            let request = self.http.post(&url).json(body);
            let request = self.apply_headers(request);

            match request.send().await {
                Ok(resp) => {
                    if resp.status() == StatusCode::SERVICE_UNAVAILABLE && attempt < self.max_retries
                    {
                        attempt += 1;
                        sleep(Duration::from_millis(200 * attempt as u64)).await;
                        continue;
                    }

                    if !resp.status().is_success() {
                        let status = resp.status();
                        let body = resp.text().await.unwrap_or_default();
                        return Err(IntelligenceClientError::UnexpectedStatus { status, body });
                    }

                    let parsed = resp
                        .json::<TResponse>()
                        .await
                        .map_err(|err| IntelligenceClientError::Deserialize(err.to_string()))?;
                    return Ok(parsed);
                }
                Err(err) => {
                    if err.is_timeout() {
                        return Err(IntelligenceClientError::Timeout);
                    }
                    return Err(IntelligenceClientError::Request(err));
                }
            }
        }
    }

    fn url(&self, path: &str) -> String {
        let normalized = path.trim_start_matches('/');
        format!("{}/{}", self.base_url, normalized)
    }

    fn apply_headers(&self, builder: RequestBuilder) -> RequestBuilder {
        if let Some(ref api_key) = self.api_key {
            builder.header("x-api-key", api_key)
        } else {
            builder
        }
    }
}
