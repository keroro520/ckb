use ckb_types::core::TransactionView;
use ckb_types::packed::{ProposalShortId, Transaction};

pub trait GetProposalId {
    fn get_proposal(&self) -> ProposalShortId;
}

impl GetProposalId for TransactionView {
    fn get_proposal(&self) -> ProposalShortId {
        self.proposal_short_id()
    }
}

impl GetProposalId for Transaction {
    fn get_proposal(&self) -> ProposalShortId {
        self.proposal_short_id()
    }
}

impl GetProposalId for ProposalShortId {
    fn get_proposal(&self) -> ProposalShortId {
        self.clone()
    }
}
