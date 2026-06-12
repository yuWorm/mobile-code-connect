#[derive(Debug, Clone)]
pub struct RelayConfig {
    pub token_secret: String,
    pub now_epoch_sec: u64,
}
