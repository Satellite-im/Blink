use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use tokio::sync::RwLock;

mod behavior;
mod peer_to_peer_service;

pub type CancellationToken = Arc<AtomicBool>;

struct BlinkImpl {}
