extern crate core;

use anyhow::Result;
use async_trait::async_trait;
use libp2p::{Multiaddr, PeerId};
use sata::Sata;
use warp::crypto::DID;

pub mod peer_to_peer_service;
