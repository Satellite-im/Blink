use anyhow::anyhow;
use blink_contract::{Event, EventBus};
use log::info;
use sata::Sata;
use std::fs::File;
use std::io::Write;
use std::time::{SystemTime, UNIX_EPOCH};
use warp::{
    crypto::DID,
    data::DataType,
    error::Error,
    module::Module,
    multipass::{
        identity::{Identifier, Identity, IdentityUpdate},
        Friends, MultiPass,
    },
    pocket_dimension::{query::QueryBuilder, PocketDimension},
    Extension, SingleHandle,
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
                info!("Event: Successfully dialed {}", x);
            }
            Event::DialError(x) => {
                info!("Event: Error dialing {}", x);
            }
            Event::ConvertKeyError => {
                info!("Event: Converting key error");
            }
            Event::SubscriptionError(x) => {
                info!("Event: Subscription error {}", x);
            }
            Event::NewListenAddr(x) => {
                info!("Event: NewListenAddr {}", x.to_string());
            }
            Event::ErrorAddingToCache(x) => {
                info!("Event: Error adding to cache {}", x);
            }
            Event::ErrorDeserializingData => {
                info!("Event: Error deserializing data");
            }
            Event::ErrorSerializingData => {
                info!("Event: Error serializing data");
            }
            Event::ErrorPublishingData(x) => {
                info!("Event: Error publishing data {}", x);
            }
            Event::SubscribedToTopic(x) => {
                info!("Event: Subscribed to topic {}", x);
            }
            Event::FailureToIdentifyPeer => {
                info!("Event: Failure to identify peer");
            }
            Event::PeerIdentified => {
                info!("Event: Peer identified");
            }
            Event::FailedToSendMessage => {
                info!("Event: Failure to send message");
            }
            Event::FailureToDisconnectPeer => {
                info!("Event: Failure to disconnect from peer");
            }
            Event::PeerConnectionClosed(x) => {
                info!("Event: Peer connection closed {}", x);
            }
            Event::ConnectionEstablished(x) => {
                info!("Event: Connection established {}", x);
            }
            Event::TaskCancelled => {
                info!("Event: Task cancelled");
            }
            Event::CouldntFindTopicForDid => {
                info!("Event: Couldn't find topic for did")
            }
            Event::GeneratedTopic(_, _) => {
                info!("Event: Generated topic")
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
    fn add_data(&mut self, _: DataType, data: &Sata) -> Result<(), Error> {
        let mut path = std::env::temp_dir();
        let time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        path.push(format!("{}.txt", time));

        let path_to_write = path.to_str().unwrap().to_string();
        let res = std::str::from_utf8(&data.data())
            .map_err(|x| anyhow!(x))?
            .to_string();
        let mut output = File::create(path)?;
        write!(output, "{}", res)?;
        info!("file wrote {}", path_to_write);
        Ok(())
    }

    fn has_data(&mut self, _: DataType, _: &QueryBuilder) -> Result<(), Error> {
        todo!()
    }

    fn get_data(&self, _: DataType, _: Option<&QueryBuilder>) -> Result<Vec<Sata>, Error> {
        todo!()
    }

    fn size(&self, _: DataType, _: Option<&QueryBuilder>) -> Result<i64, Error> {
        todo!()
    }

    fn count(&self, _: DataType, _: Option<&QueryBuilder>) -> Result<i64, Error> {
        todo!()
    }

    fn empty(&mut self, _: DataType) -> Result<(), Error> {
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
    fn create_identity(&mut self, _: Option<&str>, _: Option<&str>) -> Result<DID, Error> {
        todo!()
    }

    fn get_identity(&self, _: Identifier) -> Result<Identity, Error> {
        return Ok(Identity::default());
    }

    fn update_identity(&mut self, _: IdentityUpdate) -> Result<(), Error> {
        todo!()
    }

    fn decrypt_private_key(&self, _: Option<&str>) -> Result<DID, Error> {
        todo!()
    }

    fn refresh_cache(&mut self) -> Result<(), Error> {
        todo!()
    }
}
