use reqwest::{RequestBuilder, Response};

/// Creates a new unverified account.
pub struct Create {
    pub email: String,
}

impl super::Request for Create {
    type Output = ();
    const URL_SUFFIX: &'static str = "/api/account/create";

    fn make_req(&self, req: RequestBuilder) -> anyhow::Result<RequestBuilder> {
        Ok(
            req.json(&sms3rs_shared::account::handle::AccountCreateDescriptor {
                email: self.email.clone().parse()?,
            }),
        )
    }

    fn parse_res(&self, _response: Response) -> anyhow::Result<Self::Output> {
        Ok(())
    }
}

pub struct Activate {

}
