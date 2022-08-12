use std::collections::HashMap;
use std::sync::Arc;
use libp2p::Swarm;
use libp2p::swarm::SwarmEvent;
use tokio::sync::mpsc::Sender;
use warp::crypto::DID;
use warp::multipass::MultiPass;
use warp::pocket_dimension::PocketDimension;
use warp::sync::RwLock;
use blink_contract::{Event, EventBus};
use crate::behavior::{BehaviourEvent, BlinkBehavior};
use crate::event_handlers::{EventErrorType, EventHandler};
use crate::peer_to_peer_service::MessageContent;
use async_trait::async_trait;

#[derive(Default)]
pub(crate) struct NewListenerHandler {}

#[derive(Default)]
pub(crate) struct ConnectionEstablishedHandler {}

#[derive(Default)]
pub(crate) struct ConnectionClosedHandler {}

#[async_trait]
impl EventHandler for ConnectionClosedHandler {
    fn can_handle(&mut self, event: &SwarmEvent<BehaviourEvent, EventErrorType>) -> bool {
        if let SwarmEvent::ConnectionClosed { peer_id, .. } = event {
            return true;
        }

        false
    }

    async fn handle(&mut self, swarm: &mut Swarm<BlinkBehavior>, event: SwarmEvent<BehaviourEvent, EventErrorType>, cache: Arc<RwLock<impl PocketDimension>>, logger: Arc<RwLock<impl EventBus>>, multi_pass: Arc<RwLock<impl MultiPass>>, message_sender: &Sender<MessageContent>, did: Arc<DID>, map: Arc<RwLock<HashMap<String, String>>>) {
        if let SwarmEvent::ConnectionClosed { peer_id, .. } = event {
            logger.write().event_occurred(Event::PeerConnectionClosed(peer_id.to_string()));
        }
    }
}

#[async_trait]
impl EventHandler for ConnectionEstablishedHandler {
    fn can_handle(&mut self, event: &SwarmEvent<BehaviourEvent, EventErrorType>) -> bool {
        if let SwarmEvent::ConnectionEstablished { .. } = event {
            return true;
        }

        false
    }

    async fn handle(&mut self, swarm: &mut Swarm<BlinkBehavior>, event: SwarmEvent<BehaviourEvent, EventErrorType>, cache: Arc<RwLock<impl PocketDimension>>, logger: Arc<RwLock<impl EventBus>>, multi_pass: Arc<RwLock<impl MultiPass>>, message_sender: &Sender<MessageContent>, did: Arc<DID>, map: Arc<RwLock<HashMap<String, String>>>) {
        if let SwarmEvent::ConnectionEstablished { peer_id, .. } = event {
            logger.write().event_occurred(Event::ConnectionEstablished(peer_id.to_string()));
        }
    }
}


#[async_trait]
impl EventHandler for NewListenerHandler {
    fn can_handle(&mut self, event: &SwarmEvent<BehaviourEvent, EventErrorType>) -> bool {
        if let SwarmEvent::NewListenAddr { .. } = event {
            return true;
        }

        false
    }

    async fn handle(&mut self, swarm: &mut Swarm<BlinkBehavior>, event: SwarmEvent<BehaviourEvent, EventErrorType>, cache: Arc<RwLock<impl PocketDimension>>, logger: Arc<RwLock<impl EventBus>>, multi_pass: Arc<RwLock<impl MultiPass>>, message_sender: &Sender<MessageContent>, did: Arc<DID>, map: Arc<RwLock<HashMap<String, String>>>) {
        if let SwarmEvent::NewListenAddr { address, .. } = event {
            logger.write().event_occurred(Event::NewListenAddr(address));
        }
    }
}