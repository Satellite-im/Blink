use anyhow::Result;
use libp2p::gossipsub::{
    Gossipsub, GossipsubConfigBuilder, MessageAuthenticity, MessageId, ValidationMode,
};
use libp2p::{identity::Keypair, PeerId, Swarm};
use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
};

// Maybe having the interface in the core will help us with IoC and then with unit tests using PeerToPeerContract
// instead of going directly to the implementation.
pub trait PeerToPeerContract {
    fn listen(&mut self) -> Result<()>;
}

pub struct PeerToPeerService<'a> {
    // we will need a local key in order to generate an identity
    local_key: Keypair,
    // peer id is the our identification in a p2p network
    peer_id: PeerId,
    gossip: &'a Gossipsub,
    swarm: Swarm<Gossipsub>,
}

impl<'a> PeerToPeerService<'a> {
    pub fn new() -> Result<Self> {
        // let local_key = Keypair::generate_ed25519();
        // let peer_id = PeerId::from(&local_key);
        //
        // let gossip = Gossipsub::new(
        //     MessageAuthenticity::Signed(local_key.clone()),
        //     GossipsubConfigBuilder::default()
        //         .heartbeat_interval(std::time::Duration::from_secs(10)) // This is set to aid debugging by not cluttering the log space
        //         .validation_mode(ValidationMode::Strict) // This sets the kind of message validation. The default is Strict (enforce message signing)
        //         .message_id_fn(|message| {
        //             let mut s = DefaultHasher::new();
        //             message.data.hash(&mut s);
        //             MessageId::from(s.finish().to_string())
        //         }) // content-address messages. No two messages of the
        //         // same content will be propagated.
        //         .build()?,
        // )?;
        //
        // let gossip_ref = &gossip;
        // // Set up an encrypted TCP Transport over the Mplex and Yamux protocols
        // let transport = libp2p::development_transport(local_key).await?;
        //
        // // build a gossipsub network behaviour
        // let swarm = Swarm::new(transport, gossip, peer_id.clone());
        //
        // Ok(PeerToPeerService {
        //     local_key: local_key.clone(),
        //     peer_id,
        //     gossip: gossip_ref,
        //     swarm,
        // })

        todo!()
    }
}

impl<'a> PeerToPeerContract for PeerToPeerService<'a> {
    fn listen(&mut self) -> Result<()> {
        todo!()
    }
}

#[cfg(test)]
mod when_using_peer_to_peer_service {
    use crate::peer_to_peer_service::{PeerToPeerContract, PeerToPeerService};

    #[test]
    fn listen_must_work() {
        let mut service = PeerToPeerService::new().unwrap();
        service.listen().unwrap();
    }
}
