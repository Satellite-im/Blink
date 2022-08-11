use anyhow::Result;
use async_trait::async_trait;
use libp2p::Multiaddr;
use sata::Sata;
use warp::crypto::DID;

pub enum StreamKind {}

#[derive(Debug)]
pub enum Event {
    DialSuccessful(String),
    DialError(String),
    ConvertKeyError,
    SubscriptionError(String),
    NewListenAddr(Multiaddr),
    ErrorAddingToCache(String),
    ErrorDeserializingData,
    ErrorSerializingData,
    ErrorPublishingData(String),
    GeneratedTopic(DID, String),
    SubscribedToTopic(String),
    FailureToIdentifyPeer,
    PeerIdentified,
    FailedToSendMessage,
    FailureToDisconnectPeer,
    PeerConnectionClosed(String),
    ConnectionEstablished(String),
    TaskCancelled,
    CouldntFindTopicForDid
}

#[async_trait]
pub trait PairToAnotherPeerBlinkBehaviour {
    // Handshakes to another peer and verifies identity
    async fn pair(peers: Vec<DID>) -> Result<()>;
}

pub trait EventBus: Send + Sync {
    fn event_occurred(&mut self, event: Event);
}

#[async_trait]
pub trait SendBlinkBehaviour {
    async fn send(data: Sata) -> Result<()>;
}

#[async_trait]
pub trait Blink {
    // Starts listening for data from the remote peer(s)
    async fn open(peers: Vec<DID>) -> Result<()>;
    // Caches data to pocket dimension
    fn cache(data: Sata) -> Result<()>;
    // Allows developers to listen in on communications and hook data they care about
    fn hook(event: Event);
    // Send data directly to another peer(s)
    fn send(data: Sata) -> Result<()>;
    // Stream data to another peer(s)
    // fn stream(peers: Vec<DIDKey>, kind: StreamKind, stream: Box<dyn Stream>) -> Result<()>;
    // // aliases
    // fn call(peers: Vec<DIDKey>, stream: Stream) -> Result<()>;
    // fn video(peers: Vec<DIDKey>, stream: Stream) -> Result<()>; // calls stream()
    // fn screen_share(peers: Vec<DIDKey>, stream: Stream) -> Result<()>; // calls stream()
}
