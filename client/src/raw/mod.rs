use std::fmt::{Formatter, Write};

pub mod account;
pub mod account_manage;
pub mod post;

#[async_trait::async_trait]
pub trait Request {
    type Output;

    const URL_SUFFIX: &'static str;
    const METHOD: reqwest::Method = reqwest::Method::POST;

    fn make_req(&self, req: reqwest::RequestBuilder) -> anyhow::Result<reqwest::RequestBuilder>;

    async fn parse_res(&mut self, response: reqwest::Response) -> anyhow::Result<Self::Output>;
}

/// Calls a [`Request`] and return its output.
pub async fn call<T: Request>(
    mut req: T,
    cx: &crate::Context,
) -> anyhow::Result<<T as Request>::Output> {
    let response = req
        .make_req(
            cx.req_client
                .request(T::METHOD, format!("{}{}", cx.url_prefix, T::URL_SUFFIX)),
        )?
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
        #[allow(unused)]
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

    req.parse_res(response).await
}

impl Into<reqwest::header::HeaderMap<reqwest::header::HeaderValue>> for &crate::AccoutInfo {
    fn into(self) -> reqwest::header::HeaderMap<reqwest::header::HeaderValue> {
        let mut map = reqwest::header::HeaderMap::new();

        map.insert(
            "Token",
            self.token
                .clone()
                .expect("token not found")
                .parse()
                .unwrap(),
        );

        map.insert("AccountId", self.user.id().into());

        map
    }
}

impl From<&sms3_shared::account::handle::ViewAccountResult> for super::User {
    fn from(res: &sms3_shared::account::handle::ViewAccountResult) -> Self {
        Self {
            email: res.metadata.email.to_string(),
            name: res.metadata.name.to_owned(),
            school_id: res.metadata.school_id,
            phone: res.metadata.phone,
            house: res.metadata.house,
            org: res.metadata.organization.to_owned(),
            permissions: res.permissions.clone(),
            registration_time: res.registration_time.to_string(),
        }
    }
}
