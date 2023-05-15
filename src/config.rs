use once_cell::sync::Lazy;
use serde::Deserialize;
use std::{fs::File, io::Read};

/// The static config instance.
pub static INSTANCE: Lazy<Config> = Lazy::new(|| {
    toml::from_str(&{
        let mut string = String::new();
        File::open("./data/config.toml")
            .unwrap()
            .read_to_string(&mut string)
            .unwrap();
        string
    })
    .unwrap()
});

/// Describing the server configuration.
#[derive(Deserialize)]
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
