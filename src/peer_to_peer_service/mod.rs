use anyhow::Result;
use async_trait::async_trait;
use did_key::{DIDKey, Ed25519KeyPair, KeyMaterial};
use libp2p::Multiaddr;
use sata::Sata;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use warp::crypto::DID;
use warp::error::Error;

mod behavior;
mod peer_to_peer_service;

pub type CancellationToken = Arc<AtomicBool>;

fn did_to_libp2p_pub(public_key: &DID) -> Result<libp2p::identity::PublicKey> {
    let did = public_key.clone();
    let did: DIDKey = did.try_into()?;
    let pk = libp2p::identity::PublicKey::Ed25519(libp2p::identity::ed25519::PublicKey::decode(
        &did.public_key_bytes(),
    )?);
    Ok(pk)
}

fn libp2p_pub_to_did(public_key: &libp2p::identity::PublicKey) -> Result<DID> {
    let pk = match public_key {
        libp2p::identity::PublicKey::Ed25519(pk) => {
            let did: DIDKey = Ed25519KeyPair::from_public_key(&pk.encode()).into();
            did.try_into()?
        }
        _ => anyhow::bail!(Error::PublicKeyInvalid),
    };
    Ok(pk)
}

pub enum LogEvent {
    DialError(String),
    ConvertKeyError,
    SubscriptionError(String),
    NewListenAddr(Multiaddr),
    ErrorAddingToCache(Error),
    ErrorDeserializingData,
    ErrorSerializingData,
    ErrorPublishingData,
}

pub trait Logger: Send + Sync {
    fn event_occurred(&mut self, event: LogEvent);
}

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
    fn send(peers: Vec<DID>, data: Sata) -> Result<()>;
    // Stream data to another peer(s)
    // fn stream(peers: Vec<DIDKey>, kind: StreamKind, stream: Box<dyn Stream>) -> Result<()>;
    // // aliases
    // fn call(peers: Vec<DIDKey>, stream: Stream) -> Result<()>;
    // fn video(peers: Vec<DIDKey>, stream: Stream) -> Result<()>; // calls stream()
    // fn screen_share(peers: Vec<DIDKey>, stream: Stream) -> Result<()>; // calls stream()
}
