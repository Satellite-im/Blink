use libp2p::Swarm;
use crate::peer_to_peer_service::behavior::BlinkBehavior;
use crate::peer_to_peer_service::BlinkCommand;

pub(crate) trait BlinkCommandHandler {
    fn can_handle(&mut self, command: &BlinkCommand) -> bool;
    fn handle(&mut self, swarm: &mut Swarm<BlinkBehavior>, command: &BlinkCommand);
}

#[derive(Default)]
pub(crate) struct PairCommandHandler {
}

impl BlinkCommandHandler for PairCommandHandler {
    fn can_handle(&mut self, command: &BlinkCommand) -> bool {
        if let BlinkCommand::Pair(x) = command {
            return true;
        }

        false
    }

    fn handle(&mut self, swarm: &mut Swarm<BlinkBehavior>, command: &BlinkCommand) {
        if let BlinkCommand::Pair(peers) = command {
            for peer in peers {
                swarm.behaviour_mut().find_peer(*peer);
            }
        }
    }
}