mod behavior;
pub mod peer_to_peer_service;

extern crate core;

use anyhow::Result;
use did_key::{DIDKey, Ed25519KeyPair, KeyMaterial};
use libp2p::identity::Keypair::Ed25519;
use std::{sync::atomic::AtomicBool, sync::Arc};
use warp::{crypto::DID, error::Error};

pub type CancellationToken = Arc<AtomicBool>;

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
