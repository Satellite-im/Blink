mod behavior;

use crate::peer_to_peer_service::behavior::{BehaviourEvent, BlinkBehavior};
use anyhow::Result;
use libp2p::swarm::NetworkBehaviour;
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
    kad::{KademliaEvent, QueryResult},
};
use std::sync::Arc;
use libp2p::gossipsub::GossipsubEvent;
use sata::Sata;
use tokio::sync::mpsc::Sender;
use tokio::sync::RwLock;
use tokio::task::JoinHandle;

type CancellationToken = Arc<RwLock<bool>>;

#[derive(Debug)]
pub enum BlinkCommand {
    FindNearest(PeerId),
    Dial(PeerId),
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
    pub fn new(
        key_pair: Keypair,
        address_to_listen: &str,
        cancellation_token: CancellationToken,
    ) -> Result<Self> {
        let mut swarm = Self::create_swarm(key_pair)?;
        swarm.listen_on(address_to_listen.parse()?)?;

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
                             Self::handle_command(&mut swarm, command);
                         }
                     },
                    event = swarm.select_next_some() => {
                         if let SwarmEvent::Behaviour(_) = event {
                             Self::handle_behaviour_event(&mut swarm, event);
                         } else {
                             Self::handle_event(&mut swarm, event);
                         }
                    }
                }
            }
        });

        Ok(Self {
            command_channel: command_tx,
            task_handle: handler,
        })
    }

    fn handle_command(swarm: &mut Swarm<BlinkBehavior>, command: BlinkCommand) {
        match command {
            BlinkCommand::FindNearest(peer_id) => {
                swarm.behaviour_mut().kademlia.get_closest_peers(peer_id);
            }
            BlinkCommand::Dial(peer_id) => {
                if let Err(err) = swarm.dial(peer_id) {
                    println!("Error while dialing: {}", err);
                }
            }
        }
    }

    fn handle_behaviour_event<TErr>(
        swarm: &mut Swarm<BlinkBehavior>,
        event: SwarmEvent<BehaviourEvent, TErr>,
    ) {
        match event {
            SwarmEvent::Behaviour(BehaviourEvent::Gossipsub(gsp)) => {
                match gsp {
                    GossipsubEvent::Message { message, .. } => {
                        let message_data = message.data;
                        let data = bincode::deserialize::<Sata>(&message_data);
                        match data {
                            Ok(info) => {
                                // add info to cache
                            }
                            Err(err) => { println!("Error while deserializing {}", err);}
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

    fn handle_event<TEvent, TErr>(
        _: &mut Swarm<BlinkBehavior>,
        event: SwarmEvent<TEvent, TErr>,
    ) {
        match event {
            SwarmEvent::ConnectionEstablished { .. } => {}
            SwarmEvent::ConnectionClosed { .. } => {}
            SwarmEvent::IncomingConnection { .. } => {}
            SwarmEvent::IncomingConnectionError { .. } => {}
            SwarmEvent::OutgoingConnectionError { .. } => {}
            SwarmEvent::BannedPeer { .. } => {}
            SwarmEvent::NewListenAddr { address, .. } => {
                println!("Listening on {}", address);
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

    pub async fn pair(&mut self, peer_id: PeerId) -> Result<()> {
        self.command_channel.send(BlinkCommand::Dial(peer_id)).await?;
        Ok(())
    }
}

#[cfg(test)]
mod when_using_peer_to_peer_service {
    use crate::peer_to_peer_service::{ILogger, PeerToPeerService};
    use libp2p::{identity, PeerId};
    use std::sync::Arc;
    use std::time::Duration;
    use tokio::sync::RwLock;

    #[tokio::test]
    async fn open_does_not_throw() {
        let id_keys = identity::Keypair::generate_ed25519();
        let cancellation_token = Arc::new(RwLock::new(false));
        PeerToPeerService::new(id_keys, "/ip4/0.0.0.0/tcp/0", cancellation_token.clone()).unwrap();
    }

    #[tokio::test]
    async fn connecting_to_a_sequence_of_peers_generates_specific_events() {
        let id_keys = identity::Keypair::generate_ed25519();
        let cancellation_token = Arc::new(RwLock::new(false));
        let mut service =
            PeerToPeerService::new(id_keys, "/ip4/0.0.0.0/tcp/0", cancellation_token.clone())
                .unwrap();

        let second_client = identity::Keypair::generate_ed25519();
        service.pair(PeerId::from(second_client.public())).await.unwrap();
        tokio::time::sleep(Duration::from_secs(2)).await;
    }
}
