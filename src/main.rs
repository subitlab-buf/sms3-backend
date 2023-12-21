mod account;

fn main() {
    println!("Good bye, world!");
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("account error: {0}")]
    LibAccount(libaccount::Error),
    #[error("captcha incorrect")]
    CaptchaIncorrect,
}

macro_rules! impl_from {
    ($($t:ty => $v:ident),* $(,)?) => {
        $(
            impl From<$t> for Error {
                #[inline]
                fn from(err: $t) -> Self {
                    Self::$v(err)
                }
            }
        )*
    };
}

impl_from! {
    libaccount::Error => LibAccount,
}
