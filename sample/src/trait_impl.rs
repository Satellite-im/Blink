use blink_contract::{Event, EventBus};
use sata::Sata;
use warp::{
    error::Error,
    data::DataType,
    crypto::DID,
    module::Module,
    multipass::{
        identity::{Identifier, Identity, IdentityUpdate},
        Friends,
        MultiPass
    },
    pocket_dimension::{
        query::QueryBuilder,
        PocketDimension
    },
    Extension,
    SingleHandle
};

#[derive(Default)]
pub struct MultiPassImpl {}

#[derive(Default)]
pub struct PocketDimensionImpl {}

#[derive(Default)]
pub struct EventHandlerImpl {}

impl EventBus for EventHandlerImpl {
    fn event_occurred(&mut self, event: Event) {
        match event {
            Event::DialSuccessful(x) => {
                println!("Event: Successfully dialed {}", x);
            }
            Event::DialError(x) => {
                println!("Event: Error dialing {}", x);
            }
            Event::ConvertKeyError => {
                println!("Event: Converting key error");
            }
            Event::SubscriptionError(x) => {
                println!("Event: Subscription error {}", x);
            }
            Event::NewListenAddr(x) => {
                println!("Event: NewListenAddr {}", x.to_string());
            }
            Event::ErrorAddingToCache(x) => {
                println!("Event: Error adding to cache {}", x);
            }
            Event::ErrorDeserializingData => {
                println!("Event: Error deserializing data");
            }
            Event::ErrorSerializingData => {
                println!("Event: Error serializing data");
            }
            Event::ErrorPublishingData(x) => {
                println!("Event: Error publishing data {}", x);
            }
            Event::SubscribedToTopic(x) => {
                println!("Event: Subscribed to topic {}", x);
            }
            Event::FailureToIdentifyPeer => {
                println!("Event: Failure to identify peer");
            }
            Event::PeerIdentified => {
                println!("Event: Peer identified");
            }
            Event::FailedToSendMessage => {
                println!("Event: Failure to send message");
            }
            Event::FailureToDisconnectPeer => {
                println!("Event: Failure to disconnect from peer");
            }
            Event::PeerConnectionClosed(x) => {
                println!("Event: Peer connection closed {}", x);
            }
            Event::ConnectionEstablished(x) => {
                println!("Event: Connection established {}", x);
            }
            Event::TaskCancelled => {
                println!("Event: Task cancelled");
            }
        }
    }
}

impl Extension for PocketDimensionImpl {
    fn id(&self) -> String {
        todo!()
    }

    fn name(&self) -> String {
        todo!()
    }

    fn module(&self) -> Module {
        todo!()
    }
}

impl SingleHandle for PocketDimensionImpl {}

impl PocketDimension for PocketDimensionImpl {
    fn add_data(&mut self, dimension: DataType, data: &Sata) -> Result<(), Error> {
        todo!()
    }

    fn has_data(&mut self, dimension: DataType, query: &QueryBuilder) -> Result<(), Error> {
        todo!()
    }

    fn get_data(
        &self,
        dimension: DataType,
        query: Option<&QueryBuilder>,
    ) -> Result<Vec<Sata>, Error> {
        todo!()
    }

    fn size(&self, dimension: DataType, query: Option<&QueryBuilder>) -> Result<i64, Error> {
        todo!()
    }

    fn count(&self, dimension: DataType, query: Option<&QueryBuilder>) -> Result<i64, Error> {
        todo!()
    }

    fn empty(&mut self, dimension: DataType) -> Result<(), Error> {
        todo!()
    }
}

impl Friends for MultiPassImpl {}

impl SingleHandle for MultiPassImpl {}

impl Extension for MultiPassImpl {
    fn id(&self) -> String {
        todo!()
    }

    fn name(&self) -> String {
        todo!()
    }

    fn module(&self) -> Module {
        todo!()
    }
}

impl MultiPass for MultiPassImpl {
    fn create_identity(
        &mut self,
        username: Option<&str>,
        passphrase: Option<&str>,
    ) -> Result<DID, Error> {
        todo!()
    }

    fn get_identity(&self, id: Identifier) -> Result<Identity, Error> {
        return Ok(Identity::default());
    }

    fn update_identity(&mut self, option: IdentityUpdate) -> Result<(), Error> {
        todo!()
    }

    fn decrypt_private_key(&self, passphrase: Option<&str>) -> Result<DID, Error> {
        todo!()
    }

    fn refresh_cache(&mut self) -> Result<(), Error> {
        todo!()
    }
}
