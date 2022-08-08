use anyhow::{anyhow, Result};
use libp2p::gossipsub::{
    Gossipsub, GossipsubMessage, MessageAuthenticity, MessageId, ValidationMode,
};
use libp2p::{
    gossipsub,
    gossipsub::GossipsubEvent,
    identify::{Identify, IdentifyConfig, IdentifyEvent},
    identity::Keypair,
    kad::{store::MemoryStore, Kademlia, KademliaConfig, KademliaEvent},
    mdns::{Mdns, MdnsEvent},
    relay::v2::relay::{Event, Relay},
    NetworkBehaviour, PeerId,
};
use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
    time::Duration,
};

const IDENTIFY_PROTOCOL_VERSION: &str = "/ipfs/0.1.0";

#[derive(NetworkBehaviour)]
#[behaviour(event_process = false, out_event = "BehaviourEvent")]
pub(crate) struct BlinkBehavior {
    pub(crate) gossip_sub: Gossipsub,
    pub(crate) kademlia: Kademlia<MemoryStore>,
    pub(crate) identity: Identify,
    pub(crate) relay: Relay,
    pub(crate) mdns: Mdns,
}

impl BlinkBehavior {
    pub(crate) async fn new(key_pair: &Keypair) -> Result<Self> {
        let peer_id = PeerId::from(&key_pair.public());
        let mdns = Mdns::new(Default::default()).await?;

        let relay = Relay::new(peer_id, Default::default());
        // Create a Kademlia behaviour.
        let mut kademlia_cfg = KademliaConfig::default();
        kademlia_cfg.set_query_timeout(Duration::from_secs(5 * 60));
        let store = MemoryStore::new(peer_id.clone());
        let kademlia = Kademlia::with_config(peer_id.clone(), store, kademlia_cfg);
        // let config = gossipsub::GossipsubConfigBuilder::default()
        //     .build()
        //     .map_err(|e| anyhow::anyhow!(e))?;

        let config = gossipsub::GossipsubConfigBuilder::default()
            .heartbeat_interval(Duration::from_secs(10)) // This is set to aid debugging by not cluttering the log space
            .validation_mode(ValidationMode::Strict) // This sets the kind of message validation. The default is Strict (enforce message signing)
            .message_id_fn(|message: &GossipsubMessage| {
                let mut s = DefaultHasher::new();
                message.data.hash(&mut s);
                MessageId::from(s.finish().to_string())
            }) // content-address messages. No two messages of the
            // same content will be propagated.
            .build()
            .expect("Valid config");
        // build a gossipsub network behaviour

        let gossip_sub = Gossipsub::new(MessageAuthenticity::Signed(key_pair.clone()), config)
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
            mdns,
        })
    }
}

#[derive(Debug)]
pub(crate) enum BehaviourEvent {
    Gossipsub(GossipsubEvent),
    RelayEvent(Event),
    KademliaEvent(KademliaEvent),
    IdentifyEvent(IdentifyEvent),
    MdnsEvent(MdnsEvent),
}

impl From<MdnsEvent> for BehaviourEvent {
    fn from(event: MdnsEvent) -> Self {
        BehaviourEvent::MdnsEvent(event)
    }
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

impl From<Event> for BehaviourEvent {
    fn from(event: Event) -> Self {
        BehaviourEvent::RelayEvent(event)
    }
}
