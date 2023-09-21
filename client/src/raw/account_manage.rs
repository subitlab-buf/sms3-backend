use reqwest::{RequestBuilder, Response};

pub struct CreateUser<'a> {
    pub account_info: &'a crate::AccoutInfo,
    pub email: String,
    pub name: String,
    pub school_id: u32,
    pub phone: u64,
    pub house: Option<crate::House>,
    pub org: Option<String>,
    pub password: String,
    pub permissions: Vec<crate::Permission>,
}

#[async_trait::async_trait]
impl super::Request for CreateUser<'_> {
    type Output = u64;
    const URL_SUFFIX: &'static str = "/api/account/manage/create";

    fn make_req(&self, req: RequestBuilder) -> anyhow::Result<RequestBuilder> {
        Ok(req.headers(self.account_info.into()).json(
            &sms3_shared::account::handle::manage::MakeAccountDescriptor {
                email: self.email.parse()?,
                name: self.name.to_owned(),
                school_id: self.school_id,
                phone: self.phone,
                house: self.house,
                organization: self.org.to_owned(),
                password: self.password.to_owned(),
                permissions: self.permissions.to_vec(),
            },
        ))
    }

    async fn parse_res(&mut self, response: Response) -> anyhow::Result<Self::Output> {
        #[derive(serde::Deserialize)]
        struct Res {
            account_id: u64,
        }

        Ok(response.json::<Res>().await?.account_id)
    }
}

pub struct View<'a> {
    pub account_info: &'a crate::AccoutInfo,
    pub map: &'a mut std::collections::HashMap<u64, Option<anyhow::Result<crate::User>>>,
}

#[async_trait::async_trait]
impl super::Request for View<'_> {
    type Output = ();
    const URL_SUFFIX: &'static str = "/api/account/manage/view";

    fn make_req(&self, req: RequestBuilder) -> anyhow::Result<RequestBuilder> {
        Ok(req.headers(self.account_info.into()).json(
            &sms3_shared::account::handle::manage::ViewAccountDescriptor {
                accounts: self.map.keys().into_iter().cloned().collect(),
            },
        ))
    }

    async fn parse_res(&mut self, response: Response) -> anyhow::Result<Self::Output> {
        #[derive(serde::Deserialize)]
        struct Res {
            results: Vec<sms3_shared::account::handle::manage::ViewAccountResult>,
        }

        for result in response.json::<Res>().await?.results {
            match result {
                sms3_shared::account::handle::manage::ViewAccountResult::Err { id, error } => {
                    self.map.insert(id, Some(Err(anyhow::anyhow!(error))));
                }
                sms3_shared::account::handle::manage::ViewAccountResult::Ok(ref value) => {
                    self.map.insert(value.id, Some(Ok(value.into())));
                }
            }
        }

        Ok(())
    }
}

pub struct Modify<'a> {
    pub account_info: &'a crate::AccoutInfo,
    pub target_account_id: u64,
    pub actions: &'a [sms3_shared::account::handle::manage::AccountModifyVariant],
}

#[async_trait::async_trait]
impl super::Request for Modify<'_> {
    type Output = ();
    const URL_SUFFIX: &'static str = "/api/account/manage/view";

    fn make_req(&self, req: RequestBuilder) -> anyhow::Result<RequestBuilder> {
        Ok(req.headers(self.account_info.into()).json(
            &sms3_shared::account::handle::manage::AccountModifyDescriptor {
                account_id: self.target_account_id,
                variants: self.actions.to_vec(),
            },
        ))
    }

    async fn parse_res(&mut self, _response: Response) -> anyhow::Result<Self::Output> {
        Ok(())
    }
}
