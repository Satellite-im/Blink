extern crate core;

use anyhow::Result;
use async_trait::async_trait;
use sata::Sata;
use warp::crypto::DID;

pub mod peer_to_peer_service;

pub enum Event {}

enum StreamKind {}

#[async_trait]
pub trait Blink {
    // Handshakes to another peer and verifies identity
    async fn pair(peers: Vec<DID>) -> Result<()>;
    // Starts listening for data from the remote peer(s)
    async fn open(peers: Vec<DID>) -> Result<()>;
    // Caches data to pocket dimension
    fn cache(data: Sata) -> Result<()>;
    // Allows developers to listen in on communications and hook data they care about
    fn hook(event: Event);
    // Send data directly to another peer(s)
    fn send(data: Sata) -> Result<()>;
    // Stream data to another peer(s)
    // fn stream(peers: Vec<DIDKey>, kind: StreamKind, stream: Box<dyn Stream>) -> Result<()>;
    // // aliases
    // fn call(peers: Vec<DIDKey>, stream: Stream) -> Result<()>;
    // fn video(peers: Vec<DIDKey>, stream: Stream) -> Result<()>; // calls stream()
    // fn screen_share(peers: Vec<DIDKey>, stream: Stream) -> Result<()>; // calls stream()
}
