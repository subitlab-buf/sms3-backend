mod raw;

use std::sync::Arc;

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
    user: std::cell::UnsafeCell<MaybeUninitUserLazyWrap>,
    lock: std::sync::Mutex<()>,
}

// this should be safe due to mutex
unsafe impl Sync for LazyUser {}

enum MaybeUninitUserLazyWrap {
    Queued(std::sync::Arc<()>),
    Ok(UserWrap),
}

impl LazyUser {
    #[inline]
    pub fn new(id: u64, cx: &Context) -> Self {
        let queued = std::sync::Arc::new(());
        Self {
            id,
            user: std::cell::UnsafeCell::new(MaybeUninitUserLazyWrap::Queued(queued)),
            lock: std::sync::Mutex::new(()),
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
        match unsafe { &mut *self.user.get() } {
            MaybeUninitUserLazyWrap::Queued(_) => self.get_raw_user_and_bump_cx(cx).await,
            MaybeUninitUserLazyWrap::Ok(usr) => usr.read().await,
        }
    }

    async fn get_raw_user_and_bump_cx(&self, cx: &Context) -> anyhow::Result<User> {
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
    account: Option<AccoutInfo>,
    req_client: reqwest::Client,
    url_prefix: String,
    user_map: dashmap::DashMap<u64, (Option<UserWrap>, Option<std::sync::Weak<()>>)>, // user, needs_update
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

    #[inline]
    fn make_uninit_user(&self, id: u64) -> Arc<()> {
        if let Some(mut value) = self.user_map.get_mut(&id) {
            if let Some(arc) = value.1.as_ref().and_then(std::sync::Weak::upgrade) {
                return arc;
            }

            let arc = Arc::new(());
            value.1 = Some(Arc::downgrade(&arc));
            arc
        } else {
            let arc = Arc::new(());
            self.user_map.insert(id, (None, Some(Arc::downgrade(&arc))));
            arc
        }
    }

    #[inline]
    fn user_needs_fetch(&self, id: u64) -> bool {
        self.user_map
            .get(&id)
            .and_then(|value| value.1.as_ref().map(|e| e.strong_count() > 0))
            .unwrap_or(true)
    }
}
