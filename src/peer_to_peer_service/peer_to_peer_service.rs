use crate::peer_to_peer_service::did_keypair_to_libp2p_keypair;
use crate::{
    peer_to_peer_service::behavior::{BehaviourEvent, BlinkBehavior},
    peer_to_peer_service::{libp2p_pub_to_did, CancellationToken, LogEvent, Logger},
};
use anyhow::Result;
use bincode::serialize;
use did_key::{DIDKey, ECDH, Ed25519KeyPair, Generate, KeyMaterial};
use hmac_sha512::Hash;
use libp2p::gossipsub::{Hasher, Topic};
use libp2p::mdns::MdnsEvent;
use libp2p::{
    core::transport::upgrade,
    futures::StreamExt,
    gossipsub::{GossipsubEvent, Sha256Topic},
    identify::IdentifyEvent,
    identity::Keypair,
    kad::{KademliaEvent, QueryResult},
    mplex, noise,
    swarm::{NetworkBehaviour, SwarmBuilder, SwarmEvent},
    tcp::{GenTcpConfig, TokioTcpTransport},
    Multiaddr, PeerId, Swarm, Transport,
};
use sata::Sata;
use std::{
    collections::HashMap,
    sync::{atomic::Ordering, Arc},
};
use tokio::{
    sync::{mpsc::Sender, RwLock},
    task::JoinHandle,
};
use warp::crypto::cipher;
use warp::crypto::ed25519_dalek::{PublicKey, SignatureError};
use warp::crypto::DID;
use warp::error::Error;
use warp::multipass::identity::Identity;
use warp::{
    data::DataType,
    multipass::{identity::Identifier, MultiPass},
    pocket_dimension::{query::QueryBuilder, PocketDimension},
};

pub type TopicName = String;

#[derive(Debug)]
pub enum BlinkCommand {
    FindNearest(PeerId),
    Dial(PeerId),
    Subscribe(String),
    PublishToTopic(TopicName, Sata),
}

pub struct PeerToPeerService {
    command_channel: Sender<BlinkCommand>,
    task_handle: JoinHandle<()>
}

impl Drop for PeerToPeerService
{
    fn drop(&mut self) {
        self.task_handle.abort();
    }
}

impl PeerToPeerService
{
    pub async fn new(
        did_key: Arc<RwLock<DID>>,
        address_to_listen: &str,
        initial_known_address: Option<HashMap<PeerId, Multiaddr>>,
        cache: Arc<RwLock<impl PocketDimension + 'static>>,
        multi_pass: Arc<RwLock<impl MultiPass + 'static>>,
        logger: Arc<RwLock<impl Logger + 'static>>,
        cancellation_token: CancellationToken,
    ) -> Result<Self> {
        let key_pair = {
            let did_read = did_key.read().await;
            did_keypair_to_libp2p_keypair((*did_read).as_ref())?
        };
        let pub_key = key_pair.public();
        let peer_id = PeerId::from(&pub_key);
        let mut swarm = Self::create_swarm(&key_pair, &peer_id).await?;
        if let Some(initial_address) = initial_known_address {
            for (peer, addr) in initial_address {
                swarm.behaviour_mut().kademlia.add_address(&peer, addr);
            }
        }

        swarm.listen_on(address_to_listen.parse()?)?;

        let (command_tx, mut command_rx) = tokio::sync::mpsc::channel(32);
        let cache_to_thread = cache.clone();
        let thread_logger = logger.clone();
        let multi_pass_thread = multi_pass.clone();
        let did_thread = did_key.clone();

        let handler = tokio::spawn(async move {
            loop {
                if cancellation_token.load(Ordering::Acquire) {
                    let mut log_write = thread_logger.write().await;
                    (*log_write).event_occurred(LogEvent::TaskCancelled);
                }

                tokio::select! {
                     cmd = command_rx.recv() => {
                         if let Some(command) = cmd {
                             Self::handle_command(&mut swarm, command, thread_logger.clone()).await;
                         }
                     },
                    event = swarm.select_next_some() => {
                         Self::handle_event(&mut swarm, event, cache_to_thread.clone(),
                            thread_logger.clone(), multi_pass_thread.clone(), did_thread.clone()).await;
                    }
                }
            }
        });

        Ok(Self {
            command_channel: command_tx,
            task_handle: handler
        })
    }

    async fn handle_command(
        swarm: &mut Swarm<BlinkBehavior>,
        command: BlinkCommand,
        logger: Arc<RwLock<impl Logger>>,
    ) {
        match command {
            BlinkCommand::FindNearest(peer_id) => {
                swarm.behaviour_mut().kademlia.get_closest_peers(peer_id);
            }
            BlinkCommand::Dial(peer_id) => {
                if let Err(err) = swarm.dial(peer_id) {
                    logger
                        .write()
                        .await
                        .event_occurred(LogEvent::DialError(err.to_string()));
                }
            }
            BlinkCommand::Subscribe(address) => {
                if let Err(e) = swarm.behaviour_mut().gossip_sub.subscribe(address.clone()) {
                    let mut service = logger.write().await;
                    service.event_occurred(LogEvent::SubscriptionError(e.to_string()))
                } else {
                    let mut service = logger.write().await;
                    service.event_occurred(LogEvent::SubscribedToTopic(address));
                }
            }
            BlinkCommand::PublishToTopic(name, sata) => {
                let serialized_result = bincode::serialize(&sata);
                match serialized_result {
                    Ok(serialized) => {
                        if let Err(_) = swarm.behaviour_mut().gossip_sub.publish(name, serialized) {
                            let mut log_service = logger.write().await;
                            (*log_service).event_occurred(LogEvent::ErrorPublishingData);
                        }
                    }
                    Err(_) => {
                        let mut log_service = logger.write().await;
                        (*log_service).event_occurred(LogEvent::ErrorSerializingData);
                    }
                }
            }
        }
    }

    async fn handle_event<TErr>(
        swarm: &mut Swarm<BlinkBehavior>,
        event: SwarmEvent<BehaviourEvent, TErr>,
        cache: Arc<RwLock<impl PocketDimension>>,
        logger: Arc<RwLock<impl Logger>>,
        multi_pass: Arc<RwLock<impl MultiPass>>,
        did: Arc<RwLock<DID>>,
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
                            let multi_pass_read = multi_pass.read().await;

                            match (*multi_pass_read)
                                .get_identity(Identifier::from(their_public.clone()))
                            {
                                Ok(_) => {
                                    let private_read = did.read().await;
                                    let private_key_pair = Ed25519KeyPair::from_secret_key(&(*private_read).as_ref().private_key_bytes()).get_x25519();
                                    let public_key_pair = Ed25519KeyPair::from_public_key(&their_public.as_ref().public_key_bytes()).get_x25519();
                                    let exchange =
                                        private_key_pair.key_exchange(&public_key_pair);
                                    let hashed = Hash::hash(exchange);
                                    let topic = base64::encode(hashed);
                                    match swarm.behaviour_mut().gossip_sub.subscribe(topic.clone()) {
                                        Ok(_) => {
                                            let mut log = logger.write().await;
                                            (*log).event_occurred(LogEvent::SubscribedToTopic(topic));
                                        }
                                        Err(er) => {
                                            let mut log = logger.write().await;
                                            (*log).event_occurred(LogEvent::SubscriptionError(er.to_string()));
                                        }
                                    }
                                },
                                Err(_) => {
                                    let mut log = logger.write().await;
                                    (*log).event_occurred(LogEvent::FailureToIdentifyPeer);
                                    if swarm.disconnect_peer_id(peer_id).is_err() {
                                        (*log).event_occurred(LogEvent::FailureToDisconnectPeer);
                                    }
                                }
                            }
                        }
                        Err(_) => {
                            let mut log = logger.write().await;
                            (*log).event_occurred(LogEvent::ConvertKeyError);
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
                    let data = bincode::deserialize::<Sata>(&message_data);
                    match data {
                        Ok(info) => {
                            let mut write = cache.write().await;
                            if let Err(e) = write.add_data(DataType::Messaging, &info) {
                                let mut log_service = logger.write().await;
                                log_service.event_occurred(LogEvent::ErrorAddingToCache(e));
                            }
                        }
                        Err(_) => {
                            let mut log_service = logger.write().await;
                            log_service.event_occurred(LogEvent::ErrorDeserializingData);
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
                let mut log_service = logger.write().await;
                (*log_service).event_occurred(LogEvent::ConnectionEstablished(peer_id));
            }
            SwarmEvent::ConnectionClosed { peer_id, .. } => {
                let mut log_service = logger.write().await;
                (*log_service).event_occurred(LogEvent::PeerConnectionClosed(peer_id));
            }
            SwarmEvent::IncomingConnection { .. } => {}
            SwarmEvent::IncomingConnectionError { .. } => {}
            SwarmEvent::OutgoingConnectionError { .. } => {}
            SwarmEvent::BannedPeer { .. } => {}
            SwarmEvent::NewListenAddr { address, .. } => {
                let mut log_service = logger.write().await;
                (*log_service).event_occurred(LogEvent::NewListenAddr(address));
            }
            SwarmEvent::ExpiredListenAddr { .. } => {}
            SwarmEvent::ListenerClosed { .. } => {}
            SwarmEvent::ListenerError { .. } => {}
            SwarmEvent::Dialing(_) => {}
            _ => {}
        }
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

    pub async fn subscribe_to_topic(&self, topic_name: String) -> Result<()> {
        self.command_channel
            .send(BlinkCommand::Subscribe(topic_name))
            .await?;
        Ok(())
    }

    pub async fn pair_to_another_peer(&mut self, peer_id: PeerId) -> Result<()> {
        self.command_channel
            .send(BlinkCommand::Dial(peer_id))
            .await?;
        Ok(())
    }

    pub async fn publish_message_to_topic(&mut self, topic: String, sata: Sata) -> Result<()> {
        self.command_channel
            .send(BlinkCommand::PublishToTopic(topic, sata))
            .await?;
        Ok(())
    }
}

#[cfg(test)]
mod when_using_peer_to_peer_service {
    use crate::peer_to_peer_service::{did_keypair_to_libp2p_keypair, did_to_libp2p_pub};
    use crate::{
        peer_to_peer_service::peer_to_peer_service::PeerToPeerService,
        peer_to_peer_service::{libp2p_pub_to_did, LogEvent, Logger},
    };
    use did_key::Ed25519KeyPair;
    use libp2p::{futures::TryFutureExt, identity, Multiaddr, PeerId};
    use sata::Sata;
    use std::{
        collections::HashMap, hash::Hash, ops::Mul, sync::atomic::AtomicBool, sync::Arc,
        time::Duration,
    };
    use std::env::current_exe;
    use tokio::sync::RwLock;
    use warp::crypto::generate;
    use warp::{
        crypto::DID,
        data::DataType,
        error::Error,
        module::Module,
        multipass::identity::{Identifier, Identity, IdentityUpdate},
        multipass::{Friends, MultiPass},
        pocket_dimension::query::QueryBuilder,
        pocket_dimension::PocketDimension,
        Extension, SingleHandle,
    };
    use crate::peer_to_peer_service::behavior::BehaviourEvent;

    const TIMEOUT_SECS : u64 = 1;

    #[derive(Default)]
    struct TestCache {
        data_added: Vec<(DataType, Sata)>,
    }

    struct MultiPassImpl {
        pass_as_valid: bool,
    }

    impl MultiPassImpl {
        fn new(pass_as_valid: bool) -> Self {
            Self { pass_as_valid }
        }
    }

    impl Extension for MultiPassImpl {
        fn id(&self) -> String {
            todo!()
        }

        fn name(&self) -> String {
            todo!()
        }

        fn module(&self) -> Module {
            todo!()
        }
    }

    impl Friends for MultiPassImpl {}

    impl SingleHandle for MultiPassImpl {}

    impl MultiPass for MultiPassImpl {
        fn create_identity(
            &mut self,
            username: Option<&str>,
            passphrase: Option<&str>,
        ) -> Result<DID, Error> {
            todo!()
        }

        fn get_identity(&self, id: Identifier) -> Result<Identity, Error> {
            if self.pass_as_valid {
                return Ok(Identity::default());
            }

            Err(Error::IdentityDoesntExist)
        }

        fn update_identity(&mut self, option: IdentityUpdate) -> Result<(), Error> {
            todo!()
        }

        fn decrypt_private_key(&self, passphrase: Option<&str>) -> Result<Vec<u8>, Error> {
            todo!()
        }

        fn refresh_cache(&mut self) -> Result<(), Error> {
            todo!()
        }
    }

    impl Extension for TestCache {
        fn id(&self) -> String {
            todo!()
        }

        fn name(&self) -> String {
            todo!()
        }

        fn module(&self) -> Module {
            todo!()
        }
    }

    impl SingleHandle for TestCache {}

    impl PocketDimension for TestCache {
        fn add_data(&mut self, dimension: DataType, data: &Sata) -> Result<(), Error> {
            self.data_added.push((dimension, data.clone()));
            Ok(())
        }

        fn has_data(&mut self, dimension: DataType, query: &QueryBuilder) -> Result<(), Error> {
            todo!()
        }

        fn get_data(
            &self,
            dimension: DataType,
            query: Option<&QueryBuilder>,
        ) -> Result<Vec<Sata>, Error> {
            todo!()
        }

        fn size(&self, dimension: DataType, query: Option<&QueryBuilder>) -> Result<i64, Error> {
            todo!()
        }

        fn count(&self, dimension: DataType, query: Option<&QueryBuilder>) -> Result<i64, Error> {
            todo!()
        }

        fn empty(&mut self, dimension: DataType) -> Result<(), Error> {
            todo!()
        }
    }

    struct LogHandler {
        pub events: Vec<LogEvent>,
    }

    impl LogHandler {
        fn new() -> Self {
            Self { events: Vec::new() }
        }
    }

    impl Logger for LogHandler {
        fn event_occurred(&mut self, event: LogEvent) {
            self.events.push(event);
        }
    }

    async fn create_service(
        initial_address: HashMap<PeerId, Multiaddr>,
        pass_multi_pass_validation_requests: bool,
    ) -> (
        PeerToPeerService,
        Arc<RwLock<LogHandler>>,
        PeerId,
        Arc<RwLock<TestCache>>,
        Arc<RwLock<MultiPassImpl>>,
        Arc<RwLock<DID>>,
        HashMap<PeerId, Multiaddr>
    ) {
        let id_keys = Arc::new(RwLock::new(DID::from(did_key::generate::<Ed25519KeyPair>(
            None,
        ))));
        let key_pair = {
            let id_read = id_keys.read().await;
            did_keypair_to_libp2p_keypair((*id_read).as_ref()).unwrap()
        };
        let peer_id = PeerId::from(key_pair.public());
        let cancellation_token = Arc::new(AtomicBool::new(false));
        let cache = Arc::new(RwLock::new(TestCache::default()));
        let log_handler = Arc::new(RwLock::new(LogHandler::new()));
        let multi_pass = Arc::new(RwLock::new(MultiPassImpl::new(
            pass_multi_pass_validation_requests,
        )));
        let service = PeerToPeerService::new(
            id_keys.clone(),
            "/ip4/0.0.0.0/tcp/0",
            Some(initial_address),
            cache.clone(),
            multi_pass.clone(),
            log_handler.clone(),
            cancellation_token.clone(),
        )
        .await
        .unwrap();

        let mut addr_to_send = None;
        let mut break_loop = false;
        while !break_loop {
            let log_handler_read = log_handler.read().await;

            for event in &(*log_handler_read).events {
                if let LogEvent::NewListenAddr(addr) = event {
                    break_loop = true;
                    addr_to_send = Some(addr.clone());
                    break;
                }
            }
        }

        let mut map = HashMap::new();
        map.insert(peer_id.clone(), addr_to_send.unwrap());

        (service, log_handler, peer_id, cache, multi_pass, id_keys, map)
    }

    #[tokio::test]
    async fn open_does_not_throw() {
        tokio::time::timeout(Duration::from_secs(TIMEOUT_SECS), async {
            create_service(HashMap::new(), true).await;
        }).await.expect("timeout");
    }

    #[tokio::test]
    async fn connecting_to_peer_does_not_generate_errors() {
        tokio::time::timeout(Duration::from_secs(TIMEOUT_SECS), async {
            let (mut second_client, mut log_handler, peer_id, _, _, _, addr_map) =
                create_service(HashMap::new(), true).await;

            let (mut first_client, mut first_client_logger, _, _, _, _, _) =
                create_service(addr_map, true).await;

            pair_to_peer(&mut first_client, &peer_id, first_client_logger.clone());
        }).await.expect("Timeout");
    }

    #[tokio::test]
    async fn subscribe_to_topic_does_not_cause_errors() {
        tokio::time::timeout(Duration::from_secs(TIMEOUT_SECS), async {
            let (mut client, mut log_handler, _, _, _, _, _) = create_service(HashMap::new(), true).await;

            subscribe_to_topic(&mut client, "some topic".to_string(), log_handler.clone()).await;
        }).await.expect("Timeout");
    }

    #[tokio::test]
    async fn message_to_another_client_is_added_to_cache() {
        const TOPIC_NAME: &str = "SomeTopic";
        tokio::time::timeout(Duration::from_secs(TIMEOUT_SECS), async {
            let (mut second_client, log_handler, second_client_peer_id, second_client_cache, _, _, second_client_address) =
                create_service(HashMap::new(), true).await;

            subscribe_to_topic(&mut second_client, TOPIC_NAME.to_string(), log_handler.clone()).await;

            let (
                mut first_client,
                first_client_log_handler,
                first_client_peer_id,
                first_client_cache,
                _,
                _,
                _,
            ) = create_service(second_client_address, true).await;

            subscribe_to_topic(&mut first_client, TOPIC_NAME.to_string(), log_handler.clone()).await;

            pair_to_peer(&mut first_client, &second_client_peer_id, log_handler.clone()).await;

            first_client
                .publish_message_to_topic(TOPIC_NAME.to_string(), Sata::default())
                .await
                .unwrap();

            loop {
                let cache_read = second_client_cache.read().await;
                if (*cache_read).data_added.len() > 0 {
                    break;
                }
            }
        }).await.expect("Timeout");
    }

    #[tokio::test]
    async fn failure_to_identify_peer_causes_error() {
        tokio::time::timeout(Duration::from_secs(TIMEOUT_SECS), async {
            let (
                first_client,
                mut log_handler_first_client,
                first_client_peer_id,
                first_client_cache,
                first_client_multi_pass,
                first_client_did_key,
                first_client_address
            ) = create_service(HashMap::new(), false).await;

            let (
                mut second_client,
                mut log_handler_second_client,
                second_client_peer_id,
                second_client_cache,
                second_client_multi_pass,
                second_client_did_key,
                _
            ) = create_service(first_client_address, false).await;

            pair_to_peer(&mut second_client, &first_client_peer_id, log_handler_second_client.clone()).await;

            let mut found_error = false;
            while !found_error {
                let log_handler_read = log_handler_second_client.read().await;
                for event in &(*log_handler_read).events {
                    if let LogEvent::FailureToIdentifyPeer = event {
                        found_error = true;
                    }
                }
            }
        }).await.expect("Timeout");
    }

    async fn subscribe_to_topic(client: &mut PeerToPeerService, topic: String, logger: Arc<RwLock<LogHandler>>) {
        client.subscribe_to_topic(topic.clone()).await.unwrap();

        let mut found_event = false;
        while !found_event {
            let log_read = logger.read().await;
            for event in &(*log_read).events {
                if let LogEvent::SubscribedToTopic(subs) = event {
                    if subs.eq(&topic) {
                        found_event = true;
                    }
                }
            }
        }
    }

    async fn pair_to_peer(client: &mut PeerToPeerService, peer_id: &PeerId, logger: Arc<RwLock<LogHandler>>) {
        client.pair_to_another_peer(peer_id.clone()).await.unwrap();

        let mut found_event = false;
        while !found_event {
            let log_read = logger.read().await;
            for event in &(*log_read).events {
                if let LogEvent::ConnectionEstablished(peer_id_connected) = event {
                    found_event = true;
                    break;
                }
            }
        }
    }

    #[tokio::test]
    async fn failure_to_identify_peer_inhibits_connection() {
        const TOPIC_NAME: &str = "SomeTopic";

        tokio::time::timeout(Duration::from_secs(TIMEOUT_SECS), async {
            let (mut second_client, mut log_handler, second_client_peer_id, second_client_cache, _, _, second_client_address) =
                create_service(HashMap::new(), false).await;

            subscribe_to_topic(&mut second_client, TOPIC_NAME.to_string(), log_handler.clone()).await;

            let (
                mut first_client,
                mut first_client_log_handler,
                first_client_peer_id,
                first_client_cache,
                _,
                _,
                _
            ) = create_service(second_client_address, true).await;

            pair_to_peer(&mut first_client, &second_client_peer_id, first_client_log_handler.clone()).await;

            let log_handler_read = first_client_log_handler.read().await;

            let mut found_event = false;
            while !found_event {
                let events = &(*log_handler_read).events;
                for i in events {
                    if let LogEvent::PeerConnectionClosed(peer) = i {
                        if *peer == second_client_peer_id {
                            found_event = true;
                            break;
                        }
                    }
                }
            }
        }).await.expect("Timeout");
    }

    #[tokio::test]
    async fn subscribe_to_common_channel_after_pair() {
        tokio::time::timeout(Duration::from_secs(TIMEOUT_SECS), async {
            let (mut second_client, mut log_handler, second_client_peer_id, second_client_cache, _, _, second_client_addr) =
                create_service(HashMap::new(), true).await;;

            let (
                mut first_client,
                mut first_client_log_handler,
                first_client_peer_id,
                first_client_cache,
                _,
                _,
                _
            ) = create_service(second_client_addr, true).await;

            pair_to_peer(&mut first_client, &second_client_peer_id, log_handler.clone()).await;

            let mut found_event = false;
            while !found_event {
                let log_handler = first_client_log_handler.read().await;
                let events = &(*log_handler).events;

                for event in events {
                    if let LogEvent::SubscribedToTopic(topic) = event {
                        found_event = true;
                    }
                }
            }
        }).await.expect("Timeout");
    }
}
