use std::sync::Arc;
use tokio::sync::RwLock;

mod behavior;
mod peer_to_peer_service;

pub type CancellationToken = Arc<RwLock<bool>>;