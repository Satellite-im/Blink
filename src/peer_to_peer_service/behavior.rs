use anyhow::{anyhow, Result};
use libp2p::{
    gossipsub::{
        Gossipsub, GossipsubConfigBuilder, GossipsubEvent, MessageAuthenticity, MessageId,
        ValidationMode,
    },
    identity::Keypair,
    kad::store::MemoryStore,
    kad::{Kademlia, KademliaConfig, KademliaEvent},
    relay::v2::{
        relay,
        relay::{Event, Relay},
    },
    swarm::NetworkBehaviour,
    NetworkBehaviour, PeerId,
};
use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
    time::Duration,
};
use libp2p::identify::{Identify, IdentifyConfig, IdentifyEvent};
use libp2p::kad::QueryId;

const IDENTIFY_PROTOCOL_VERSION: &str = "/ipfs/0.1.0";

#[derive(NetworkBehaviour)]
#[behaviour(event_process = false, out_event = "BehaviourEvent")]
pub(crate) struct BlinkBehavior {
    gossip_sub: Gossipsub,
    kademlia: Kademlia<MemoryStore>,
    identity: Identify,
    relay: Relay,
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
            .build()
            .map_err(|x| anyhow!(x))?;

        let relay = Relay::new(peer_id, Default::default());
        // Create a Kademlia behaviour.
        let mut kademlia_cfg = KademliaConfig::default();
        kademlia_cfg.set_query_timeout(Duration::from_secs(5 * 60));
        let store = MemoryStore::new(peer_id.clone());
        let kademlia = Kademlia::with_config(peer_id.clone(), store, kademlia_cfg);
        let gossip_sub: Gossipsub =
            Gossipsub::new(MessageAuthenticity::Signed(key_pair.clone()), gossip_config)
                .map_err(|x| anyhow!(x))?;

        let identity = Identify::new(IdentifyConfig::new(
            IDENTIFY_PROTOCOL_VERSION.into(),
            key_pair.public(),
        ));

        Ok(Self {
            gossip_sub,
            kademlia,
            relay,
            identity,
        })
    }

    pub(crate) fn find_peer(&mut self, peer_id: PeerId) -> QueryId {
        self.kademlia.get_closest_peers(peer_id)
    }
}

#[derive(Debug)]
pub(crate) enum BehaviourEvent {
    Gossipsub(GossipsubEvent),
    RelayEvent(Event),
    KademliaEvent(KademliaEvent),
    IdentifyEvent(IdentifyEvent)
}

impl From<IdentifyEvent> for BehaviourEvent {
    fn from(event: IdentifyEvent) -> Self {
        BehaviourEvent::IdentifyEvent(event)
    }
}

impl From<KademliaEvent> for BehaviourEvent {
    fn from(event: KademliaEvent) -> Self {
        BehaviourEvent::KademliaEvent(event)
    }
}

impl From<GossipsubEvent> for BehaviourEvent {
    fn from(event: GossipsubEvent) -> Self {
        BehaviourEvent::Gossipsub(event)
    }
}

impl From<relay::Event> for BehaviourEvent {
    fn from(event: relay::Event) -> Self {
        BehaviourEvent::RelayEvent(event)
    }
}
