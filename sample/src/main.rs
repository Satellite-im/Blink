use crate::{
    did_key::Ed25519KeyPair,
    trait_impl::{EventHandlerImpl, MultiPassImpl, PocketDimensionImpl},
};
use blink_impl::peer_to_peer_service::{MessageContent, PeerToPeerService};
use libp2p::Multiaddr;
use log::{error, info};
use sata::{libipld::IpldCodec, Kind, Sata};
use std::{
    collections::HashMap, future::Future, io::stdin, pin::Pin, sync::atomic::AtomicBool, sync::Arc,
};
use tokio::{sync::mpsc::Receiver, task::JoinHandle};
use warp::crypto::{did_key, DID};
use warp::sync::RwLock;

mod trait_impl;

fn handle_coming_messages(mut receiver: Receiver<MessageContent>) -> JoinHandle<()> {
    let handle = tokio::spawn(async move {
        loop {
            let message = receiver.recv().await;

            if let Some(message_content) = message {
                let res = std::str::from_utf8(&message_content.1.data())
                    .unwrap()
                    .to_string();
                info!(
                    "Message arrived, topic hash: {}, message content: {}",
                    message_content.0.to_string(),
                    res
                );
            }
        }
    });

    handle
}

async fn create_service() -> (PeerToPeerService, Receiver<MessageContent>) {
    let id_keys = Arc::new(DID::from(did_key::generate::<Ed25519KeyPair>(None)));

    info!("DID Key: {}", (*id_keys).to_string());

    let cancellation_token = Arc::new(AtomicBool::new(false));
    let cache = Arc::new(RwLock::new(PocketDimensionImpl::default()));
    let log_handler = Arc::new(RwLock::new(EventHandlerImpl::default()));
    let multi_pass = Arc::new(RwLock::new(MultiPassImpl::default()));

    let result = PeerToPeerService::new(
        id_keys.clone(),
        "/ip4/0.0.0.0/tcp/0",
        None,
        cache.clone(),
        multi_pass.clone(),
        log_handler.clone(),
        cancellation_token.clone(),
    )
    .await
    .unwrap();

    result
}

fn create_command_map_handler() -> HashMap<
    String,
    Box<
        dyn FnMut(Arc<RwLock<PeerToPeerService>>, Vec<String>) -> Pin<Box<dyn Future<Output = ()>>>,
    >,
> {
    let mut map_command: HashMap<
        String,
        Box<
            dyn FnMut(
                Arc<RwLock<PeerToPeerService>>,
                Vec<String>,
            ) -> Pin<Box<dyn Future<Output = ()>>>,
        >,
    > = HashMap::new();

    map_command.insert(
        "pair".to_string(),
        Box::new(
            |service: Arc<RwLock<PeerToPeerService>>, args: Vec<String>| {
                Box::pin(async move {
                    if args.len() == 1 {
                        let addr = args[0].parse::<Multiaddr>();
                        match addr {
                            Ok(x) => match service.write().pair_to_another_peer(x.into()).await {
                                Ok(_) => {
                                    info!("Success sending pairing request");
                                }
                                Err(_) => {
                                    error!("Failure sending pairing request");
                                }
                            },
                            Err(_) => {
                                error!("Failed to parse address");
                            }
                        }
                    } else {
                        error!("pair peer_address");
                    }
                })
            },
        ),
    );

    map_command.insert(
        "send".to_string(),
        Box::new(
            |service: Arc<RwLock<PeerToPeerService>>, args: Vec<String>| {
                Box::pin(async move {
                    if args.len() >= 2 {
                        let mut sata = Sata::default();
                        for item in &args[..args.len() - 1] {
                            match DID::try_from(item.clone()) {
                                Ok(did_key) => {
                                    if let Err(e) = sata.add_recipient(&did_key.as_ref()) {
                                        error!("{}", anyhow::anyhow!(e).to_string());
                                    }
                                }
                                Err(e) => {
                                    error!("{}", e.enum_to_string());
                                }
                            }
                        }

                        match sata.encode(IpldCodec::DagJson, Kind::Dynamic, args.last().unwrap()) {
                            Ok(o) => {
                                if let Err(x) = service.write().send(o).await {
                                    error!("{}", anyhow::anyhow!(x).to_string());
                                }
                            }
                            Err(e) => {
                                error!("{}", anyhow::anyhow!(e).to_string());
                            }
                        }
                    }
                })
            },
        ),
    );

    map_command
}

#[tokio::main]
async fn main() {
    env_logger::init();
    let (service_int, rx) = create_service().await;
    let service = Arc::new(RwLock::new(service_int));
    let handle = handle_coming_messages(rx);
    let mut map_command = create_command_map_handler();
    let mut command = String::new();
    let read_from_stdin = stdin();
    let quit = "quit".to_string();

    while command != quit {
        info!("Type your command");
        if let Ok(_) = read_from_stdin.read_line(&mut command) {
            let words: Vec<String> = command
                .split(' ')
                .map(|item| {
                    let chars: Vec<char> = item.chars().collect();

                    if chars[chars.len() - 1] == '\n' {
                        chars[..chars.len() - 1].into_iter().collect()
                    } else {
                        chars.into_iter().collect()
                    }
                })
                .collect();
            if words.len() < 1 {
                error!("Invalid command");
                continue;
            }

            if let Some(function) = map_command.get_mut(&words[0]) {
                function(service.clone(), (&words[1..]).to_vec()).await;
            } else {
                error!("Invalid command");
            }

            command.clear();
        }
    }

    handle.abort();
}
