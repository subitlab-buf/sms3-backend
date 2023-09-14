use std::fmt::{Formatter, Write};

mod account;

pub trait Request {
    type Output;

    const URL_SUFFIX: &'static str;
    const METHOD: reqwest::Method = reqwest::Method::POST;

    fn make_req(&self, req: reqwest::RequestBuilder)
        -> anyhow::Result<reqwest::RequestBuilder>;

    fn parse_res(&self, response: reqwest::Response) -> anyhow::Result<Self::Output>;
}

/// Calls a [`Request`] and return its output.
pub async fn call<T: Request>(
    req: T,
    client: &reqwest::Client,
    url_prefix: &str,
) -> anyhow::Result<<T as Request>::Output> {
    let response = req
        .make_req(client.request(T::METHOD, format!("{url_prefix}{}", T::URL_SUFFIX)))?
        .send()
        .await?;
    let status = response.status();

    if !status.is_success() {
        #[derive(Debug)]
        struct ResponseError {
            status_code: reqwest::StatusCode,
            error: Option<String>,
        }

        impl std::fmt::Display for ResponseError {
            fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
                f.write_str(self.status_code.as_str())?;

                if let Some(msg) = self.status_code.canonical_reason() {
                    f.write_char(' ')?;
                    f.write_str(msg)?;
                }

                if let Some(ref msg) = self.error {
                    f.write_str(": ")?;
                    f.write_str(msg)?;
                }

                Ok(())
            }
        }

        impl std::error::Error for ResponseError {}

        #[derive(serde::Deserialize)]
        struct ThrownError {
            error: String,
        }

        let err_msg = response
            .json::<ThrownError>()
            .await
            .ok()
            .map(|msg| msg.error);

        return Err(anyhow::Error::new(ResponseError {
            status_code: status,
            error: err_msg,
        }));
    }

    req.parse_res(response)
}
