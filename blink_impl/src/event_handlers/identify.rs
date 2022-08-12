use std::collections::HashMap;
use std::sync::Arc;
use libp2p::swarm::SwarmEvent;
use crate::behavior::{BehaviourEvent, BlinkBehavior};
use crate::event_handlers::{EventErrorType, EventHandler, generate_topic_from_key_exchange};
use async_trait::async_trait;
use libp2p::gossipsub::IdentTopic;
use libp2p::identify::IdentifyEvent;
use libp2p::Swarm;
use tokio::sync::mpsc::Sender;
use warp::crypto::DID;
use warp::multipass::identity::Identifier;
use warp::multipass::MultiPass;
use warp::pocket_dimension::PocketDimension;
use warp::sync::RwLock;
use blink_contract::{Event, EventBus};
use crate::libp2p_pub_to_did;
use crate::peer_to_peer_service::MessageContent;

#[derive(Default)]
pub(crate) struct IdentifyEventHandler {}

#[async_trait]
impl EventHandler for IdentifyEventHandler {
    fn can_handle(&mut self, event: &SwarmEvent<BehaviourEvent, EventErrorType>) -> bool {
        if let SwarmEvent::Behaviour(BehaviourEvent::IdentifyEvent(identify)) = event {
            return true;
        }

        false
    }

    async fn handle(&mut self, swarm: &mut Swarm<BlinkBehavior>, event: SwarmEvent<BehaviourEvent, EventErrorType>, cache: Arc<RwLock<impl PocketDimension>>, logger: Arc<RwLock<impl EventBus>>, multi_pass: Arc<RwLock<impl MultiPass>>, _: &Sender<MessageContent>, did: Arc<DID>, map: Arc<RwLock<HashMap<String, String>>>) {
        if let SwarmEvent::Behaviour(BehaviourEvent::IdentifyEvent(identify)) = event {
            match identify {
                IdentifyEvent::Received { peer_id, info } => {
                    let did_result = libp2p_pub_to_did(&info.public_key);

                    match did_result {
                        Ok(their_public) => {
                            match multi_pass.read()
                                .get_identity(Identifier::from(their_public.clone()))
                            {
                                Ok(_) => {
                                    let topic = generate_topic_from_key_exchange(&*did, &their_public);
                                    let pb = their_public.clone().to_string();
                                    map.write().insert(pb, topic.clone());

                                    let topic_subs = IdentTopic::new(&topic);
                                    match swarm.behaviour_mut().gossip_sub.subscribe(&topic_subs) {
                                        Ok(_) => {
                                            logger.write().event_occurred(Event::GeneratedTopic(their_public, topic.clone()));
                                            logger.write().event_occurred(Event::SubscribedToTopic(topic));
                                            logger.write().event_occurred(Event::PeerIdentified);
                                        }
                                        Err(er) => {
                                            logger.write().event_occurred(Event::SubscriptionError(
                                                er.to_string(),
                                            ));
                                        }
                                    }
                                }
                                Err(_) => {
                                    logger.write().event_occurred(Event::FailureToIdentifyPeer);
                                    if swarm.disconnect_peer_id(peer_id).is_err() {
                                        logger.write().event_occurred(Event::FailureToDisconnectPeer);
                                    }
                                }
                            }
                        }
                        Err(_) => {
                            logger.write().event_occurred(Event::ConvertKeyError);
                        }
                    }
                }
                _ => {}
            }
        }
    }
}