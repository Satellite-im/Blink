use anyhow::{anyhow, Result};
use libp2p::gossipsub::{
    Gossipsub, GossipsubConfigBuilder, MessageAuthenticity, MessageId, ValidationMode,
};
use libp2p::{identity::Keypair, Multiaddr, PeerId, Swarm};
use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
};

// Maybe having the interface in the core will help us with IoC and then with unit tests using PeerToPeerContract
// instead of going directly to the implementation.
pub trait PeerToPeerContract {
    fn listen(&mut self) -> Result<()>;
}

pub struct PeerToPeerService {
    // we will need a local key in order to generate an identity
    local_key: Keypair,
    // peer id is the our identification in a p2p network
    peer_id: PeerId,
    swarm: Option<Swarm<Gossipsub>>,
}

impl PeerToPeerService {
    pub fn new() -> Self {
        let local_key = Keypair::generate_ed25519();
        let peer_id = PeerId::from(&local_key.public());

        Self {
            local_key,
            peer_id,
            swarm: None,
        }
    }

    pub async fn setup(&mut self) -> Result<()> {
        let local_key = Keypair::generate_ed25519();
        let peer_id = PeerId::from(&local_key.public());

        let config = GossipsubConfigBuilder::default()
            .heartbeat_interval(std::time::Duration::from_secs(10)) // This is set to aid debugging by not cluttering the log space
            .validation_mode(ValidationMode::Strict) // This sets the kind of message validation. The default is Strict (enforce message signing)
            .message_id_fn(|message| {
                let mut s = DefaultHasher::new();
                message.data.hash(&mut s);
                MessageId::from(s.finish().to_string())
            }) // content-address messages. No two messages of the
            // same content will be propagated.
            .build().map_err(|err| anyhow!(err))?;

        let gossip = Gossipsub::new(
            MessageAuthenticity::Signed(local_key.clone()),
            config,
        ).map_err(|err| anyhow!(err))?;

        // Set up an encrypted TCP Transport over the Mplex and Yamux protocols
        let transport = libp2p::development_transport(local_key).await?;

        // build a gossipsub network behaviour
        let swarm = Swarm::new(transport, gossip, peer_id.clone());

        self.swarm = Some(swarm);

        Ok(())
    }

    pub async fn send(&mut self, address: &str) -> Result<()> {
        if let Some(swm) = &mut self.swarm {
            let addr : Multiaddr = address.parse()?;
            swm.dial(addr)?;
        }

        Err(anyhow!("You need to setup the service first"))
    }
}

impl<'a> PeerToPeerContract for PeerToPeerService {
    fn listen(&mut self) -> Result<()> {
        if let Some(swm) = &mut self.swarm {
            swm.listen_on("/ip4/0.0.0.0/tcp/0".parse().unwrap())?;
            return Ok(());
        }

        Err(anyhow!("You need to setup the service first"))
    }
}

#[cfg(test)]
mod when_using_peer_to_peer_service {
    use libp2p::futures::StreamExt;
    use libp2p::swarm::SwarmEvent;
    use crate::peer_to_peer_service::{PeerToPeerContract, PeerToPeerService};

    #[tokio::test]
    async fn listen_does_not_throw() {
        let mut service = PeerToPeerService::new();
        service.setup().await.unwrap();
        service.listen().unwrap();
    }

    #[tokio::test]
    async fn sending_message_to_listening_service_works() {
        let mut service = PeerToPeerService::new();
        service.setup().await;
        service.listen().unwrap();

        let temp = service.swarm.unwrap().select_next_some().await;

        if let SwarmEvent::NewListenAddr { listener_id,  address  } = temp {
            let mut some_client  = PeerToPeerService::new();
            some_client.setup().await.unwrap();
            some_client.listen().unwrap();

            some_client.send(address.to_string().as_str()).await.unwrap();
        } else {
            panic!("Fail")
        }
    }
}
