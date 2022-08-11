use std::collections::HashMap;
use crate::{behavior::{BehaviourEvent, BlinkBehavior}, {libp2p_pub_to_did, CancellationToken}, did_keypair_to_libp2p_keypair};
use anyhow::Result;
use blink_contract::{Event, EventBus};
use did_key::{Ed25519KeyPair, Generate, KeyMaterial, ECDH};
use hmac_sha512::Hash;
use serde::{Serialize, Deserialize};
use libp2p::{
    gossipsub::TopicHash,
    mdns::MdnsEvent,
    core::transport::upgrade,
    futures::StreamExt,
    gossipsub::GossipsubEvent,
    identify::IdentifyEvent,
    identity::Keypair,
    kad::{KademliaEvent, QueryResult},
    mplex, noise,
    swarm::dial_opts::DialOpts,
    swarm::{NetworkBehaviour, SwarmBuilder, SwarmEvent},
    tcp::{GenTcpConfig, TokioTcpTransport},
    Multiaddr,
    PeerId,
    Swarm,
    Transport,
    gossipsub::IdentTopic
};
use sata::Sata;
use std::sync::{atomic::Ordering, Arc};
use std::time::UNIX_EPOCH;
use tokio::{
    sync::{
        mpsc::{
            Receiver,
            Sender
        }
    },
    task::JoinHandle,
};
use warp::{
    crypto::DID,
    data::DataType,
    multipass::{identity::Identifier, MultiPass},
    pocket_dimension::PocketDimension,
};
use warp::sync::RwLock;

pub type TopicName = String;

pub type MessageContent = (TopicHash, Sata);

const CHANNEL_SIZE: usize = 64;

#[derive(Debug)]
pub(crate) enum BlinkCommand {
    FindNearest(PeerId),
    Dial(DialOpts),
    Subscribe(String),
    PublishToTopic(TopicName, SataWrapper),
}

#[derive(Serialize, Deserialize, Debug, Default)]
pub(crate) struct SataWrapper {
    sata: Vec<u8>,
    time_sent: u128,
}

impl SataWrapper {
    pub(crate) fn new(sata: Sata) -> Result<HashMap<String, Self>> {
        let mut result = HashMap::new();
        let serialized = bincode::serialize(&sata)?;
        if let Some(recipients) = sata.recipients() {
            for recipient in recipients {
                let did = DID::from(recipient);
                let key = did.to_string();
                let time = std::time::SystemTime::now().duration_since(UNIX_EPOCH)?.as_micros();
                result.insert(key, Self {
                    sata: serialized.clone(),
                    time_sent: time,
                });
            }
        }

        Ok(result)
    }
}

pub struct PeerToPeerService {
    command_channel: Sender<BlinkCommand>,
    task_handle: JoinHandle<()>,
    map_peer_topic: Arc<RwLock<HashMap<String, String>>>,
    event_bus: Arc<RwLock<dyn EventBus>>,
}

impl Drop for PeerToPeerService {
    fn drop(&mut self) {
        self.task_handle.abort();
    }
}

impl PeerToPeerService {
    pub async fn new(
        did_key: Arc<DID>,
        address_to_listen: &str,
        initial_known_address: Option<Vec<Multiaddr>>,
        cache: Arc<RwLock<impl PocketDimension + 'static>>,
        multi_pass: Arc<RwLock<impl MultiPass + 'static>>,
        logger: Arc<RwLock<impl EventBus + 'static>>,
        cancellation_token: CancellationToken,
    ) -> Result<(Self, Receiver<MessageContent>)> {
        let key_pair = did_keypair_to_libp2p_keypair((*did_key).as_ref())?;
        let pub_key = key_pair.public();
        let peer_id = PeerId::from(&pub_key);
        let mut swarm = Self::create_swarm(&key_pair, &peer_id).await?;
        if let Some(initial_address) = initial_known_address {
            for addr in &initial_address {
                if let Some(peer_addr) = PeerId::try_from_multiaddr(addr) {
                    let behaviour = swarm.behaviour_mut();
                    behaviour.kademlia.add_address(&peer_addr, addr.clone());
                    behaviour.gossip_sub.add_explicit_peer(&peer_addr);
                }
            }
        }

        swarm.listen_on(address_to_listen.parse()?)?;

        let map = Arc::new(RwLock::new(HashMap::new()));
        let map_clone = map.clone();
        let logger_thread = logger.clone();
        let (command_tx, mut command_rx) = tokio::sync::mpsc::channel(CHANNEL_SIZE);
        let (message_tx, message_rx) = tokio::sync::mpsc::channel(CHANNEL_SIZE);

        let handler = tokio::spawn(async move {
            loop {
                if cancellation_token.load(Ordering::Acquire) {
                    logger_thread.write().event_occurred(Event::TaskCancelled);
                }

                tokio::select! {
                     cmd = command_rx.recv() => {
                         if let Some(command) = cmd {
                             Self::handle_command(&mut swarm, command, logger_thread.clone()).await;
                         }
                     },
                    event = swarm.select_next_some() => {
                         Self::handle_event(&mut swarm, event, cache.clone(),
                            logger_thread.clone(), multi_pass.clone(), &message_tx, did_key.clone(), map_clone.clone()).await;
                    }
                }
            }
        });

        Ok((
            Self {
                command_channel: command_tx,
                task_handle: handler,
                map_peer_topic: map,
                event_bus: logger.clone()
            },
            message_rx,
        ))
    }

    async fn handle_command(
        swarm: &mut Swarm<BlinkBehavior>,
        command: BlinkCommand,
        logger: Arc<RwLock<impl EventBus>>,
    ) {
        match command {
            BlinkCommand::FindNearest(peer_id) => {
                swarm.behaviour_mut().kademlia.get_closest_peers(peer_id);
            }
            BlinkCommand::Dial(dial_opts) => {
                let peer_id = (&dial_opts)
                    .get_peer_id()
                    .map_or(String::new(), |x| x.to_string());
                match swarm.dial(dial_opts) {
                    Ok(_) => {
                        logger
                            .write()
                            .event_occurred(Event::DialSuccessful(peer_id));
                    }
                    Err(err) => {
                        logger
                            .write()
                            .event_occurred(Event::DialError(err.to_string()));
                    }
                }
            }
            BlinkCommand::Subscribe(address) => {
                let topic = IdentTopic::new(address.clone());
                match swarm.behaviour_mut().gossip_sub.subscribe(&topic) {
                    Ok(_) => {
                        logger.write().event_occurred(Event::SubscribedToTopic(address));
                    }
                    Err(e) => {
                        logger.write().event_occurred(Event::SubscriptionError(e.to_string()))
                    }
                }
            }
            BlinkCommand::PublishToTopic(name, sata) => {
                let serialized_result = bincode::serialize(&sata);
                match serialized_result {
                    Ok(serialized) => {
                        let topic = IdentTopic::new(name);
                        if let Err(err) =
                            swarm.behaviour_mut().gossip_sub.publish(topic, serialized)
                        {
                            logger.write().event_occurred(Event::ErrorPublishingData(err.to_string()));
                        }
                    }
                    Err(_) => {
                        logger.write().event_occurred(Event::ErrorSerializingData);
                    }
                }
            }
        }
    }

    async fn handle_event<TErr>(
        swarm: &mut Swarm<BlinkBehavior>,
        event: SwarmEvent<BehaviourEvent, TErr>,
        cache: Arc<RwLock<impl PocketDimension>>,
        logger: Arc<RwLock<impl EventBus>>,
        multi_pass: Arc<RwLock<impl MultiPass>>,
        message_sender: &Sender<MessageContent>,
        did: Arc<DID>,
        map: Arc<RwLock<HashMap<String, String>>>
    ) {
        match event {
            SwarmEvent::Behaviour(BehaviourEvent::MdnsEvent(event)) => match event {
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
            },
            SwarmEvent::Behaviour(BehaviourEvent::IdentifyEvent(identify)) => match identify {
                IdentifyEvent::Received { peer_id, info } => {
                    let did_result = libp2p_pub_to_did(&info.public_key);

                    match did_result {
                        Ok(their_public) => {
                            match multi_pass.read()
                                .get_identity(Identifier::from(their_public.clone()))
                            {
                                Ok(_) => {
                                    let topic = Self::generate_topic_from_key_exchange(&*did, &their_public);
                                    {
                                        let pb = their_public.to_string();
                                        map.write().insert(pb, topic.clone());
                                    }
                                    let topic_subs = IdentTopic::new(&topic);
                                    match swarm.behaviour_mut().gossip_sub.subscribe(&topic_subs) {
                                        Ok(_) => {
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
                IdentifyEvent::Sent { .. } => {}
                IdentifyEvent::Pushed { .. } => {}
                IdentifyEvent::Error { .. } => {}
            },
            SwarmEvent::Behaviour(BehaviourEvent::Gossipsub(gsp)) => match gsp {
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
            },
            SwarmEvent::Behaviour(BehaviourEvent::KademliaEvent(kad)) => match kad {
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
            },
            SwarmEvent::ConnectionEstablished { peer_id, .. } => {
                logger.write().event_occurred(Event::ConnectionEstablished(peer_id.to_string()));
            }
            SwarmEvent::ConnectionClosed { peer_id, .. } => {
                logger.write().event_occurred(Event::PeerConnectionClosed(peer_id.to_string()));
            }
            SwarmEvent::IncomingConnection { .. } => {}
            SwarmEvent::IncomingConnectionError { .. } => {}
            SwarmEvent::OutgoingConnectionError { .. } => {}
            SwarmEvent::BannedPeer { .. } => {}
            SwarmEvent::NewListenAddr { address, .. } => {
                logger.write().event_occurred(Event::NewListenAddr(address));
            }
            SwarmEvent::ExpiredListenAddr { .. } => {}
            SwarmEvent::ListenerClosed { .. } => {}
            SwarmEvent::ListenerError { .. } => {}
            SwarmEvent::Dialing(_) => {}
            _ => {}
        }
    }

    fn generate_topic_from_key_exchange(private_key: &DID, public_key: &DID) -> String {
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

    async fn create_swarm(key_pair: &Keypair, peer_id: &PeerId) -> Result<Swarm<BlinkBehavior>> {
        let blink_behaviour = BlinkBehavior::new(&key_pair).await?;
        // Create a keypair for authenticated encryption of the transport.
        let noise_keys = noise::Keypair::<noise::X25519Spec>::new().into_authentic(&key_pair)?;

        // Create a tokio-based TCP transport use noise for authenticated
        // encryption and Mplex for multiplexing of substreams on a TCP stream.
        let transport = TokioTcpTransport::new(GenTcpConfig::default().nodelay(true))
            .upgrade(upgrade::Version::V1)
            .authenticate(noise::NoiseConfig::xx(noise_keys).into_authenticated())
            .multiplex(mplex::MplexConfig::new())
            .boxed();

        let swarm = SwarmBuilder::new(transport, blink_behaviour, peer_id.clone())
            .executor(Box::new(|fut| {
                tokio::spawn(fut);
            }))
            .build();

        Ok(swarm)
    }

    async fn subscribe_to_topic(&self, topic_name: String) -> Result<()> {
        self.command_channel
            .send(BlinkCommand::Subscribe(topic_name))
            .await?;
        Ok(())
    }

    pub async fn pair_to_another_peer(&mut self, dial_opts: DialOpts) -> Result<()> {
        self.command_channel
            .send(BlinkCommand::Dial(dial_opts))
            .await?;
        Ok(())
    }

    pub async fn send(&mut self, sata: Sata) -> Result<()> {
        let sata_sent = SataWrapper::new(sata)?;
        for (key, val) in sata_sent {
            if let Some(topic) = self.map_peer_topic.read().get(&key) {
                self.command_channel
                    .send(BlinkCommand::PublishToTopic(topic.clone(), val))
                    .await?;
            } else {
                self.event_bus.write().event_occurred(Event::CouldntFindTopicForDid);
            }
        }

        Ok(())
    }
}
