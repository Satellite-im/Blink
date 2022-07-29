use did_key::DIDKey;
use libp2p::futures::Stream;
use sata::Sata;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use tokio::sync::RwLock;
use anyhow::Result;
use async_trait::async_trait;
use libp2p::Multiaddr;

mod behavior;
mod peer_to_peer_service;

pub type CancellationToken = Arc<AtomicBool>;

pub enum LogEvent {
    DialError(String),
    SubscriptionError(String),
    NewListenAddr(Multiaddr),
}

pub trait Logger : Send + Sync {
    fn event_occurred(&mut self, event: LogEvent);
}

pub enum Event {
}

enum StreamKind {

}

#[async_trait]
pub trait Blink {
    // Handshakes to another peer and verifies identity
    async fn pair(peers: Vec<DIDKey>) -> Result<()>;
    // Starts listening for data from the remote peer(s)
    async fn open(peers: Vec<DIDKey>) -> Result<()>;
    // Caches data to pocket dimension
    fn cache(data: Sata) -> Result<()>;
    // Allows developers to listen in on communications and hook data they care about
    fn hook(event: Event);
    // Send data directly to another peer(s)
    fn send(peers: Vec<DIDKey>, data: Sata) -> Result<()>;
    // Stream data to another peer(s)
    // fn stream(peers: Vec<DIDKey>, kind: StreamKind, stream: Box<dyn Stream>) -> Result<()>;
    // // aliases
    // fn call(peers: Vec<DIDKey>, stream: Stream) -> Result<()>;
    // fn video(peers: Vec<DIDKey>, stream: Stream) -> Result<()>; // calls stream()
    // fn screen_share(peers: Vec<DIDKey>, stream: Stream) -> Result<()>; // calls stream()
}
