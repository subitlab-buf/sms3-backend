use std::sync::Arc;

use lettre::AsyncSmtpTransport;

fn main() {}

#[derive(Debug, Clone)]
pub struct State {
    smtp_transport: Arc<AsyncSmtpTransport<lettre::Tokio1Executor>>,
}
