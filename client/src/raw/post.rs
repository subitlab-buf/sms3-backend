use reqwest::{RequestBuilder, Response};

pub struct CacheImg<'a> {
    pub account_info: &'a crate::AccoutInfo,
    pub file: std::sync::Mutex<Option<tokio::fs::File>>,
}

#[async_trait::async_trait]
impl super::Request for CacheImg<'_> {
    type Output = u64;
    const URL_SUFFIX: &'static str = "/api/post/upload-image";

    fn make_req(&self, req: RequestBuilder) -> anyhow::Result<RequestBuilder> {
        Ok(req.headers(self.account_info.into()).body(
            self.file
                .lock()
                .unwrap()
                .take()
                .ok_or_else(|| anyhow::anyhow!("file should not be none"))?,
        ))
    }

    async fn parse_res(&mut self, response: Response) -> anyhow::Result<Self::Output> {
        #[derive(serde::Deserialize)]
        struct Res {
            hash: u64,
        }

        Ok(response.json::<Res>().await?.hash)
    }
}

pub struct GetImg<'a> {
    pub account_info: &'a crate::AccoutInfo,
    pub hash: u64,
}

#[async_trait::async_trait]
impl super::Request for GetImg<'_> {
    type Output = bytes::Bytes;
    const URL_SUFFIX: &'static str = "/api/post/get-image";

    fn make_req(&self, req: RequestBuilder) -> anyhow::Result<RequestBuilder> {
        Ok(req
            .headers(self.account_info.into())
            .json(&sms3rs_shared::post::handle::GetImageDescriptor { hash: self.hash }))
    }

    async fn parse_res(&mut self, response: Response) -> anyhow::Result<Self::Output> {
        Ok(response.bytes().await?)
    }
}
