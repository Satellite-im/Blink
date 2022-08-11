mod identify;
mod mdns;
mod gossipsub;
mod kademlia;
mod swarm_event;

use std::collections::HashMap;
use std::sync::Arc;
use libp2p::core::either::EitherError;
use libp2p::gossipsub::error::GossipsubHandlerError;
use libp2p::relay::v2::{InboundHopFatalUpgradeError, OutboundStopFatalUpgradeError};
use libp2p::swarm::{ConnectionHandlerUpgrErr, SwarmEvent};
use crate::behavior::{BehaviourEvent, BlinkBehavior};
use async_trait::async_trait;
use did_key::{ECDH, Ed25519KeyPair, Generate, KeyMaterial};
use hmac_sha512::Hash;
use libp2p::Swarm;
use tokio::sync::mpsc::Sender;
use warp::crypto::DID;
use warp::multipass::MultiPass;
use warp::pocket_dimension::PocketDimension;
use warp::sync::RwLock;
use blink_contract::EventBus;
use crate::peer_to_peer_service::MessageContent;

pub(crate) type EventErrorType = EitherError<EitherError<EitherError<EitherError<EitherError<GossipsubHandlerError, std::io::Error>, std::io::Error>,
    either::Either<ConnectionHandlerUpgrErr<EitherError<InboundHopFatalUpgradeError, OutboundStopFatalUpgradeError>>, void::Void>>, void::Void>, libp2p::ping::Failure>;

#[async_trait]
pub(crate) trait EventHandler {
    fn can_handle(event: &SwarmEvent<BehaviourEvent, EventErrorType>) -> bool;
    async fn handle(swarm: &mut Swarm<BlinkBehavior>,
                    event: SwarmEvent<BehaviourEvent, EventErrorType>,
                    cache: Arc<RwLock<impl PocketDimension>>,
                    logger: Arc<RwLock<impl EventBus>>,
                    multi_pass: Arc<RwLock<impl MultiPass>>,
                    message_sender: &Sender<MessageContent>,
                    did: Arc<DID>,
                    map: Arc<RwLock<HashMap<String, String>>>);
}

pub(crate) fn generate_topic_from_key_exchange(private_key: &DID, public_key: &DID) -> String {
    let private_key_pair = Ed25519KeyPair::from_secret_key(
        &private_key.as_ref().private_key_bytes()
    ).get_x25519();
    let public_key_pair = Ed25519KeyPair::from_public_key(
        &public_key.as_ref().public_key_bytes(),
    ).get_x25519();
    let exchange = private_key_pair.key_exchange(&public_key_pair);
    let hashed = Hash::hash(exchange);
    let topic = base64::encode(hashed);

    topic
}