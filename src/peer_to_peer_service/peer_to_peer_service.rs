use crate::peer_to_peer_service::behavior::{BehaviourEvent, BlinkBehavior};
use crate::peer_to_peer_service::{CancellationToken, LogEvent, Logger};
use anyhow::Result;
use libp2p::core::transport::upgrade;
use libp2p::futures::StreamExt;
use libp2p::gossipsub::{GossipsubEvent, Sha256Topic, Topic};
use libp2p::identity::Keypair;
use libp2p::kad::{KademliaEvent, QueryResult};
use libp2p::swarm::{NetworkBehaviour, SwarmBuilder, SwarmEvent};
use libp2p::tcp::{GenTcpConfig, TokioTcpTransport};
use libp2p::Transport;
use libp2p::{mplex, noise, Multiaddr, PeerId, Swarm};
use sata::Sata;
use std::collections::HashMap;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use libp2p::identify::IdentifyEvent;
use tokio::sync::mpsc::Sender;
use tokio::sync::RwLock;
use tokio::task::JoinHandle;
use warp::data::{DataObject, DataType};
use warp::pocket_dimension::PocketDimension;

#[derive(Debug)]
pub enum BlinkCommand {
    FindNearest(PeerId),
    Dial(PeerId),
    Subscribe(String),
}

pub struct PeerToPeerService<TLogger: Logger + Send + Sync + 'static> {
    command_channel: Sender<BlinkCommand>,
    task_handle: JoinHandle<()>,
    local_addr: Arc<RwLock<Vec<Multiaddr>>>,
    cache: Arc<RwLock<Box<dyn PocketDimension>>>,
    logger: Arc<RwLock<TLogger>>,
}

impl<TLogger: Logger + Send + Sync + 'static> Drop for PeerToPeerService<TLogger> {
    fn drop(&mut self) {
        self.task_handle.abort();
    }
}

impl<TLogger: Logger + Send + Sync + 'static> PeerToPeerService<TLogger> {
    pub fn new(
        key_pair: Keypair,
        address_to_listen: &str,
        initial_known_address: Option<HashMap<PeerId, Multiaddr>>,
        cache: Arc<RwLock<Box<dyn PocketDimension>>>,
        logger: Arc<RwLock<TLogger>>,
        cancellation_token: CancellationToken,
    ) -> Result<Self> {
        let mut swarm = Self::create_swarm(key_pair)?;
        if let Some(initial_address) = initial_known_address {
            for (peer, addr) in initial_address {
                swarm.behaviour_mut().kademlia.add_address(&peer, addr);
            }
        }

        swarm.listen_on(address_to_listen.parse()?)?;

        let (command_tx, mut command_rx) = tokio::sync::mpsc::channel(32);
        let local_address = Arc::new(RwLock::new(Vec::new()));
        let thread_addr_to_set = local_address.clone();
        let cache_to_thread = cache.clone();
        let thread_logger = logger.clone();

        let handler = tokio::spawn(async move {
            loop {
                if cancellation_token.load(Ordering::Acquire) {
                    println!("leaving the loop");
                    break;
                }

                tokio::select! {
                     cmd = command_rx.recv() => {
                         if let Some(command) = cmd {
                             Self::handle_command(&mut swarm, command, thread_logger.clone()).await;
                         }
                     },
                    event = swarm.select_next_some() => {
                         if let SwarmEvent::Behaviour(_) = event {
                             Self::handle_behaviour_event(&mut swarm, event, cache_to_thread.clone(), thread_logger.clone()).await;
                         } else {
                             Self::handle_event(&mut swarm, event, thread_addr_to_set.clone(), thread_logger.clone()).await;
                         }
                    }
                }
            }
        });

        Ok(Self {
            command_channel: command_tx,
            task_handle: handler,
            local_addr: local_address,
            cache,
            logger,
        })
    }

    async fn handle_command(
        swarm: &mut Swarm<BlinkBehavior>,
        command: BlinkCommand,
        logger: Arc<RwLock<TLogger>>,
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
                let topic = Sha256Topic::new(address);
                if let Err(e) = swarm.behaviour_mut().gossip_sub.subscribe(&topic) {
                    let mut service = logger.write().await;
                    service.event_occurred(LogEvent::SubscriptionError(e.to_string()))
                }
            }
        }
    }

    async fn handle_behaviour_event<TErr>(
        swarm: &mut Swarm<BlinkBehavior>,
        event: SwarmEvent<BehaviourEvent, TErr>,
        cache: Arc<RwLock<Box<dyn PocketDimension>>>,
        logger: Arc<RwLock<TLogger>>,
    ) {
        match event {
            SwarmEvent::Behaviour(BehaviourEvent::IdentifyEvent(identify)) => {
                match identify {
                    IdentifyEvent::Received { peer_id, info} => {
                        let public_key = info.public_key;


                    }
                    IdentifyEvent::Sent { .. } => {}
                    IdentifyEvent::Pushed { .. } => {}
                    IdentifyEvent::Error { .. } => {}
                }
            }
            SwarmEvent::Behaviour(BehaviourEvent::Gossipsub(gsp)) => {
                match gsp {
                    GossipsubEvent::Message { message, .. } => {
                        let message_data = message.data;
                        let data = bincode::deserialize::<Sata>(&message_data);
                        match data {
                            Ok(info) => {
                                let mut write = cache.write().await;
                                // if let Err(e) = write.add_data(DataType::Messaging, info) {
                                //     println!("Error while writing data to cache");
                                // }
                            }
                            Err(err) => {
                                println!("Error while deserializing {}", err);
                            }
                        }
                    }
                    GossipsubEvent::Subscribed { .. } => {}
                    GossipsubEvent::Unsubscribed { .. } => {}
                    GossipsubEvent::GossipsubNotSupported { .. } => {}
                }
            }
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
            _ => {}
        }
    }

    async fn handle_event<TEvent, TErr>(
        _: &mut Swarm<BlinkBehavior>,
        event: SwarmEvent<TEvent, TErr>,
        local_address: Arc<RwLock<Vec<Multiaddr>>>,
        logger: Arc<RwLock<TLogger>>,
    ) {
        match event {
            SwarmEvent::ConnectionEstablished { .. } => {}
            SwarmEvent::ConnectionClosed { .. } => {}
            SwarmEvent::IncomingConnection { .. } => {}
            SwarmEvent::IncomingConnectionError { .. } => {}
            SwarmEvent::OutgoingConnectionError { .. } => {}
            SwarmEvent::BannedPeer { .. } => {}
            SwarmEvent::NewListenAddr { address, .. } => {
                let mut write = local_address.write().await;
                (*write).push(address);
            }
            SwarmEvent::ExpiredListenAddr { .. } => {}
            SwarmEvent::ListenerClosed { .. } => {}
            SwarmEvent::ListenerError { .. } => {}
            SwarmEvent::Dialing(_) => {}
            _ => {}
        }
    }

    fn create_swarm(key_pair: Keypair) -> Result<Swarm<BlinkBehavior>> {
        let peer_id = PeerId::from(&key_pair.public());
        let blink_behaviour = BlinkBehavior::new(&key_pair)?;

        // Create a keypair for authenticated encryption of the transport.
        let noise_keys = noise::Keypair::<noise::X25519Spec>::new().into_authentic(&key_pair)?;

        // Create a tokio-based TCP transport use noise for authenticated
        // encryption and Mplex for multiplexing of substreams on a TCP stream.
        let transport = TokioTcpTransport::new(GenTcpConfig::default().nodelay(true))
            .upgrade(upgrade::Version::V1)
            .authenticate(noise::NoiseConfig::xx(noise_keys).into_authenticated())
            .multiplex(mplex::MplexConfig::new())
            .boxed();

        let swarm = SwarmBuilder::new(transport, blink_behaviour, peer_id)
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

    async fn get_local_address(&self) -> Vec<Multiaddr> {
        let read = self.local_addr.read().await;
        read.clone()
    }

    async fn pair_to_another_peer(&mut self, peer_id: PeerId) -> Result<()> {
        self.command_channel
            .send(BlinkCommand::Dial(peer_id))
            .await?;
        Ok(())
    }
}

#[cfg(test)]
mod when_using_peer_to_peer_service {
    use crate::peer_to_peer_service::peer_to_peer_service::PeerToPeerService;
    use crate::peer_to_peer_service::{LogEvent, Logger};
    use libp2p::{identity, PeerId};
    use std::collections::HashMap;
    use std::sync::atomic::AtomicBool;
    use std::sync::Arc;
    use std::time::Duration;
    use tokio::sync::RwLock;
    use warp::data::{DataObject, DataType};
    use warp::error::Error;
    use warp::module::Module;
    use warp::pocket_dimension::query::QueryBuilder;
    use warp::pocket_dimension::PocketDimension;
    use warp::{Extension, SingleHandle};

    #[derive(Default)]
    struct TestCache {}

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
        fn add_data(&mut self, dimension: DataType, data: &DataObject) -> Result<(), Error> {
            todo!()
        }

        fn has_data(&mut self, dimension: DataType, query: &QueryBuilder) -> Result<(), Error> {
            todo!()
        }

        fn get_data(
            &self,
            dimension: DataType,
            query: Option<&QueryBuilder>,
        ) -> Result<Vec<DataObject>, Error> {
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

    #[tokio::test]
    async fn open_does_not_throw() {
        let id_keys = identity::Keypair::generate_ed25519();
        let cancellation_token = Arc::new(AtomicBool::new(false));
        let cache: Arc<RwLock<Box<dyn PocketDimension>>> =
            Arc::new(RwLock::new(Box::new(TestCache::default())));
        let log_handler = Arc::new(RwLock::new(LogHandler::new()));
        let service = PeerToPeerService::new(
            id_keys,
            "/ip4/0.0.0.0/tcp/0",
            None,
            cache.clone(),
            log_handler.clone(),
            cancellation_token.clone(),
        )
        .unwrap();
        tokio::time::sleep(Duration::from_secs(1)).await;
        let addr = service.get_local_address().await;
        assert!(addr.len() > 0);
    }

    #[tokio::test]
    async fn connecting_to_peer_does_not_generate_errors() {
        let cancellation_token = Arc::new(AtomicBool::new(false));
        let mut addr_map = HashMap::new();

        let log_handler = Arc::new(RwLock::new(LogHandler::new()));
        let second_client_id = identity::Keypair::generate_ed25519();
        let second_client_peer = PeerId::from(second_client_id.public());
        let cache: Arc<RwLock<Box<dyn PocketDimension>>> =
            Arc::new(RwLock::new(Box::new(TestCache::default())));
        let second_client = PeerToPeerService::new(
            second_client_id,
            "/ip4/0.0.0.0/tcp/0",
            None,
            cache.clone(),
            log_handler.clone(),
            cancellation_token.clone(),
        )
        .unwrap();

        tokio::time::sleep(Duration::from_secs(1)).await;
        let addr = second_client.get_local_address().await;
        addr_map.insert(second_client_peer, addr[0].clone());

        let first_client_id = identity::Keypair::generate_ed25519();

        let mut first_client = PeerToPeerService::new(
            first_client_id,
            "/ip4/0.0.0.0/tcp/0",
            Some(addr_map),
            cache.clone(),
            log_handler.clone(),
            cancellation_token.clone(),
        )
        .unwrap();

        first_client.pair_to_another_peer(second_client_peer).await.unwrap();
        tokio::time::sleep(Duration::from_secs(1)).await;

        assert!(log_handler.read().await.events.is_empty());
    }

    #[tokio::test]
    async fn subscribe_to_topic_does_not_cause_errors() {
        let cancellation_token = Arc::new(AtomicBool::new(false));

        let log_handler = Arc::new(RwLock::new(LogHandler::new()));
        let client_id = identity::Keypair::generate_ed25519();
        let cache: Arc<RwLock<Box<dyn PocketDimension>>> =
            Arc::new(RwLock::new(Box::new(TestCache::default())));
        let mut client = PeerToPeerService::new(
            client_id,
            "/ip4/0.0.0.0/tcp/0",
            None,
            cache.clone(),
            log_handler.clone(),
            cancellation_token.clone(),
        )
        .unwrap();

        client
            .subscribe_to_topic("some channel".to_string())
            .await
            .unwrap();

        tokio::time::sleep(Duration::from_secs(1)).await;

        log_handler.read().await.events.iter().all(|x| {
            if let LogEvent::SubscriptionError(_) = *x {
                return true;
            }
            false
        });
    }
}
