use crate::relayer::{compact_block::BlockProposal, Relayer};
use crate::{attempt, Status, StatusCode};
use ckb_core::transaction::{ProposalShortId, Transaction};
use ckb_logger::{debug_target, warn_target};
use ckb_network::CKBProtocolContext;
use ckb_protocol::BlockProposal as BlockProposalMessage;
use futures::{self, future::FutureResult, lazy};
use numext_fixed_hash::H256;
use std::convert::TryInto;
use std::sync::Arc;

pub struct BlockProposalProcess<'a> {
    message: &'a BlockProposalMessage<'a>,
    relayer: &'a Relayer,
    nc: Arc<dyn CKBProtocolContext>,
}

impl<'a> BlockProposalProcess<'a> {
    pub fn new(
        message: &'a BlockProposalMessage,
        relayer: &'a Relayer,
        nc: Arc<dyn CKBProtocolContext>,
    ) -> Self {
        BlockProposalProcess {
            message,
            relayer,
            nc,
        }
    }

    pub fn execute(self) -> Status {
        let block_proposal: BlockProposal =
            attempt!(TryInto::<BlockProposal>::try_into(*self.message));
        let txs: Vec<Transaction> = block_proposal.transactions;
        let unknown_txs: Vec<(H256, Transaction)> = txs
            .into_iter()
            .filter_map(|tx| {
                let tx_hash = tx.hash();
                if self.relayer.shared().already_known_tx(&tx_hash) {
                    None
                } else {
                    Some((tx_hash.to_owned(), tx))
                }
            })
            .collect();

        if unknown_txs.is_empty() {
            return Status::ignored();
        }

        let proposals: Vec<ProposalShortId> = unknown_txs
            .iter()
            .map(|(tx_hash, _)| ProposalShortId::from_tx_hash(tx_hash))
            .collect();
        let removes = self.relayer.shared().remove_inflight_proposals(&proposals);
        let mut asked_txs = Vec::new();
        for (previously_in, (tx_hash, transaction)) in removes.into_iter().zip(unknown_txs) {
            if previously_in {
                self.relayer.shared().mark_as_known_tx(tx_hash);
                asked_txs.push(transaction);
            }
        }

        if asked_txs.is_empty() {
            return Status::ignored();
        }

        if let Err(err) = self.nc.future_task(
            {
                let tx_pool_executor = Arc::clone(&self.relayer.tx_pool_executor);
                Box::new(lazy(move || -> FutureResult<(), ()> {
                    let ret = tx_pool_executor.verify_and_add_txs_to_pool(asked_txs);
                    if ret.is_err() {
                        warn_target!(
                            crate::LOG_TARGET_RELAY,
                            "BlockProposal add_tx_to_pool error {:?}",
                            ret
                        )
                    }
                    futures::future::ok(())
                }))
            },
            true,
        ) {
            debug_target!(
                crate::LOG_TARGET_RELAY,
                "relayer send future task error: {:?}",
                err,
            );
        }
        StatusCode::OK.into()
    }
}
