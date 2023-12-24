use lettre::{transport::smtp, AsyncSmtpTransport};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    /// SMTP configuration.
    pub smtp: Smtp,
}

/// SMTP mailing configuration.
#[derive(Debug, Serialize, Deserialize)]
pub struct Smtp {
    /// The SMTP Server address.
    pub server: String,
    /// The SMTP Server port.
    #[serde(default)]
    pub port: Option<u16>,

    /// The email address.
    pub address: lettre::Address,

    pub username: String,
    pub password: String,

    /// The auth mechanism.
    ///
    /// Serialized and deserialized as `PascalCase`.
    pub auth: Vec<smtp::authentication::Mechanism>,
}

impl Smtp {
    /// Make this configuration to an async smtp transport.
    pub fn to_transport<E>(&self) -> Result<AsyncSmtpTransport<E>, smtp::Error>
    where
        E: lettre::Executor,
    {
        let mut builder = AsyncSmtpTransport::<E>::relay(&self.server)?
            .credentials(smtp::authentication::Credentials::new(
                self.username.to_owned(),
                self.password.to_owned(),
            ))
            .authentication(self.auth.clone());
        if let Some(port) = self.port {
            builder = builder.port(port);
        }
        Ok(builder.build())
    }
}
