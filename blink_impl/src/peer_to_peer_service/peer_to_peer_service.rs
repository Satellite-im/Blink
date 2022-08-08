use crate::{
    peer_to_peer_service::did_keypair_to_libp2p_keypair,
    peer_to_peer_service::behavior::{BehaviourEvent, BlinkBehavior},
    peer_to_peer_service::{libp2p_pub_to_did, CancellationToken}
};
use anyhow::Result;
use blink_contract::{Event, EventBus};
use did_key::{Ed25519KeyPair, Generate, KeyMaterial, ECDH};
use hmac_sha512::Hash;
use libp2p::{
    gossipsub::{Sha256Topic, TopicHash},
    mdns::MdnsEvent,
    swarm::dial_opts::DialOpts,
    core::transport::upgrade,
    futures::StreamExt,
    gossipsub::GossipsubEvent,
    identify::IdentifyEvent,
    identity::Keypair,
    kad::{KademliaEvent, QueryResult},
    mplex,
    noise,
    swarm::{NetworkBehaviour, SwarmBuilder, SwarmEvent},
    tcp::{GenTcpConfig, TokioTcpTransport},
    Multiaddr,
    PeerId,
    Swarm,
    Transport
};
use sata::Sata;
use std::{
    collections::HashMap,
    sync::{atomic::Ordering, Arc},
};
use tokio::{
    sync::{
        mpsc::{
            Receiver,
            Sender
        },
        RwLock
    },
    task::JoinHandle
};
use warp::{
    crypto::DID,
    data::DataType,
    multipass::{identity::Identifier, MultiPass},
    pocket_dimension::PocketDimension
};

pub type TopicName = String;

pub type MessageContent = (TopicHash, Sata);

const CHANNEL_SIZE: usize = 64;

#[derive(Debug)]
pub enum BlinkCommand {
    FindNearest(PeerId),
    Dial(DialOpts),
    Subscribe(String),
    PublishToTopic(TopicName, Sata),
}

pub struct PeerToPeerService {
    command_channel: Sender<BlinkCommand>,
    task_handle: JoinHandle<()>,
}

impl Drop for PeerToPeerService {
    fn drop(&mut self) {
        self.task_handle.abort();
    }
}

impl PeerToPeerService {
    pub async fn new(
        did_key: Arc<RwLock<DID>>,
        address_to_listen: &str,
        initial_known_address: Option<HashMap<PeerId, Multiaddr>>,
        cache: Arc<RwLock<impl PocketDimension + 'static>>,
        multi_pass: Arc<RwLock<impl MultiPass + 'static>>,
        logger: Arc<RwLock<impl EventBus + 'static>>,
        cancellation_token: CancellationToken,
    ) -> Result<(Self, Receiver<MessageContent>)> {
        let key_pair = {
            let did_read = did_key.read().await;
            did_keypair_to_libp2p_keypair((*did_read).as_ref())?
        };
        let pub_key = key_pair.public();
        let peer_id = PeerId::from(&pub_key);
        let mut swarm = Self::create_swarm(&key_pair, &peer_id).await?;
        if let Some(initial_address) = initial_known_address {
            for (peer, addr) in initial_address {
                let behaviour = swarm.behaviour_mut();
                behaviour.kademlia.add_address(&peer, addr);
                behaviour.gossip_sub.add_explicit_peer(&peer_id);
            }
        }

        swarm.listen_on(address_to_listen.parse()?)?;

        let (command_tx, mut command_rx) = tokio::sync::mpsc::channel(CHANNEL_SIZE);
        let (message_tx, message_rx) = tokio::sync::mpsc::channel(CHANNEL_SIZE);
        let cache_to_thread = cache.clone();
        let thread_logger = logger.clone();
        let multi_pass_thread = multi_pass.clone();
        let did_thread = did_key.clone();

        let handler = tokio::spawn(async move {
            loop {
                if cancellation_token.load(Ordering::Acquire) {
                    let mut log_write = thread_logger.write().await;
                    (*log_write).event_occurred(Event::TaskCancelled);
                }

                tokio::select! {
                     cmd = command_rx.recv() => {
                         if let Some(command) = cmd {
                             Self::handle_command(&mut swarm, command, thread_logger.clone()).await;
                         }
                     },
                    event = swarm.select_next_some() => {
                         Self::handle_event(&mut swarm, event, cache_to_thread.clone(),
                            thread_logger.clone(), multi_pass_thread.clone(), &message_tx, did_thread.clone()).await;
                    }
                }
            }
        });

        Ok((Self {
            command_channel: command_tx,
            task_handle: handler
        }, message_rx))
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
                            .await
                            .event_occurred(Event::DialSuccessful(peer_id));
                    }
                    Err(err) => {
                        logger
                            .write()
                            .await
                            .event_occurred(Event::DialError(err.to_string()));
                    }
                }
            }
            BlinkCommand::Subscribe(address) => {
                let topic = Sha256Topic::new(address.clone());
                match swarm.behaviour_mut().gossip_sub.subscribe(&topic) {
                    Ok(_) => {
                        let mut service = logger.write().await;
                        service.event_occurred(Event::SubscribedToTopic(address));
                    }
                    Err(e) => {
                        let mut service = logger.write().await;
                        service.event_occurred(Event::SubscriptionError(e.to_string()))
                    }
                }
            }
            BlinkCommand::PublishToTopic(name, sata) => {
                let serialized_result = bincode::serialize(&sata);
                match serialized_result {
                    Ok(serialized) => {
                        let topic = Sha256Topic::new(name);
                        if let Err(err) =
                            swarm.behaviour_mut().gossip_sub.publish(topic, serialized)
                        {
                            let mut log_service = logger.write().await;
                            (*log_service)
                                .event_occurred(Event::ErrorPublishingData(err.to_string()));
                        }
                    }
                    Err(_) => {
                        let mut log_service = logger.write().await;
                        (*log_service).event_occurred(Event::ErrorSerializingData);
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
                                    let private_key_pair = Ed25519KeyPair::from_secret_key(
                                        &(*private_read).as_ref().private_key_bytes(),
                                    )
                                    .get_x25519();
                                    let public_key_pair = Ed25519KeyPair::from_public_key(
                                        &their_public.as_ref().public_key_bytes(),
                                    )
                                    .get_x25519();
                                    let exchange = private_key_pair.key_exchange(&public_key_pair);
                                    let hashed = Hash::hash(exchange);
                                    let topic = base64::encode(hashed);
                                    let topic_subs = Sha256Topic::new(&topic);
                                    match swarm.behaviour_mut().gossip_sub.subscribe(&topic_subs) {
                                        Ok(_) => {
                                            let mut log = logger.write().await;
                                            (*log).event_occurred(Event::SubscribedToTopic(topic));
                                            (*log).event_occurred(Event::PeerIdentified);
                                        }
                                        Err(er) => {
                                            let mut log = logger.write().await;
                                            (*log).event_occurred(Event::SubscriptionError(
                                                er.to_string(),
                                            ));
                                        }
                                    }
                                }
                                Err(_) => {
                                    let mut log = logger.write().await;
                                    (*log).event_occurred(Event::FailureToIdentifyPeer);
                                    if swarm.disconnect_peer_id(peer_id).is_err() {
                                        (*log).event_occurred(Event::FailureToDisconnectPeer);
                                    }
                                }
                            }
                        }
                        Err(_) => {
                            let mut log = logger.write().await;
                            (*log).event_occurred(Event::ConvertKeyError);
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
                                log_service
                                    .event_occurred(Event::ErrorAddingToCache(e.enum_to_string()));
                            }
                            if let Err(_) = message_sender.send((message.topic, info.clone())).await
                            {
                                let mut log_service = logger.write().await;
                                log_service.event_occurred(Event::FailedToSendMessage);
                            }
                        }
                        Err(_) => {
                            let mut log_service = logger.write().await;
                            log_service.event_occurred(Event::ErrorDeserializingData);
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
                (*log_service).event_occurred(Event::ConnectionEstablished(peer_id.to_string()));
            }
            SwarmEvent::ConnectionClosed { peer_id, .. } => {
                let mut log_service = logger.write().await;
                (*log_service).event_occurred(Event::PeerConnectionClosed(peer_id.to_string()));
            }
            SwarmEvent::IncomingConnection { .. } => {}
            SwarmEvent::IncomingConnectionError { .. } => {}
            SwarmEvent::OutgoingConnectionError { .. } => {}
            SwarmEvent::BannedPeer { .. } => {}
            SwarmEvent::NewListenAddr { address, .. } => {
                let mut log_service = logger.write().await;
                (*log_service).event_occurred(Event::NewListenAddr(address));
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

    pub async fn pair_to_another_peer(&mut self, dial_opts: DialOpts) -> Result<()> {
        self.command_channel
            .send(BlinkCommand::Dial(dial_opts))
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
    use crate::peer_to_peer_service::did_keypair_to_libp2p_keypair;
    use crate::peer_to_peer_service::peer_to_peer_service::{MessageContent, PeerToPeerService};
    use blink_contract::{Event, EventBus};
    use did_key::Ed25519KeyPair;
    use libp2p::{Multiaddr, PeerId};
    use sata::Sata;
    use std::{collections::HashMap, sync::atomic::AtomicBool, sync::Arc, time::Duration};
    use tokio::sync::mpsc::Receiver;
    use tokio::sync::RwLock;
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

    const TIMEOUT_SECS: u64 = 1;

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
        fn create_identity(&mut self, _: Option<&str>, _: Option<&str>) -> Result<DID, Error> {
            todo!()
        }

        fn get_identity(&self, _: Identifier) -> Result<Identity, Error> {
            if self.pass_as_valid {
                return Ok(Identity::default());
            }

            Err(Error::IdentityDoesntExist)
        }

        fn update_identity(&mut self, _: IdentityUpdate) -> Result<(), Error> {
            todo!()
        }

        fn decrypt_private_key(&self, passphrase: Option<&str>) -> Result<DID, Error> {
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

        fn has_data(&mut self, _: DataType, _: &QueryBuilder) -> Result<(), Error> {
            todo!()
        }

        fn get_data(&self, _: DataType, _: Option<&QueryBuilder>) -> Result<Vec<Sata>, Error> {
            todo!()
        }

        fn size(&self, _: DataType, _: Option<&QueryBuilder>) -> Result<i64, Error> {
            todo!()
        }

        fn count(&self, _: DataType, _: Option<&QueryBuilder>) -> Result<i64, Error> {
            todo!()
        }

        fn empty(&mut self, _: DataType) -> Result<(), Error> {
            todo!()
        }
    }

    struct LogHandler {
        pub events: Vec<Event>,
    }

    impl LogHandler {
        fn new() -> Self {
            Self { events: Vec::new() }
        }
    }

    impl EventBus for LogHandler {
        fn event_occurred(&mut self, event: Event) {
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
        HashMap<PeerId, Multiaddr>,
        Receiver<MessageContent>
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
        let (service, receiver) = PeerToPeerService::new(
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
                if let Event::NewListenAddr(addr) = event {
                    break_loop = true;
                    addr_to_send = Some(addr.clone());
                    break;
                }
            }
        }

        let mut map = HashMap::new();
        map.insert(peer_id.clone(), addr_to_send.unwrap());

        (
            service,
            log_handler,
            peer_id,
            cache,
            multi_pass,
            id_keys,
            map,
            receiver
        )
    }

    async fn subscribe_to_topic(
        client: &mut PeerToPeerService,
        topic: String,
        logger: Arc<RwLock<LogHandler>>,
    ) {
        client.subscribe_to_topic(topic.clone()).await.unwrap();

        let mut found_event = false;
        while !found_event {
            let log_read = logger.read().await;
            for event in &(*log_read).events {
                if let Event::SubscribedToTopic(subs) = event {
                    if subs.eq(&topic) {
                        found_event = true;
                    }
                }
            }
        }
    }

    #[tokio::test]
    async fn open_does_not_throw() {
        tokio::time::timeout(Duration::from_secs(TIMEOUT_SECS), async {
            create_service(HashMap::new(), true).await;
        })
        .await
        .expect("timeout");
    }

    #[tokio::test]
    async fn connecting_to_peer_does_not_generate_errors() {
        tokio::time::timeout(Duration::from_secs(TIMEOUT_SECS), async {
            let (_, _, peer_id, _, _, _, addr_map, _)
                = create_service(HashMap::new(), true).await;

            let (mut first_client, _, _, _, _, _, _, _) = create_service(addr_map, true).await;

            first_client.pair_to_another_peer(peer_id.into()).await.unwrap();
        })
        .await
        .expect("Timeout");
    }

    #[tokio::test]
    async fn subscribe_to_topic_does_not_cause_errors() {
        tokio::time::timeout(Duration::from_secs(TIMEOUT_SECS), async {
            let (mut client, log_handler, _, _, _, _, _, _) =
                create_service(HashMap::new(), true).await;

            subscribe_to_topic(&mut client, "some topic".to_string(), log_handler.clone()).await;
        })
        .await
        .expect("Timeout");
    }

    #[tokio::test]
    async fn message_reaches_other_client() {
        const TOPIC_NAME: &str = "SomeTopic";
        tokio::time::timeout(Duration::from_secs(TIMEOUT_SECS), async {
            let (
                mut second_client,
                log_handler,
                second_client_peer_id,
                second_client_cache,
                _,
                _,
                second_client_addr,
                mut message_rx
            ) = create_service(HashMap::new(), true).await;

            let (mut first_client, first_client_log_handler, _, _, _, _, _, _) =
                create_service(second_client_addr, true).await;

            first_client
                .pair_to_another_peer(second_client_peer_id.into())
                .await
                .unwrap();

            subscribe_to_topic(
                &mut first_client,
                TOPIC_NAME.to_string(),
                first_client_log_handler.clone(),
            )
            .await;

            subscribe_to_topic(
                &mut second_client,
                TOPIC_NAME.to_string(),
                log_handler.clone(),
            )
            .await;

            let mut connection_ok = false;

            // wait for connection to be good to go
            while !connection_ok {
                let first_client_log_read = first_client_log_handler.read().await;
                let events = &(*first_client_log_read).events;

                for event in events {
                    if let Event::PeerIdentified = event {
                        connection_ok = true;
                        break;
                    }
                }
            }

            first_client
                .publish_message_to_topic(TOPIC_NAME.to_string(), Sata::default())
                .await
                .unwrap();

            while message_rx.recv().await.is_none() {}
        })
        .await
        .expect("Timeout");
    }

    #[tokio::test]
    async fn message_to_another_client_is_added_to_cache() {
        const TOPIC_NAME: &str = "SomeTopic";
        tokio::time::timeout(Duration::from_secs(TIMEOUT_SECS), async {
            let (
                mut second_client,
                log_handler,
                second_client_peer_id,
                second_client_cache,
                _,
                _,
                second_client_addr,
                _
            ) = create_service(HashMap::new(), true).await;

            let (mut first_client, first_client_log_handler, _, _, _, _, _, message_rx) =
                create_service(second_client_addr, true).await;

            first_client
                .pair_to_another_peer(second_client_peer_id.into())
                .await
                .unwrap();

            subscribe_to_topic(
                &mut first_client,
                TOPIC_NAME.to_string(),
                first_client_log_handler.clone(),
            )
            .await;

            subscribe_to_topic(
                &mut second_client,
                TOPIC_NAME.to_string(),
                log_handler.clone(),
            )
            .await;

            let mut connection_ok = false;

            // wait for connection to be good to go
            while !connection_ok {
                let first_client_log_read = first_client_log_handler.read().await;
                let events = &(*first_client_log_read).events;

                for event in events {
                    if let Event::PeerIdentified = event {
                        connection_ok = true;
                        break;
                    }
                }
            }

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
        })
        .await
        .expect("Timeout");
    }

    #[tokio::test]
    async fn failure_to_identify_peer_causes_error() {
        tokio::time::timeout(Duration::from_secs(TIMEOUT_SECS), async {
            let (_, _, first_client_peer_id, _, _, _, first_client_address, _) =
                create_service(HashMap::new(), false).await;

            let (mut second_client, log_handler_second_client, _, _, _, _, _, _) =
                create_service(first_client_address, false).await;

            second_client
                .pair_to_another_peer(first_client_peer_id.into())
                .await
                .unwrap();

            let mut found_error = false;
            while !found_error {
                let log_handler_read = log_handler_second_client.read().await;
                for event in &(*log_handler_read).events {
                    if let Event::FailureToIdentifyPeer = event {
                        found_error = true;
                    }
                }
            }
        })
        .await
        .expect("Timeout");
    }

    #[tokio::test]
    async fn subscribe_to_common_channel_after_pair() {
        tokio::time::timeout(Duration::from_secs(TIMEOUT_SECS), async {
            let (_, log_handler, second_client_peer_id, _, _, _, second_client_addr, _) =
                create_service(HashMap::new(), true).await;

            let (mut first_client, first_client_log_handler, _, _, _, _, _, _) =
                create_service(second_client_addr, true).await;

            first_client
                .pair_to_another_peer(second_client_peer_id.into())
                .await
                .unwrap();

            let mut found_event = false;
            while !found_event {
                let log_handler = first_client_log_handler.read().await;
                let events = &(*log_handler).events;

                for event in events {
                    if let Event::SubscribedToTopic(_) = event {
                        found_event = true;
                    }
                }
            }
        })
        .await
        .expect("Timeout");
    }
}
