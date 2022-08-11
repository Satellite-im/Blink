use std::collections::HashMap;
use std::sync::Arc;
use libp2p::gossipsub::GossipsubEvent;
use libp2p::Swarm;
use libp2p::swarm::SwarmEvent;
use sata::Sata;
use tokio::sync::mpsc::Sender;
use warp::crypto::DID;
use warp::data::DataType;
use warp::multipass::MultiPass;
use warp::pocket_dimension::PocketDimension;
use warp::sync::RwLock;
use blink_contract::{Event, EventBus};
use crate::behavior::{BehaviourEvent, BlinkBehavior};
use crate::event_handlers::{EventErrorType, EventHandler};
use crate::peer_to_peer_service::{MessageContent, SataWrapper};
use async_trait::async_trait;

#[derive(Default)]
pub(crate) struct GossipSubHandler {}

#[async_trait]
impl EventHandler for GossipSubHandler {
    fn can_handle(event: &SwarmEvent<BehaviourEvent, EventErrorType>) -> bool {
        if let SwarmEvent::Behaviour(BehaviourEvent::Gossipsub(_)) = event {
            return true;
        }

        false
    }

    async fn handle(swarm: &mut Swarm<BlinkBehavior>, event: SwarmEvent<BehaviourEvent, EventErrorType>, cache: Arc<RwLock<impl PocketDimension>>, logger: Arc<RwLock<impl EventBus>>, multi_pass: Arc<RwLock<impl MultiPass>>, message_sender: &Sender<MessageContent>, did: Arc<DID>, map: Arc<RwLock<HashMap<String, String>>>) {
        if let SwarmEvent::Behaviour(BehaviourEvent::Gossipsub(sub)) = event {
            match sub {
                GossipsubEvent::Message { message, .. } => {
                    let message_data = message.data;
                    let sata_sent = bincode::deserialize::<SataWrapper>(&message_data);
                    match sata_sent {
                        Ok(wrapped) => {
                            let data = bincode::deserialize::<Sata>(&wrapped.sata);
                            match data {
                                Ok(info) => {
                                    if let Err(e) = cache.write().add_data(DataType::Messaging, &info) {
                                        logger.write().event_occurred(Event::ErrorAddingToCache(e.enum_to_string()));
                                    }
                                    if let Err(_) = message_sender.send((message.topic, info.clone())).await {
                                        logger.write().event_occurred(Event::FailedToSendMessage);
                                    }
                                }
                                Err(_) => {
                                    logger.write().event_occurred(Event::ErrorDeserializingData);
                                }
                            }
                        }
                        Err(_) => {
                            logger.write().event_occurred(Event::ErrorDeserializingData);
                        }
                    }
                }
                GossipsubEvent::Subscribed { .. } => {}
                GossipsubEvent::Unsubscribed { .. } => {}
                GossipsubEvent::GossipsubNotSupported { .. } => {}
            }
        }
    }
}