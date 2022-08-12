use blink_contract::{Event, EventBus};
use did_key::Ed25519KeyPair;
use libp2p::Multiaddr;
use sata::{Kind, Sata};
use std::{sync::atomic::AtomicBool, sync::Arc, time::Duration};
use libp2p::swarm::dial_opts::DialOpts;
use sata::libipld::IpldCodec;
use tokio::sync::mpsc::Receiver;
use warp::{
    crypto::DID,
    data::DataType,
    error::Error,
    module::Module,
    multipass::identity::{Identifier, Identity, IdentityUpdate},
    multipass::{Friends, MultiPass},
    pocket_dimension::query::QueryBuilder,
    pocket_dimension::PocketDimension,
    Extension, SingleHandle,
};
use warp::sync::RwLock;
use crate::peer_to_peer_service::{MessageContent, PeerToPeerService};

const TIMEOUT_SECS: u64 = 1;

#[derive(Default)]
struct TestCache {
    data_added: Vec<(DataType, Sata)>,
}

struct MultiPassImpl {
    pass_as_valid: bool,
}

impl MultiPassImpl {
    fn new(pass_as_valid: bool) -> Self {
        Self { pass_as_valid }
    }
}

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

impl Friends for MultiPassImpl {}

impl SingleHandle for MultiPassImpl {}

impl MultiPass for MultiPassImpl {
    fn create_identity(&mut self, _: Option<&str>, _: Option<&str>) -> Result<DID, Error> {
        todo!()
    }

    fn get_identity(&self, _: Identifier) -> Result<Identity, Error> {
        if self.pass_as_valid {
            return Ok(Identity::default());
        }

        Err(Error::IdentityDoesntExist)
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

impl Extension for TestCache {
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

impl SingleHandle for TestCache {}

impl PocketDimension for TestCache {
    fn add_data(&mut self, dimension: DataType, data: &Sata) -> Result<(), Error> {
        self.data_added.push((dimension, data.clone()));
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

struct LogHandler {
    pub events: Vec<Event>,
}

impl LogHandler {
    fn new() -> Self {
        Self { events: Vec::new() }
    }
}

impl EventBus for LogHandler {
    fn event_occurred(&mut self, event: Event) {
        self.events.push(event);
    }
}

async fn create_service(
    initial_address: Vec<Multiaddr>,
    pass_multi_pass_validation_requests: bool,
) -> (
    PeerToPeerService,
    Arc<RwLock<LogHandler>>,
    Arc<RwLock<TestCache>>,
    Arc<RwLock<MultiPassImpl>>,
    Arc<DID>,
    Vec<Multiaddr>,
    Receiver<MessageContent>
) {
    let id_keys = Arc::new(DID::from(did_key::generate::<Ed25519KeyPair>(
        None,
    )));
    let cancellation_token = Arc::new(AtomicBool::new(false));
    let cache = Arc::new(RwLock::new(TestCache::default()));
    let log_handler = Arc::new(RwLock::new(LogHandler::new()));
    let multi_pass = Arc::new(RwLock::new(MultiPassImpl::new(
        pass_multi_pass_validation_requests,
    )));
    let (service, receiver) = PeerToPeerService::new(
        id_keys.clone(),
        "/ip4/0.0.0.0/tcp/0",
        Some(initial_address),
        cache.clone(),
        multi_pass.clone(),
        log_handler.clone(),
        cancellation_token.clone(),
    )
        .await
        .unwrap();

    let mut addr_to_send = None;
    let mut break_loop = false;
    while !break_loop {
        for event in &log_handler.read().events {
            if let Event::NewListenAddr(addr) = event {
                break_loop = true;
                addr_to_send = Some(addr.clone());
                break;
            }
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }

    let mut map = Vec::new();
    map.push(addr_to_send.unwrap());

    (
        service,
        log_handler,
        cache,
        multi_pass,
        id_keys,
        map,
        receiver
    )
}

#[tokio::test]
async fn open_does_not_throw() {
    tokio::time::timeout(Duration::from_secs(TIMEOUT_SECS), async {
        create_service(Vec::new(), true).await;
    })
        .await
        .expect("timeout");
}

#[tokio::test]
async fn connecting_to_peer_does_not_generate_errors() {
    tokio::time::timeout(Duration::from_secs(TIMEOUT_SECS), async {
        #[allow(dead_code)]
        let service_a
            = create_service(Vec::new(), true).await;

        let mut service_b
            = create_service(service_a.5.clone(), true).await;

        pair_to_another_peer(&mut service_b.0, service_a.5.first().unwrap().clone().into(), service_b.1).await;
    })
        .await
        .expect("Timeout");
}

#[tokio::test]
async fn message_reaches_other_client() {
    tokio::time::timeout(Duration::from_secs(TIMEOUT_SECS), async {
        let mut second_client =
            create_service(Vec::new(), true).await;

        let (mut first_client, first_client_log_handler, _, _, _, _, _) =
            create_service(second_client.5.clone(), true).await;

        let (did_from_pair, _) =
            pair_to_another_peer(&mut first_client, second_client.5.first().unwrap().clone().into(), first_client_log_handler.clone()).await;

        let mut some_data = Sata::default();
        some_data.add_recipient(did_from_pair.as_ref()).unwrap();

        first_client.send(some_data).await.unwrap();

        while second_client.6.recv().await.is_none() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
        .await
        .expect("Timeout");
}

#[tokio::test]
async fn message_to_another_client_is_added_to_cache() {
    tokio::time::timeout(Duration::from_secs(TIMEOUT_SECS), async {
        let second_client = create_service(Vec::new(), true).await;

        let (mut first_client, first_client_log_handler, _, _, _, _, _) =
            create_service(second_client.5.clone(), true).await;

        let (did_from_pair, _) = pair_to_another_peer(&mut first_client, second_client.5.first().unwrap().clone().into(), first_client_log_handler.clone()).await;

        let mut some_data = Sata::default();
        some_data.add_recipient(did_from_pair.as_ref()).unwrap();

        first_client.send(some_data).await.unwrap();

        loop {
            if second_client.2.read().data_added.len() > 0 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
        .await
        .expect("Timeout");
}

#[tokio::test]
async fn failure_to_identify_peer_causes_error() {
    tokio::time::timeout(Duration::from_secs(TIMEOUT_SECS), async {
        let first_client =
            create_service(Vec::new(), false).await;

        let (mut second_client, log_handler_second_client, _, _, _, _, _) =
            create_service(first_client.5.clone(), false).await;

        second_client
            .pair_to_another_peer(first_client.5.first().unwrap().clone().into())
            .await
            .unwrap();

        let mut found_error = false;
        while !found_error {
            for event in &log_handler_second_client.read().events {
                if let Event::FailureToIdentifyPeer = event {
                    found_error = true;
                }
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
        .await
        .expect("Timeout");
}

async fn pair_to_another_peer(service: &mut PeerToPeerService, dial_opts: DialOpts, logger: Arc<RwLock<LogHandler>>) -> (DID, String){
    service.pair_to_another_peer(dial_opts).await.unwrap();

    let mut found_event = false;
    let mut result = None;
    let mut to_remove = 0;

    while !found_event {
        for (i, event) in logger.read().events.iter().enumerate() {
            if let Event::GeneratedTopic(did, topic) = event {
                result = Some((did.clone(), topic.clone()));
                found_event = true;
                to_remove = i;
                break;
            }
        }
        if !found_event {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }

    let event_number = logger.read().events.len();
    logger.write().events.swap_remove(to_remove);
    assert_eq!(event_number - 1, logger.read().events.len());

    result.unwrap()
}

async fn assert_message(receiver: &mut Receiver<MessageContent>) {
    let mut message_received = false;

    while !message_received {
        if let Some(_) = receiver.recv().await {
            message_received = true;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

#[tokio::test]
async fn send_message_sends_it_to_every_recipient() {
    tokio::time::timeout(Duration::from_secs(7), async {
        let message_content = "Test".to_string();
        let mut client_a
            = create_service(Vec::new(), true).await;
        let mut client_b
            = create_service(Vec::new(), true).await;
        client_a.5.extend(client_b.5.into_iter());

        let (mut service_c, c_log, _, _, _, _, _)
            = create_service(client_a.5.clone(), true).await;

        let (did_a, _) = pair_to_another_peer(&mut service_c, client_a.5[0].clone().into(), c_log.clone()).await;
        let (did_b, _) = pair_to_another_peer(&mut service_c, client_a.5[1].clone().into(), c_log.clone()).await;

        assert_ne!(did_a, did_b);

        let mut sata = Sata::default();
        sata.add_recipient(did_a.as_ref()).unwrap();
        sata.add_recipient(did_b.as_ref()).unwrap();

        let to_send = sata.encode(IpldCodec::DagJson, Kind::Dynamic, message_content.clone()).unwrap();
        assert_eq!(to_send.recipients().as_ref().unwrap().len(), 2);

        service_c.send(to_send).await.unwrap();

        assert_message(&mut client_a.6).await;
        assert_message(&mut client_b.6).await;
    })
        .await
        .expect("Timeout");
}