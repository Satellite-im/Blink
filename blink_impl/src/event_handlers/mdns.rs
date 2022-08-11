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
use libp2p::mdns::MdnsEvent;

#[derive(Default)]
struct MdnsHandler {}

#[async_trait]
impl EventHandler for MdnsHandler {
    fn can_handle(event: &SwarmEvent<BehaviourEvent, EventErrorType>) -> bool {
        if let SwarmEvent::Behaviour(BehaviourEvent::MdnsEvent(_)) = event {
            return true;
        }

        false
    }

    async fn handle(swarm: &mut Swarm<BlinkBehavior>, event: SwarmEvent<BehaviourEvent, EventErrorType>, cache: Arc<RwLock<impl PocketDimension>>, logger: Arc<RwLock<impl EventBus>>, multi_pass: Arc<RwLock<impl MultiPass>>, message_sender: &Sender<MessageContent>, did: Arc<DID>, map: Arc<RwLock<HashMap<String, String>>>) {
        if let SwarmEvent::Behaviour(BehaviourEvent::MdnsEvent(e)) = event {
            match e {
                MdnsEvent::Discovered(list) => {
                    for (peer, _) in list {
                        swarm.behaviour_mut().gossip_sub.add_explicit_peer(&peer);
                    }
                }
                MdnsEvent::Expired(list) => {
                    for (peer, _) in list {
                        if !swarm.behaviour().mdns.has_node(&peer) {
                            swarm.behaviour_mut().gossip_sub.remove_explicit_peer(&peer);
                        }
                    }
                }
            }
        }
    }
}