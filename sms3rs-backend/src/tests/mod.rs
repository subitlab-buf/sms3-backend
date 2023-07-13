mod account;
mod account_manage;

mod post;

/// Reset all static instances.
fn reset_all() {
    crate::account::INSTANCE.reset();
    crate::post::INSTANCE.reset();
    crate::post::cache::INSTANCE.reset();
}
