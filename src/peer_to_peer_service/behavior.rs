use anyhow::{anyhow, Result};
use libp2p::{
    gossipsub::{
        Gossipsub, GossipsubConfigBuilder, GossipsubEvent, MessageAuthenticity, MessageId,
        ValidationMode,
    },
    identity::Keypair,
    kad::store::MemoryStore,
    kad::{Kademlia, KademliaConfig, KademliaEvent},
    swarm::NetworkBehaviourEventProcess,
    NetworkBehaviour, PeerId,
};
use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
    time::Duration,
};

#[derive(NetworkBehaviour)]
#[behaviour(event_process = true)]
pub(crate) struct BlinkBehavior {
    gossip_sub: Gossipsub,
    kademlia: Kademlia<MemoryStore>
}

impl BlinkBehavior {
    pub(crate) fn new(key_pair: &Keypair) -> Result<Self> {
        let peer_id = PeerId::from(&key_pair.public());
        let gossip_config = GossipsubConfigBuilder::default()
            .heartbeat_interval(Duration::from_secs(10)) // This is set to aid debugging by not cluttering the log space
            .validation_mode(ValidationMode::Strict) // This sets the kind of message validation. The default is Strict (enforce message signing)
            .message_id_fn(|message| {
                // To content-address message, we can take the hash of message and use it as an ID.
                let mut s = DefaultHasher::new();
                message.data.hash(&mut s);
                MessageId::from(s.finish().to_string())
            }) // content-address messages. No two messages of the
            // same content will be propagated.
            .build().map_err(|x| anyhow!(x))?;

        // Create a Kademlia behaviour.
        let mut kademlia_cfg = KademliaConfig::default();
        kademlia_cfg.set_query_timeout(Duration::from_secs(5 * 60));
        let store = MemoryStore::new(peer_id.clone());
        let kademlia = Kademlia::with_config(peer_id.clone(), store, kademlia_cfg);
        let gossip_sub =
            Gossipsub::new(MessageAuthenticity::Signed(key_pair.clone()), gossip_config).map_err(|x| anyhow!(x))?;
        Ok(Self {
            gossip_sub,
            kademlia
        })
    }

    // pub(crate) fn subscribe_to_topic(&mut self, topic: &Topic<DefaultHasher>) -> Result<bool> {
    //     self.gossip_sub.subscribe(topic).map_err(|err| anyhow::Error::from(err))
    // }
}

impl NetworkBehaviourEventProcess<KademliaEvent> for BlinkBehavior {
    fn inject_event(&mut self, _: KademliaEvent) {
        todo!()
    }
}

impl NetworkBehaviourEventProcess<GossipsubEvent> for BlinkBehavior {
    fn inject_event(&mut self, event: GossipsubEvent) {
        if let GossipsubEvent::Message {
            message,
            message_id: _,
            propagation_source: _,
        } = event
        {
            println!("{}", String::from_utf8_lossy(&message.data));
        }
    }
}
