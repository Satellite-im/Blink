use crate::{
    did_key::Ed25519KeyPair,
    trait_impl::{EventHandlerImpl, MultiPassImpl, PocketDimensionImpl},
};
use anyhow::Result;
use blink_impl::peer_to_peer_service::peer_to_peer_service::{MessageContent, PeerToPeerService};
use libp2p::Multiaddr;
use sata::{error::Error, libipld::IpldCodec, Kind, Sata};
use std::{
    collections::HashMap, future::Future, io::stdin, pin::Pin, sync::atomic::AtomicBool, sync::Arc,
};
use tokio::{main, sync::RwLock, task::JoinHandle};
use tokio::sync::mpsc::Receiver;
use warp::crypto::{did_key, DID};
use log::{debug, error, log_enabled, info, Level};

mod trait_impl;

fn handle_coming_messages(mut receiver: Receiver<MessageContent>) -> JoinHandle<()> {
    let handle = tokio::spawn(async move {
        loop {
            let message = receiver.recv().await;

            if let Some(message_content) = message {
                let res = std::str::from_utf8(&message_content.1.data()).unwrap().to_string();
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
    let id_keys = Arc::new(RwLock::new(DID::from(did_key::generate::<Ed25519KeyPair>(
        None,
    ))));
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
        dyn FnMut(
            Arc<RwLock<PeerToPeerService>>,
            Vec<String>,
        ) -> Pin<Box<dyn Future<Output = ()>>>,
    >,
> {
    let mut map_command: HashMap<
        String,
        Box<
            dyn FnMut(
                Arc<RwLock<PeerToPeerService>>,
                Vec<String>,
            ) -> Pin<Box<dyn Future<Output = ()>>>,
        >> = HashMap::new();

    map_command.insert(
        "pair".to_string(),
        Box::new(
            |service: Arc<RwLock<PeerToPeerService>>, args: Vec<String>| {
                Box::pin(async move {
                    if args.len() == 1 {
                        let addr = args[0].parse::<Multiaddr>();
                        match addr {
                            Ok(x) => {
                                let mut service_write = service.write().await;
                                match service_write.pair_to_another_peer(x.into()).await {
                                    Ok(_) => {
                                        info!("Sucess sending pairing request");
                                    }
                                    Err(_) => {
                                        error!("Failure sending pairing request");
                                    }
                                }
                            }
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
        "subscribe".to_string(),
        Box::new(
            |service: Arc<RwLock<PeerToPeerService>>, args: Vec<String>| {
                Box::pin(async move {
                    if args.len() == 1 {
                        let service_write = service.write().await;
                        match service_write.subscribe_to_topic(args[0].clone()).await {
                            Ok(_) => {
                                info!("Success sending topic subscription");
                            }
                            Err(_) => {
                                error!("Failure to send topic subscription");
                            }
                        }
                    } else {
                        error!("subscribe topic");
                    }
                })
            },
        ),
    );

    map_command.insert(
        "publish".to_string(),
        Box::new(
            |service: Arc<RwLock<PeerToPeerService>>, args: Vec<String>| {
                Box::pin(async move {
                    if args.len() == 2 {
                        let mut service_write = service.write().await;
                        let sata = Sata::default();
                        let result = sata.encode(IpldCodec::DagJson, Kind::Dynamic, args[1].clone());
                        if result.is_ok() {
                            match service_write
                                .publish_message_to_topic(args[0].clone(), result.unwrap())
                                .await
                            {
                                Ok(_) => {
                                    info!("Success sending publish message request");
                                }
                                Err(_) => {
                                    error!("Failure sending publish message request");
                                }
                            }
                        } else {
                            error!("Error encoding data");
                        }
                    } else {
                        error!("publish topic content")
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
