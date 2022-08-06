use blink_contract::{Event, EventBus};
use sata::Sata;
use warp::crypto::DID;
use warp::data::DataType;
use warp::error::Error;
use warp::module::Module;
use warp::multipass::identity::{Identifier, Identity, IdentityUpdate};
use warp::multipass::{Friends, MultiPass};
use warp::pocket_dimension::query::QueryBuilder;
use warp::pocket_dimension::PocketDimension;
use warp::{Extension, SingleHandle};

#[derive(Default)]
pub struct MultiPassImpl {}

#[derive(Default)]
pub struct PocketDimensionImpl {}

#[derive(Default)]
pub struct EventHandlerImpl {}

impl EventBus for EventHandlerImpl {
    fn event_occurred(&mut self, event: Event) {
        match event {
            Event::DialSuccessful(_) => {}
            Event::DialError(_) => {}
            Event::ConvertKeyError => {}
            Event::SubscriptionError(_) => {}
            Event::NewListenAddr(x) => {
                println!("NewListenAddr {}", x.to_string());
            }
            Event::ErrorAddingToCache(_) => {}
            Event::ErrorDeserializingData => {}
            Event::ErrorSerializingData => {}
            Event::ErrorPublishingData(_) => {}
            Event::SubscribedToTopic(_) => {}
            Event::FailureToIdentifyPeer => {}
            Event::PeerIdentified => {}
            Event::FailedToSendMessage => {}
            Event::FailureToDisconnectPeer => {}
            Event::PeerConnectionClosed(_) => {}
            Event::ConnectionEstablished(_) => {}
            Event::TaskCancelled => {}
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
        todo!()
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
