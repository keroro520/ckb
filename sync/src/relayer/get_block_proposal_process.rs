use crate::relayer::Relayer;
use ckb_core::transaction::ProposalShortId;
use ckb_network::{CKBProtocolContext, PeerIndex};
use ckb_protocol::{cast, GetBlockProposal, RelayMessage};
use ckb_store::ChainStore;
use failure::Error as FailureError;
use flatbuffers::FlatBufferBuilder;
use std::convert::TryInto;
use std::sync::Arc;

pub struct GetBlockProposalProcess<'a, CS> {
    message: &'a GetBlockProposal<'a>,
    relayer: &'a Relayer<CS>,
    nc: Arc<dyn CKBProtocolContext>,
    peer: PeerIndex,
}

impl<'a, CS: ChainStore> GetBlockProposalProcess<'a, CS> {
    pub fn new(
        message: &'a GetBlockProposal,
        relayer: &'a Relayer<CS>,
        nc: Arc<dyn CKBProtocolContext>,
        peer: PeerIndex,
    ) -> Self {
        GetBlockProposalProcess {
            message,
            nc,
            relayer,
            peer,
        }
    }

    pub fn execute(self) -> Result<(), FailureError> {
        let mut pending_proposals_request = self.relayer.state.pending_proposal_requests.lock();
        let proposals = cast!(self.message.proposals())?;

        let transactions = {
            let chain_state = self.relayer.shared.lock_chain_state();
            let tx_pool = chain_state.tx_pool();

            let proposals: Vec<ProposalShortId> = proposals
                .iter()
                .map(TryInto::try_into)
                .collect::<Result<_, FailureError>>()?;

            proposals
                .into_iter()
                .filter_map(|short_id| {
                    tx_pool.get_tx(&short_id).or({
                        pending_proposals_request
                            .entry(short_id)
                            .or_insert_with(Default::default)
                            .insert(self.peer);
                        None
                    })
                })
                .collect::<Vec<_>>()
        };

        let fbb = &mut FlatBufferBuilder::new();
        let message = RelayMessage::build_block_proposal(fbb, &transactions);
        fbb.finish(message, None);

        self.nc
            .send_message_to(self.peer, fbb.finished_data().into());
        Ok(())
    }
}
