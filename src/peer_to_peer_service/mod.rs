mod behavior;
mod command_handlers;
mod event_handlers;

use std::sync::Arc;
use crate::peer_to_peer_service::behavior::{BehaviourEvent, BlinkBehavior};
use anyhow::Result;
use libp2p::{
    core::transport::upgrade,
    futures::StreamExt,
    identity::Keypair,
    mplex, noise,
    swarm::{SwarmBuilder, SwarmEvent},
    tcp::GenTcpConfig,
    tcp::TokioTcpTransport,
    PeerId, Swarm, Transport,
};
use libp2p::{
    core::PublicKey,
    kad::{GetClosestPeersResult, KademliaEvent, QueryId, QueryResult},
    kad::PutRecordPhase::GetClosestPeers
};
use tokio::sync::mpsc::Sender;
use tokio::sync::{Mutex, RwLock};
use tokio::task::JoinHandle;
use crate::peer_to_peer_service::command_handlers::{BlinkCommandHandler, PairCommandHandler};
use crate::peer_to_peer_service::event_handlers::EventHandler;

type CancellationToken = Arc<RwLock<bool>>;

#[derive(Debug)]
pub enum BlinkCommand {
    Pair(Vec<PeerId>)
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

pub trait ILogger {
    fn info(&mut self, info: &String);
}

impl PeerToPeerService {
    pub fn new(key_pair: Keypair,
               address_to_listen: &str,
               logger: Arc<Mutex<Box<dyn ILogger + Send>>>,
               cancellation_token: CancellationToken) -> Result<Self> {
        let mut swarm = Self::create_swarm(key_pair)?;
        swarm.listen_on(address_to_listen.parse()?)?;

        let mut command_handlers : Vec<Box<dyn BlinkCommandHandler + Send>> = vec![Box::new(PairCommandHandler::default())];
        let (command_tx, mut command_rx) = tokio::sync::mpsc::channel(32);

        let handler = tokio::spawn(async move {
            loop {
                let cnl = cancellation_token.read().await;

                if *cnl {
                    println!("leaving the loop");
                    break;
                }

                tokio::select! {
                    cmd = command_rx.recv() => {
                        if let Some(command) = cmd {
                            for handler in &mut command_handlers {
                                if handler.can_handle(&command) {
                                    handler.handle(&mut swarm, &command);
                                }
                            }
                        }
                    },
                   event = swarm.select_next_some() => {
                        if let SwarmEvent::NewListenAddr { address, .. } = &event {
                            let mut log = logger.lock().await;
                            log.info(&format!("Listening on {}", address));
                        } else {
                            Self::handle_event(&mut swarm, event);
                        }
                   }
               }
            }
        });

        // connect to relay?

        Ok(Self {
            command_channel: command_tx,
            task_handle: handler,
        })
    }

    fn handle_event<HandlerErr>(swarm: &mut Swarm<BlinkBehavior>, event: SwarmEvent<BehaviourEvent, HandlerErr>) {
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

    pub async fn pair(&mut self, peers_to_connect: &[PublicKey]) -> Result<()> {
        self.command_channel.send(BlinkCommand::Pair(peers_to_connect.iter().map(|x| PeerId::from(x)).collect())).await?;
        Ok(())
    }
}

#[cfg(test)]
mod when_using_peer_to_peer_service {
    use std::sync::Arc;
    use std::time::Duration;
    use crate::peer_to_peer_service::{ILogger, PeerToPeerService};
    use libp2p::{identity, PeerId};
    use tokio::sync::{Mutex, RwLock};

    #[derive(Default)]
    struct Logger {}
    unsafe impl Send for Logger {}

    impl ILogger for Logger {
        fn info(&mut self, info: &String) {
            println!("{}", info);
        }
    }

    #[tokio::test]
    async fn open_does_not_throw() {
        let id_keys = identity::Keypair::generate_ed25519();
        let cancellation_token = Arc::new(RwLock::new(false));
        let logger: Arc<Mutex<Box<dyn ILogger + Send + 'static>>>  = Arc::new(Mutex::new(Box::new(Logger::default())));
        PeerToPeerService::new(id_keys, "/ip4/0.0.0.0/tcp/0", logger.clone(), cancellation_token.clone()).unwrap();
    }

    #[tokio::test]
    async fn connecting_to_a_sequence_of_peers_generates_specific_events() {
        let id_keys = identity::Keypair::generate_ed25519();
        let cancellation_token = Arc::new(RwLock::new(false));
        let logger: Arc<Mutex<Box<dyn ILogger + Send + 'static>>> = Arc::new(Mutex::new(Box::new(Logger::default())));
        let mut service = PeerToPeerService::new(id_keys, "/ip4/0.0.0.0/tcp/0", logger.clone(), cancellation_token.clone()).unwrap();

        let second_client = identity::Keypair::generate_ed25519();
        service.pair(&vec![second_client.public()]).await.unwrap();
        tokio::time::sleep(Duration::from_secs(2)).await;
    }
}
