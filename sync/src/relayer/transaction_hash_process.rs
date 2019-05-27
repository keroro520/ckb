use crate::relayer::Relayer;
use ckb_core::transaction::ProposalShortId;
use ckb_network::{CKBProtocolContext, PeerIndex};
use ckb_protocol::RelayTransactionHash as FbsRelayTransactionHash;
use ckb_store::ChainStore;
use failure::Error as FailureError;
use log::{debug, trace};
use numext_fixed_hash::H256;
use std::convert::TryInto;
use std::sync::Arc;

pub struct TransactionHashProcess<'a, CS> {
    message: &'a FbsRelayTransactionHash<'a>,
    relayer: &'a Relayer<CS>,
    _nc: Arc<dyn CKBProtocolContext>,
    peer: PeerIndex,
}

impl<'a, CS: ChainStore> TransactionHashProcess<'a, CS> {
    pub fn new(
        message: &'a FbsRelayTransactionHash,
        relayer: &'a Relayer<CS>,
        nc: Arc<dyn CKBProtocolContext>,
        peer: PeerIndex,
    ) -> Self {
        TransactionHashProcess {
            message,
            relayer,
            _nc: nc,
            peer,
        }
    }

    pub fn execute(self) -> Result<(), FailureError> {
        let tx_hash: H256 = (*self.message).try_into()?;
        let short_id = ProposalShortId::from_tx_hash(&tx_hash);
        if self.relayer.state.already_known_tx(&tx_hash) {
            debug!(
                target: "relay",
                "transaction({:#x}) from {} already known, ignore it",
                tx_hash,
                self.peer,
            );
        } else if self
            .relayer
            .shared
            .lock_chain_state()
            .tx_pool()
            .get_tx_with_cycles(&short_id)
            .is_some()
        {
            trace!(
                target: "relay",
                "transaction({:#x}) from {} already in transaction pool, ignore it",
                tx_hash,
                self.peer,
            );
            self.relayer.state.mark_as_known_tx(tx_hash.clone());
        } else {
            debug!(
                target: "relay",
                "transaction({:#x}) from {} not known, get it from the peer",
                tx_hash,
                self.peer,
            );
            let last_ask_timeout = self
                .relayer
                .state
                .tx_already_asked
                .lock()
                .get(&tx_hash)
                .cloned();
            if let Some(next_ask_timeout) = self
                .relayer
                .peers_manager
                .state
                .write()
                .get_mut(&self.peer)
                .and_then(|peer_state| peer_state.add_ask_for_tx(tx_hash.clone(), last_ask_timeout))
            {
                self.relayer
                    .state
                    .tx_already_asked
                    .lock()
                    .insert(tx_hash.clone(), next_ask_timeout);
            }
        }

        Ok(())
    }
}
