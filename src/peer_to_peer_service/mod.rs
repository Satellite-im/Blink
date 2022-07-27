mod behavior;

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
use std::time::Duration;
use libp2p::core::PublicKey;
use libp2p::kad::{GetClosestPeersResult, KademliaEvent, QueryId, QueryResult};
use libp2p::kad::PutRecordPhase::GetClosestPeers;
use tokio::sync::mpsc::Sender;
use tokio::sync::RwLock;

type CancellationToken = Arc<RwLock<bool>>;

pub enum BlinkCommand {
    Pair(Vec<PeerId>)
}

pub struct PeerToPeerService {
    command_channel: Sender<BlinkCommand>,
}

trait BlinkCommandHandler {
    fn can_handle(command: &BlinkCommand) -> bool;
    fn handle(swarm: &mut Swarm<BlinkBehavior>, command: &BlinkCommand);
}

#[derive(Default)]
struct PairCommandHandler {
}

impl BlinkCommandHandler for PairCommandHandler {
    fn can_handle(command: &BlinkCommand) -> bool {
        if let BlinkCommand::Pair(x) = command {
            return true;
        }

        false
    }

    fn handle(swarm: &mut Swarm<BlinkBehavior>, command: &BlinkCommand) {
        if let BlinkCommand::Pair(peers) = command {
            for peer in peers {
                swarm.behaviour_mut().find_peer(*peer);
            }
        }
    }
}

trait EventHandler {
    fn can_handle<TErr>(event: &SwarmEvent<BehaviourEvent, TErr>) -> bool;
    fn handle<TErr>(swarm: &mut Swarm<BlinkBehavior>, event: &SwarmEvent<BehaviourEvent, TErr>);
}

#[derive(Default)]
struct PairResultEventHandler {
}

impl EventHandler for PairResultEventHandler {
    fn can_handle<TErr>(event: &SwarmEvent<BehaviourEvent, TErr>) -> bool {
        if let SwarmEvent::Behaviour(BehaviourEvent::KademliaEvent(ev)) = event {
            if let KademliaEvent::OutboundQueryCompleted { .. } = ev {
                return true;
            }
        }

        false
    }

    fn handle<TErr>(swarm: &mut Swarm<BlinkBehavior>, event: &SwarmEvent<BehaviourEvent, TErr>) {
        if let SwarmEvent::Behaviour(BehaviourEvent::KademliaEvent(ev)) = event {
            if let KademliaEvent::OutboundQueryCompleted { result: outbound, .. } = ev {
                match outbound {
                    QueryResult::Bootstrap(_) => {}
                    QueryResult::GetClosestPeers(_) => {}
                    QueryResult::GetProviders(_) => {}
                    QueryResult::StartProviding(_) => {}
                    QueryResult::RepublishProvider(_) => {}
                    QueryResult::GetRecord(_) => {}
                    QueryResult::PutRecord(_) => {}
                    QueryResult::RepublishRecord(_) => {}
                }
            }
        }
    }
}


impl PeerToPeerService {
    pub fn new(key_pair: Keypair, address_to_listen: &str, cancellation_token: CancellationToken) -> Result<Self> {
        let mut swarm = Self::create_swarm(key_pair)?;
        swarm.listen_on(address_to_listen.parse()?)?;

        let (command_tx, mut command_rx) = tokio::sync::mpsc::channel(32);

        tokio::spawn(async move {
            loop {
                let cnl = cancellation_token.read().await;

                if *cnl {
                    break;
                }

                tokio::select! {
                    cmd = command_rx.recv() => {
                        Self::handle_command(&mut swarm, cmd);
                    },
                   event = swarm.select_next_some() => {
                        Self::handle_event(&mut swarm, event);
                   }
               }
            }
        });

        // connect to relay?

        Ok(Self {
            command_channel: command_tx
        })
    }

    fn handle_command(swarm: &mut Swarm<BlinkBehavior>, command: Option<BlinkCommand>) {
        todo!()
    }

    fn handle_event<HandlerErr>(swarm: &mut Swarm<BlinkBehavior>, event: SwarmEvent<BehaviourEvent, HandlerErr>) {
        todo!()
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

    pub async fn pair(&mut self, peers_to_connect: &[PublicKey]) {
        self.command_channel.send(BlinkCommand::Pair(peers_to_connect.iter().map(|x| PeerId::from(x)).collect()));
    }
}

#[cfg(test)]
mod when_using_peer_to_peer_service {
    use std::sync::Arc;
    use crate::peer_to_peer_service::PeerToPeerService;
    use libp2p::{identity, PeerId};
    use tokio::sync::RwLock;

    // #[tokio::test]
    // async fn listen_does_not_throw() {
    //     let id_keys = identity::Keypair::generate_ed25519();
    //     let mut service = PeerToPeerService::new(id_keys).unwrap();
    //     let cancellation_token = Arc::new(RwLock::new(false));
    //     service.open("/ip4/0.0.0.0/tcp/0", cancellation_token.clone()).await.unwrap();
    // }
    //
    // #[tokio::test]
    // async fn connecting_to_a_sequence_of_peers_generates_specific_events() {
    //     let id_keys = identity::Keypair::generate_ed25519();
    //     let mut service = PeerToPeerService::new(id_keys).unwrap();
    //     let cancellation_token = Arc::new(RwLock::new(false));
    //     service.open("/ip4/0.0.0.0/tcp/0", cancellation_token.clone()).await.unwrap();
    //
    //     let second_client = identity::Keypair::generate_ed25519();
    //     service.pair(&vec![second_client.public()]).await;
    // }
}
