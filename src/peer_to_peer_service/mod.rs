mod behavior;

use crate::peer_to_peer_service::behavior::BlinkBehavior;
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
use libp2p::kad::QueryId;

pub struct PeerToPeerService {
    swarm: Swarm<BlinkBehavior>,
}

impl PeerToPeerService {
    pub fn new(key_pair: Keypair) -> Result<Self> {
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

        Ok(Self { swarm })
    }

    // fn subscribe(&mut self, topic: Topic<DefaultHasher>) {
    //     let temp = self.swarm.behaviour_mut().;
    // }

    pub async fn pair(&mut self, peers_to_connect: &[PublicKey]) {
        for key in peers_to_connect {
            let id = PeerId::from(key);
            self.swarm.behaviour_mut().find_peers(id);
        }
    }

    pub async fn listen(&mut self, address: &str) -> Result<()> {
        self.swarm.listen_on(address.parse()?)?;

        let mut tick = tokio::time::interval(Duration::from_secs(2));
        loop {
            tokio::select! {
                event = self.swarm.select_next_some() => {
                    if let SwarmEvent::NewListenAddr { address, .. } = event {
                        println!("Listening on {:?}", address);
                    }
                },
                _ = tick.tick() => {
                    break
                }
            }
        }

        // connect to relay?

        Ok(())
    }
}

// impl PeerToPeerContract for PeerToPeerService {
//     fn listen(&mut self) -> Result<()> {
//         self.swarm.listen("/ip4/0.0.0.0/tcp/0".parse().unwrap())?
//     }
// }

#[cfg(test)]
mod when_using_peer_to_peer_service {
    use crate::peer_to_peer_service::PeerToPeerService;
    use libp2p::{identity, PeerId};

    #[tokio::test]
    async fn listen_does_not_throw() {
        let id_keys = identity::Keypair::generate_ed25519();
        let mut service = PeerToPeerService::new(id_keys).unwrap();
        service.listen("/ip4/0.0.0.0/tcp/0").await.unwrap();
    }

    #[tokio::test]
    async fn connecting_to_a_sequence_of_peers_generates_specific_events() {
        let id_keys = identity::Keypair::generate_ed25519();
        let mut service = PeerToPeerService::new(id_keys).unwrap();
        service.listen("/ip4/0.0.0.0/tcp/0").await.unwrap();

        let second_client = identity::Keypair::generate_ed25519();
        service.pair(&vec![second_client.public()]).await;
    }
}
