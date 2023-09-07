pub mod handle;
pub mod verify;

use chrono::{DateTime, Duration, Utc};
use dashmap::DashMap;
use once_cell::sync::Lazy;
use parking_lot::RwLock;
use rand::Rng;
use serde::{Deserialize, Serialize};
use sha256::digest;
use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
    ops::{Deref, DerefMut},
};

pub use sms3rs_shared::account::*;

/// The static instance of accounts.
pub static INSTANCE: Lazy<AccountManager> = Lazy::new(AccountManager::new);

#[derive(thiserror::Error, Debug)]
pub enum AccountError {
    #[error("verification code not match")]
    VerificationCode,
    #[error("user has not been verified")]
    UserUnverified,
    #[error("user already registered")]
    UserRegistered,
    #[error("password incorrect")]
    PasswordIncorrect,
    #[error("token incorrect")]
    TokenIncorrect,
    #[error("domain of email address is not from PKUSchool")]
    EmailDomainNotInSchool,
    #[error("date out of range")]
    DateOutOfRange,
    #[error("smtp error while sending verification mail: {0}")]
    MailSend(lettre::transport::smtp::Error),
    #[error("permission denied")]
    PermissionDenied,
}

/// Represent an account, including unverified and verified.
#[derive(Serialize, Deserialize, Debug)]
pub enum Account {
    /// An unverified account.
    Unverified(verify::Context),

    /// A normal user.
    Verified {
        /// Identifier of this user.
        id: u64,
        /// Attributes of this user.
        attributes: UserAttributes,
        /// This account's token manager.
        tokens: verify::Tokens,
        /// The verify context of this account exists in some conditions (ex. forget password).
        verify: UserVerifyVariant,
    },
}

impl Account {
    /// Create a new unverified account.
    pub async fn new(email: lettre::Address) -> Result<Self, AccountError> {
        if email.domain() != "i.pkuschool.edu.cn" && email.domain() != "pkuschool.edu.cn" {
            return Err(AccountError::EmailDomainNotInSchool);
        }

        Ok(Self::Unverified({
            let cxt = verify::Context {
                email,
                code: {
                    let mut rng = rand::thread_rng();
                    rng.gen_range(100000..999999)
                },
                expire_time: match Utc::now()
                    .naive_utc()
                    .checked_add_signed(Duration::minutes(15))
                {
                    Some(e) => e,
                    _ => return Err(AccountError::DateOutOfRange),
                },
            };
            cxt.send_verify();
            cxt
        }))
    }

    /// Verify this account based on the variant.
    fn verify(
        &mut self,
        verify_code: u32,
        variant: AccountVerifyVariant,
    ) -> Result<(), AccountError> {
        match variant {
            AccountVerifyVariant::Activate(attributes) => {
                if let Self::Unverified(cxt) = self {
                    if cxt.code != verify_code {
                        return Err(AccountError::VerificationCode);
                    }
                    *self = Self::Verified {
                        id: {
                            let mut hasher = DefaultHasher::new();
                            attributes.email.hash(&mut hasher);
                            hasher.finish()
                        },
                        attributes,
                        tokens: verify::Tokens::new(),
                        verify: UserVerifyVariant::None,
                    };
                    Ok(())
                } else {
                    Err(AccountError::UserRegistered)
                }
            }
            AccountVerifyVariant::ResetPassword(password) => {
                if let Self::Verified {
                    attributes, verify, ..
                } = self
                {
                    match verify {
                        UserVerifyVariant::None => Err(AccountError::PermissionDenied),
                        UserVerifyVariant::ForgetPassword(cxt) => {
                            if cxt.code != verify_code {
                                return Err(AccountError::VerificationCode);
                            }
                            attributes.password_sha = digest(password);
                            *verify = UserVerifyVariant::None;
                            Ok(())
                        }
                    }
                } else {
                    Err(AccountError::UserUnverified)
                }
            }
        }
    }

    /// Get the only id of this user.
    pub fn id(&self) -> u64 {
        match self {
            Account::Unverified(cxt) => {
                let mut hasher = DefaultHasher::new();
                cxt.email.hash(&mut hasher);
                hasher.finish()
            }
            Account::Verified { id, .. } => *id,
        }
    }

    /// Get email of this user.
    pub fn email(&self) -> &lettre::Address {
        match self {
            Account::Unverified(cxt) => &cxt.email,
            Account::Verified { attributes, .. } => &attributes.email,
        }
    }

    /// Get metadata of this user.
    pub fn metadata(&self) -> Result<UserMetadata, AccountError> {
        if let Self::Verified { attributes, .. } = self {
            Ok(UserMetadata {
                email: attributes.email.clone(),
                name: attributes.name.clone(),
                school_id: attributes.school_id,
                phone: attributes.phone,
                house: attributes.house,
                organization: attributes.organization.clone(),
            })
        } else {
            Err(AccountError::UserUnverified)
        }
    }

    /// Get all permissions this user has.
    pub fn permissions(&self) -> Permissions {
        match self {
            Account::Unverified(_) => Vec::new(),
            Account::Verified { attributes, .. } => attributes.permissions.clone(),
        }
    }

    /// Indicates whether this user has the target permission.
    pub fn has_permission(&self, permission: Permission) -> bool {
        self.permissions().contains(&permission)
    }

    /// Login into the account and return back a token in a `Result`.
    pub fn login(&mut self, password: &str) -> Result<String, AccountError> {
        match self {
            Account::Unverified(_) => Err(AccountError::UserUnverified),
            Account::Verified {
                id,
                attributes,
                tokens,
                ..
            } => {
                if digest(password) == attributes.password_sha {
                    Ok(tokens.new_token(*id, attributes.token_expiration_time))
                } else {
                    Err(AccountError::PasswordIncorrect)
                }
            }
        }
    }

    /// Logout this account with the target token.
    pub fn logout(&mut self, token: &str) -> Result<(), AccountError> {
        match self {
            Account::Unverified(_) => Err(AccountError::UserUnverified),
            Account::Verified { tokens, .. } => {
                if tokens.remove(token) {
                    Ok(())
                } else {
                    Err(AccountError::TokenIncorrect)
                }
            }
        }
    }

    /// Save this account and return whether if this account was saved successfully.
    pub fn save(&self) {
        #[cfg(not(test))]
        {
            let id = self.id();
            let data = toml::to_string(&self).unwrap_or_default();

            tokio::spawn(async move {
                use tokio::io::AsyncWriteExt;

                let mut file = tokio::fs::File::create(format!("./data/accounts/{}.toml", id))
                    .await
                    .unwrap();
                file.write(data.as_bytes()).await.unwrap();
            });
        }
    }

    /// Remove this account from filesystem and return whether this account was removed successfully.
    pub fn remove(&self) {
        #[cfg(not(test))]
        {
            let id = self.id();

            tokio::spawn(async move {
                tokio::fs::remove_file(format!("./data/accounts/{}.json", id))
                    .await
                    .unwrap()
            });
        }
    }
}

enum AccountVerifyVariant {
    /// Activate an unverified account.
    Activate(UserAttributes),
    /// Reset a forgotten password.
    ResetPassword(String),
}

#[derive(Deserialize, Serialize, Debug)]
pub enum UserVerifyVariant {
    None,
    ForgetPassword(verify::Context),
}

// Attributes of a registered user.
#[derive(Serialize, Deserialize, Debug)]
pub struct UserAttributes {
    /// Email address of this user.
    pub email: lettre::Address,
    /// Name of this user.
    pub name: String,
    /// School id of this user (ex. 2522xxx).
    pub school_id: u32,
    /// Phone number of this user.
    pub phone: u64,
    /// House this student belongs to. Can be `None`.
    pub house: Option<House>,
    /// Organization this user belongs to. Can be `None`.
    pub organization: Option<String>,
    /// Permissions this user has.
    pub permissions: Permissions,
    /// The registration time of this user.
    pub registration_time: DateTime<Utc>,
    /// Hash of this user's password.
    pub password_sha: String,
    /// The expiration time of a token in days.
    /// `0` means never expire.
    pub token_expiration_time: u16,
}

#[derive(thiserror::Error, Debug)]
pub enum AccountManagerError {
    #[error("account {0} errored: {1}")]
    Account(u64, AccountError),
    #[error("account {0} not found")]
    AccountNotFound(u64),
}

/// A simple account manager.
pub struct AccountManager {
    accounts: RwLock<Vec<RwLock<Account>>>,
    /// An index cache for getting index from an id.
    index: DashMap<u64, usize>,
}

impl AccountManager {
    /// Read and create an account manager from `./data/accounts`.
    pub fn new() -> Self {
        #[cfg(not(test))]
        {
            use std::fs::{self, File};
            use std::io::Read;

            let mut vec = Vec::new();
            let index = DashMap::new();
            let mut i = 0;
            for dir in fs::read_dir("./data/accounts").unwrap() {
                if let Ok(e) = dir.map(|e| {
                    toml::from_str::<Account>(&{
                        let mut string = String::new();
                        File::open(e.path())
                            .unwrap()
                            .read_to_string(&mut string)
                            .unwrap();
                        string
                    })
                    .unwrap()
                }) {
                    index.insert(e.id(), i);
                    vec.push(RwLock::new(e));
                    i += 1;
                } else {
                    continue;
                }
            }
            Self {
                accounts: RwLock::new(vec),
                index,
            }
        }

        #[cfg(test)]
        Self {
            accounts: RwLock::new(Vec::new()),
            index: DashMap::new(),
        }
    }

    /// Get inner accounts.
    pub fn inner(&self) -> &RwLock<Vec<RwLock<Account>>> {
        &self.accounts
    }

    /// Get inner indexe cache.
    pub fn index(&self) -> &DashMap<u64, usize> {
        &self.index
    }

    /// Update index cache of this instance.
    pub fn update_index(&self) {
        self.index.clear();
        for account in self.accounts.read().iter().enumerate() {
            self.index.insert(account.1.read().id(), account.0);
        }
    }

    /// Refresh this instance.
    ///
    /// - Remove expired unverified accounts
    /// - Remove expired tokens
    pub fn refresh_all(&self) {
        {
            let mut rm_list: Vec<usize> = Vec::new();
            for account in self.accounts.read().iter().enumerate() {
                {
                    let r_binding = account.1.read();
                    if match r_binding.deref() {
                        Account::Unverified(cxt) => cxt.is_expired(),
                        _ => false,
                    } {
                        rm_list.push(account.0);
                    }
                }
            }
            let mut w = self.accounts.write();
            for i in rm_list.iter().enumerate() {
                w.remove(*i.1 - i.0);
            }
            if !rm_list.is_empty() {
                self.update_index();
            }
        }

        {
            for account in self.accounts.read().iter() {
                let mut w = account.write();
                if let Account::Verified { tokens, verify, .. } = w.deref_mut() {
                    tokens.refresh();
                    if match verify {
                        UserVerifyVariant::None => false,
                        UserVerifyVariant::ForgetPassword(e) => e.is_expired(),
                    } {
                        *verify = UserVerifyVariant::None;
                    }
                }
            }
        }
    }

    /// Refresh target account.
    ///
    /// - Remove expired unverified account;
    /// - Remove expired tokens.
    pub fn refresh(&self, id: u64) {
        if let Some(index) = self.index.get(&id) {
            if let Some(account) = self.accounts.read().get(*index) {
                {
                    if match account.read().deref() {
                        Account::Unverified(cxt) => cxt.is_expired(),
                        _ => false,
                    } {
                        self.remove(id);
                    }
                }
                {
                    match account.write().deref_mut() {
                        Account::Verified { tokens, verify, .. } => {
                            tokens.refresh();
                            if match verify {
                                UserVerifyVariant::None => false,
                                UserVerifyVariant::ForgetPassword(e) => e.is_expired(),
                            } {
                                *verify = UserVerifyVariant::None;
                            }
                        }
                        _ => (),
                    }
                }
            }
        }
    }

    /// Remove target account.
    pub fn remove(&self, id: u64) {
        if let Some(index) = self.index.get(&id) {
            {
                let b = self.accounts.read();
                b.get(*index).unwrap().read().remove();
            }
            self.accounts.write().remove(*index);
        }
        self.update_index();
    }

    /// Push an account to this instance, only for testing.
    #[cfg(test)]
    pub fn push(&self, account: Account) {
        assert!(self
            .index
            .insert(account.id(), self.accounts.read().len())
            .is_none());
        self.accounts.write().push(RwLock::new(account));
    }

    #[cfg(test)]
    pub fn reset(&self) {
        *self.accounts.write().deref_mut() = Vec::new();
        self.index.clear()
    }
}
