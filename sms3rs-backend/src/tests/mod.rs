mod account;

/// Reset all static instances.
async fn reset_all() {
    crate::account::INSTANCE.reset().await;
    crate::post::INSTANCE.reset().await;
    crate::post::cache::INSTANCE.reset().await;
}
