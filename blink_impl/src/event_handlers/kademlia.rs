use std::collections::HashMap;
use std::sync::Arc;
use libp2p::Swarm;
use libp2p::swarm::{NetworkBehaviour, SwarmEvent};
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
use libp2p::kad::{KademliaEvent, QueryResult};

#[derive(Default)]
pub(crate) struct KademliaEventHandler {
}

#[async_trait]
impl EventHandler for KademliaEventHandler {
    fn can_handle(&mut self, event: &SwarmEvent<BehaviourEvent, EventErrorType>) -> bool {
        if let SwarmEvent::Behaviour(BehaviourEvent::KademliaEvent(_)) = event {
            return true;
        }

        false
    }

    async fn handle(&mut self, swarm: &mut Swarm<BlinkBehavior>, event: SwarmEvent<BehaviourEvent, EventErrorType>, cache: Arc<RwLock<impl PocketDimension>>, logger: Arc<RwLock<impl EventBus>>, multi_pass: Arc<RwLock<impl MultiPass>>, message_sender: &Sender<MessageContent>, did: Arc<DID>, map: Arc<RwLock<HashMap<String, String>>>) {
        if let SwarmEvent::Behaviour(BehaviourEvent::KademliaEvent(kad)) = event {
            match kad {
                KademliaEvent::InboundRequest { .. } => {}
                KademliaEvent::OutboundQueryCompleted { result, .. } => match result {
                    QueryResult::Bootstrap(_) => {}
                    QueryResult::GetClosestPeers(Ok(ok)) => {
                        let kademlia = &mut swarm.behaviour_mut().kademlia;
                        for peer in ok.peers {
                            let addrs = kademlia.addresses_of_peer(&peer);
                            for addr in addrs {
                                kademlia.add_address(&peer, addr);
                            }
                        }
                    }
                    QueryResult::GetProviders(_) => {}
                    QueryResult::StartProviding(_) => {}
                    QueryResult::RepublishProvider(_) => {}
                    QueryResult::GetRecord(_) => {}
                    QueryResult::PutRecord(_) => {}
                    QueryResult::RepublishRecord(_) => {}
                    _ => {}
                },
                KademliaEvent::RoutingUpdated { .. } => {}
                KademliaEvent::UnroutablePeer { .. } => {}
                KademliaEvent::RoutablePeer { .. } => {}
                KademliaEvent::PendingRoutablePeer { .. } => {}
            }
        }
    }
}