use reqwest::{RequestBuilder, Response};

pub struct Create {
    pub email: String,
}

#[async_trait::async_trait]
impl super::Request for Create {
    type Output = ();
    const URL_SUFFIX: &'static str = "/api/account/create";

    fn make_req(&self, req: RequestBuilder) -> anyhow::Result<RequestBuilder> {
        Ok(
            req.json(&sms3_shared::account::handle::AccountCreateDescriptor {
                email: self.email.parse()?,
            }),
        )
    }

    async fn parse_res(&mut self, _response: Response) -> anyhow::Result<Self::Output> {
        Ok(())
    }
}

pub struct Activate {
    pub email: String,
    pub name: String,
    pub school_id: u32,
    pub phone: u64,
    pub house: Option<crate::House>,
    pub org: Option<String>,
    pub password: String,
    pub verify_code: u32,
}

#[async_trait::async_trait]
impl super::Request for Activate {
    type Output = ();
    const URL_SUFFIX: &'static str = "/api/account/verify";

    fn make_req(&self, req: reqwest::RequestBuilder) -> anyhow::Result<reqwest::RequestBuilder> {
        Ok(
            req.json(&sms3_shared::account::handle::AccountVerifyDescriptor {
                code: self.verify_code,
                variant: sms3_shared::account::handle::AccountVerifyVariant::Activate {
                    email: self.email.parse()?,
                    name: self.name.clone(),
                    id: self.school_id,
                    phone: self.phone,
                    house: self.house,
                    organization: self.org.clone(),
                    password: self.password.clone(),
                },
            }),
        )
    }

    async fn parse_res(&mut self, _response: reqwest::Response) -> anyhow::Result<Self::Output> {
        Ok(())
    }
}

pub struct ResetPasswordActivate {
    pub email: String,
    pub password: String,
    pub verify_code: u32,
}

#[async_trait::async_trait]
impl super::Request for ResetPasswordActivate {
    type Output = ();
    const URL_SUFFIX: &'static str = "/api/account/verify";

    fn make_req(&self, req: reqwest::RequestBuilder) -> anyhow::Result<reqwest::RequestBuilder> {
        Ok(
            req.json(&sms3_shared::account::handle::AccountVerifyDescriptor {
                code: self.verify_code,
                variant: sms3_shared::account::handle::AccountVerifyVariant::ResetPassword {
                    email: self.email.parse()?,
                    password: self.password.clone(),
                },
            }),
        )
    }

    async fn parse_res(&mut self, _response: reqwest::Response) -> anyhow::Result<Self::Output> {
        Ok(())
    }
}

pub struct Login {
    pub email: String,
    pub password: String,
}

#[async_trait::async_trait]
impl super::Request for Login {
    /// AccountId, Token
    type Output = (u64, String);
    const URL_SUFFIX: &'static str = "/api/account/login";

    fn make_req(&self, req: RequestBuilder) -> anyhow::Result<RequestBuilder> {
        Ok(
            req.json(&sms3_shared::account::handle::AccountLoginDescriptor {
                email: self.email.parse()?,
                password: self.password.clone(),
            }),
        )
    }

    async fn parse_res(&mut self, response: Response) -> anyhow::Result<Self::Output> {
        #[derive(serde::Deserialize)]
        struct ResponseBody {
            account_id: u64,
            token: String,
        }

        Ok(response
            .json::<ResponseBody>()
            .await
            .map(|value| (value.account_id, value.token))?)
    }
}

pub struct Logout<'a> {
    pub account_info: &'a crate::AccoutInfo,
}

#[async_trait::async_trait]
impl super::Request for Logout<'_> {
    type Output = ();
    const URL_SUFFIX: &'static str = "/api/account/logout";

    fn make_req(&self, req: RequestBuilder) -> anyhow::Result<RequestBuilder> {
        Ok(req.headers(self.account_info.into()))
    }

    async fn parse_res(&mut self, _response: Response) -> anyhow::Result<Self::Output> {
        Ok(())
    }
}

pub struct SignOut<'a> {
    pub account_info: &'a crate::AccoutInfo,
    pub password: String,
}

#[async_trait::async_trait]
impl super::Request for SignOut<'_> {
    type Output = ();
    const URL_SUFFIX: &'static str = "/api/account/signout";

    fn make_req(&self, req: RequestBuilder) -> anyhow::Result<RequestBuilder> {
        Ok(req.headers(self.account_info.into()).json(
            &sms3_shared::account::handle::AccountSignOutDescriptor {
                password: self.password.clone(),
            },
        ))
    }

    async fn parse_res(&mut self, _response: Response) -> anyhow::Result<Self::Output> {
        Ok(())
    }
}

pub struct View<'a> {
    pub account_info: &'a crate::AccoutInfo,
}

#[async_trait::async_trait]
impl super::Request for View<'_> {
    type Output = crate::User;
    const URL_SUFFIX: &'static str = "/api/account/view";

    fn make_req(&self, req: RequestBuilder) -> anyhow::Result<RequestBuilder> {
        Ok(req.headers(self.account_info.into()))
    }

    async fn parse_res(&mut self, response: Response) -> anyhow::Result<Self::Output> {
        let res: sms3_shared::account::handle::ViewAccountResult = response.json().await?;
        assert_eq!(res.id, self.account_info.user.id);

        Ok((&res).into())
    }
}

pub struct Edit<'a> {
    pub account_info: &'a crate::AccoutInfo,
    pub actions: &'a [sms3_shared::account::handle::AccountEditVariant],
}

#[async_trait::async_trait]
impl super::Request for Edit<'_> {
    type Output = ();
    const URL_SUFFIX: &'static str = "/api/account/edit";

    fn make_req(&self, req: RequestBuilder) -> anyhow::Result<RequestBuilder> {
        Ok(req.headers(self.account_info.into()).json(
            &sms3_shared::account::handle::AccountEditDescriptor {
                variants: self.actions.to_vec(),
            },
        ))
    }

    async fn parse_res(&mut self, _response: Response) -> anyhow::Result<Self::Output> {
        Ok(())
    }
}

pub struct ResetPasswordReq {
    pub email: String,
}

#[async_trait::async_trait]
impl super::Request for ResetPasswordReq {
    type Output = ();
    const URL_SUFFIX: &'static str = "/api/account/reset-password";

    fn make_req(&self, req: RequestBuilder) -> anyhow::Result<RequestBuilder> {
        Ok(
            req.json(&sms3_shared::account::handle::ResetPasswordDescriptor {
                email: self.email.parse()?,
            }),
        )
    }

    async fn parse_res(&mut self, _response: Response) -> anyhow::Result<Self::Output> {
        Ok(())
    }
}

pub struct ResetPasswordAct {
    pub email: String,
    pub password: String,
    pub verify_code: u32,
}

#[async_trait::async_trait]
impl super::Request for ResetPasswordAct {
    type Output = ();
    const URL_SUFFIX: &'static str = "/api/account/verify";

    fn make_req(&self, req: RequestBuilder) -> anyhow::Result<RequestBuilder> {
        Ok(
            req.json(&sms3_shared::account::handle::AccountVerifyDescriptor {
                code: self.verify_code,
                variant: sms3_shared::account::handle::AccountVerifyVariant::ResetPassword {
                    email: self.email.parse()?,
                    password: self.password.to_owned(),
                },
            }),
        )
    }

    async fn parse_res(&mut self, _response: Response) -> anyhow::Result<Self::Output> {
        Ok(())
    }
}
