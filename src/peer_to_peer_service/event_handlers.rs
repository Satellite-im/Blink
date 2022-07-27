use libp2p::kad::{KademliaEvent, QueryResult};
use libp2p::Swarm;
use libp2p::swarm::SwarmEvent;
use crate::peer_to_peer_service::behavior::{BehaviourEvent, BlinkBehavior};

pub(crate) trait EventHandler<TErr> {
    fn can_handle(&mut self, event: &SwarmEvent<BehaviourEvent, TErr>) -> bool;
    fn handle(&mut self, swarm: &mut Swarm<BlinkBehavior>, event: &SwarmEvent<BehaviourEvent, TErr>);
}

#[derive(Default)]
pub(crate) struct PairResultEventHandler {
}

impl<TErr> EventHandler<TErr> for PairResultEventHandler {
    fn can_handle(&mut self, event: &SwarmEvent<BehaviourEvent, TErr>) -> bool {
        if let SwarmEvent::Behaviour(BehaviourEvent::KademliaEvent(ev)) = event {
            if let KademliaEvent::OutboundQueryCompleted { .. } = ev {
                return true;
            }
        }

        false
    }

    fn handle(&mut self, swarm: &mut Swarm<BlinkBehavior>, event: &SwarmEvent<BehaviourEvent, TErr>) {
        if let SwarmEvent::Behaviour(BehaviourEvent::KademliaEvent(ev)) = event {
            if let KademliaEvent::OutboundQueryCompleted { result: outbound, .. } = ev {
                match outbound {
                    QueryResult::Bootstrap(_) => {}
                    QueryResult::GetClosestPeers(_) => {}
                    QueryResult::GetProviders(_) => {}
                    QueryResult::StartProviding(_) => {}
                    QueryResult::RepublishProvider(_) => {}
                    QueryResult::GetRecord(_) => {}
                    QueryResult::PutRecord(_) => {}
                    QueryResult::RepublishRecord(_) => {}
                }
            }
        }
    }
}
