use once_cell::sync::Lazy;
use serde::Deserialize;

/// The static config instance.
#[allow(dead_code)]
pub static INSTANCE: Lazy<Config> = Lazy::new(|| {
    #[cfg(not(test))]
    {
        use std::{fs::File, io::Read};

        return toml::from_str(&{
            let mut string = String::new();
            File::open("./data/config.toml")
                .unwrap()
                .read_to_string(&mut string)
                .unwrap();
            string
        })
        .unwrap();
    }

    #[cfg(test)]
    Config::default()
});

/// Describing the server configuration.
#[derive(Deserialize, Default)]
pub struct Config {
    pub mail_smtp: MailSmtp,
}

/// Describing mailing configuration.
#[derive(Deserialize, Clone)]
pub struct MailSmtp {
    pub server: String,
    pub port: u16,
    pub username: String,
    pub password: String,
    pub address: lettre::Address,
}

impl Default for MailSmtp {
    fn default() -> Self {
        Self {
            server: String::default(),
            port: 0,
            username: String::default(),
            password: String::default(),
            address: lettre::Address::new("user", "email.com").unwrap(),
        }
    }
}
