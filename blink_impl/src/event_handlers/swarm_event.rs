use std::collections::HashMap;
use std::sync::Arc;
use libp2p::Swarm;
use libp2p::swarm::SwarmEvent;
use tokio::sync::mpsc::Sender;
use warp::crypto::DID;
use warp::multipass::MultiPass;
use warp::pocket_dimension::PocketDimension;
use warp::sync::RwLock;
use blink_contract::EventBus;
use crate::behavior::{BehaviourEvent, BlinkBehavior};
use crate::event_handlers::{EventErrorType, EventHandler};
use crate::peer_to_peer_service::MessageContent;
use async_trait::async_trait;

#[derive(Default)]
pub(crate) struct NewListenerHandler {}

#[async_trait]
impl EventHandler for NewListenerHandler {
    fn can_handle(event: &SwarmEvent<BehaviourEvent, EventErrorType>) -> bool {
        todo!()
    }

    async fn handle(swarm: &mut Swarm<BlinkBehavior>, event: SwarmEvent<BehaviourEvent, EventErrorType>, cache: Arc<RwLock<impl PocketDimension>>, logger: Arc<RwLock<impl EventBus>>, multi_pass: Arc<RwLock<impl MultiPass>>, message_sender: &Sender<MessageContent>, did: Arc<DID>, map: Arc<RwLock<HashMap<String, String>>>) {
        todo!()
    }
}