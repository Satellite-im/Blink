use anyhow::{anyhow, Result};
use libp2p::{
    gossipsub::GossipsubEvent,
    identify::{Identify, IdentifyConfig, IdentifyEvent},
    identity::Keypair,
    kad::{store::MemoryStore, Kademlia, KademliaConfig, KademliaEvent},
    mdns::{Mdns, MdnsEvent},
    relay::v2::relay::{Event, Relay},
    NetworkBehaviour, PeerId,
};
use libp2p_helper::gossipsub::GossipsubStream;
use std::time::Duration;

const IDENTIFY_PROTOCOL_VERSION: &str = "/ipfs/0.1.0";

#[derive(NetworkBehaviour)]
#[behaviour(event_process = false, out_event = "BehaviourEvent")]
pub(crate) struct BlinkBehavior {
    pub(crate) gossip_sub: GossipsubStream,
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
        let gossip_sub = GossipsubStream::new(key_pair.clone()).map_err(|err| anyhow!(err))?;

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
