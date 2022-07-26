mod behavior;

use std::collections::hash_map::DefaultHasher;
use crate::peer_to_peer_service::behavior::BlinkBehavior;
use anyhow::Result;
use libp2p::{
    core::transport::upgrade, identity::Keypair, mplex, noise, tcp::GenTcpConfig,
    tcp::TokioTcpTransport,
    PeerId, Swarm,
    Transport
};
use libp2p::core::transport::ListenerId;
use libp2p::gossipsub::Topic;
use libp2p::swarm::SwarmBuilder;

// Maybe having the interface in the core will help us with IoC and then with unit tests using PeerToPeerContract
// instead of going directly to the implementation.
pub trait PeerToPeerContract {
    fn listen(&mut self) -> Result<()>;
}

pub struct PeerToPeerService {
    swarm: Swarm<BlinkBehavior>,
    listener_id: Option<ListenerId>,
}

impl PeerToPeerService {
    pub fn new(key_pair: Keypair) -> Result<Self> {
        let peer_id = PeerId::from(&key_pair.public());
        let blink_behaviour = BlinkBehavior::new(&key_pair)?;

        // Create a keypair for authenticated encryption of the transport.
        let noise_keys = noise::Keypair::<noise::X25519Spec>::new()
            .into_authentic(&key_pair)?;

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
        })).build();

        Ok(Self { swarm, listener_id: None })
    }

    // fn subscribe(&mut self, topic: Topic<DefaultHasher>) {
    //     let temp = self.swarm.behaviour_mut().;
    // }
}

impl PeerToPeerContract for PeerToPeerService {
    fn listen(&mut self) -> Result<()> {
        self.listener_id = Some(self.swarm.listen_on("/ip4/0.0.0.0/tcp/0".parse()?)?);
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
    use libp2p::identity;
    use crate::peer_to_peer_service::{PeerToPeerContract, PeerToPeerService};

    #[tokio::test]
    async fn listen_does_not_throw() {
        let id_keys = identity::Keypair::generate_ed25519();
        let mut service = PeerToPeerService::new(id_keys).unwrap();
        service.listen().unwrap();
    }

    #[tokio::test]
    async fn sending_message_to_listening_service_works() {
        // let mut service = PeerToPeerService::new();
        // service.setup().await;
        // service.listen().unwrap();
        //
        // let temp = service.swarm.as_mut().unwrap().select_next_some().await;
        //
        // if let SwarmEvent::NewListenAddr {
        //     listener_id,
        //     address,
        // } = temp
        // {
        //     let mut some_client = PeerToPeerService::new();
        //     some_client.setup().await.unwrap();
        //     some_client.listen().unwrap();
        //
        //     some_client
        //         .send(address.to_string().as_str())
        //         .await
        //         .unwrap();
        //
        //     let next = service.swarm.as_mut().unwrap().select_next_some().await;
        //
        //     dbg!(&next);
        // } else {
        //     panic!("Fail")
        // }
    }
}
