use anyhow::Result;
use did_key::{DIDKey, Ed25519KeyPair, KeyMaterial};
use libp2p::identity::Keypair::Ed25519;
use libp2p::{Multiaddr, PeerId};
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

fn did_keypair_to_libp2p_keypair(key_pair: &DIDKey) -> Result<libp2p::identity::Keypair> {
    let private = key_pair.private_key_bytes();
    let secret_key = libp2p::identity::ed25519::SecretKey::from_bytes(private)?;
    Ok(Ed25519(secret_key.into()))
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

#[derive(Debug)]
pub enum LogEvent {
    DialError(String),
    ConvertKeyError,
    SubscriptionError(String),
    NewListenAddr(Multiaddr),
    ErrorAddingToCache(Error),
    ErrorDeserializingData,
    ErrorSerializingData,
    ErrorPublishingData(String),
    SubscribedToTopic(String),
    FailureToIdentifyPeer,
    FailureToDisconnectPeer,
    PeerConnectionClosed(PeerId),
    ConnectionEstablished(PeerId),
    TaskCancelled,
}

pub trait Logger: Send + Sync {
    fn event_occurred(&mut self, event: LogEvent);
}
