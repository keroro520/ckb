use super::GetProposalId;
use ckb_types::core::BlockView;
use ckb_types::packed::{Block, ProposalShortId};

pub trait GetProposalIds {
    fn get_proposal_ids(&self) -> Vec<ProposalShortId>;
}

impl<T> GetProposalIds for T
where
    T: GetProposalId,
{
    fn get_proposal_ids(&self) -> Vec<ProposalShortId> {
        vec![self.get_proposal()]
    }
}

impl<T> GetProposalIds for Vec<T>
where
    T: GetProposalId,
{
    fn get_proposal_ids(&self) -> Vec<ProposalShortId> {
        self.iter().map(|t| t.get_proposal()).collect()
    }
}

impl GetProposalIds for Block {
    fn get_proposal_ids(&self) -> Vec<ProposalShortId> {
        self.proposals().into_iter().collect()
    }
}

impl GetProposalIds for BlockView {
    fn get_proposal_ids(&self) -> Vec<ProposalShortId> {
        self.data().get_proposal_ids()
    }
}
