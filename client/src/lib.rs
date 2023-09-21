mod raw;

pub use sms3_shared::account::{House, Permission};

pub mod raw_shared {
    pub use sms3_shared::*;
}

pub struct AccoutInfo {
    email: String,
    token: Option<String>,
    user: LazyUser,
}

pub struct User {
    email: String,
    name: String,
    school_id: u32,
    phone: u64,
    house: Option<House>,
    org: Option<String>,
    permissions: Vec<Permission>,
    registration_time: String,
}

pub struct Post {
    images: Vec<u64>,
    title: String,
    archived: bool,
    ext: Option<PostExt>,
}

struct PostExt {
    description: String,
    time: std::ops::RangeInclusive<chrono::NaiveDate>,
    publisher: LazyUser,
    status: Vec<PostAccept>,
}

pub struct PostAccept {
    operator: LazyUser,
    status: sms3_shared::post::PostAcceptationStatus,
    time: chrono::DateTime<chrono::Utc>,
}

type UserWrap = std::sync::Arc<tokio::sync::RwLock<anyhow::Result<User>>>;

struct LazyUser {
    id: u64,
    user: std::sync::OnceLock<UserWrap>,
}

impl LazyUser {
    #[inline]
    pub fn new(id: u64) -> Self {
        Self {
            id,
            user: std::sync::OnceLock::new(),
        }
    }

    #[inline]
    pub fn id(&self) -> u64 {
        self.id
    }

    pub async fn try_get(
        &self,
        cx: &Context,
    ) -> tokio::sync::RwLockReadGuard<anyhow::Result<User>> {
        self.user
            .get_or_init(|| {
                if let Some(user) = cx.user_map.get(&self.id) {
                    std::sync::Arc::clone(user.value())
                } else {
                    todo!()
                }
            })
            .read()
            .await
    }

    async fn get_raw_user(&self, cx: &Context) -> anyhow::Result<User> {
        if cx.account().user.id == self.id {
            raw::call(
                raw::account::View {
                    account_info: cx.account(),
                },
                cx,
            )
            .await
        } else {
            let mut map = std::collections::HashMap::new();
            map.insert(self.id, None);

            raw::call(
                raw::account_manage::View {
                    account_info: cx.account(),
                    map: &mut map,
                },
                cx,
            )
            .await?;

            map.remove(&self.id)
                .unwrap()
                .ok_or_else(|| anyhow::anyhow!("account not got"))
                .and_then(std::convert::identity)
        }
    }
}

pub struct Context {
    user_map: dashmap::DashMap<u64, UserWrap>,
    req_client: reqwest::Client,
    url_prefix: String,
    account: Option<AccoutInfo>,
}

impl Context {
    #[inline]
    pub fn new(url_prefix: &str) -> Self {
        Self {
            user_map: dashmap::DashMap::new(),
            req_client: reqwest::Client::builder()
                .https_only(true)
                .build()
                .unwrap_or_default(),
            url_prefix: url_prefix.to_owned(),
            account: None,
        }
    }

    #[inline]
    pub fn account(&self) -> &AccoutInfo {
        self.account
            .as_ref()
            .expect("trying to get account info when not logged in")
    }
}
