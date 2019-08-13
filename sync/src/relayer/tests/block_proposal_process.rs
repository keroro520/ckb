use crate::relayer::block_proposal_process::BlockProposalProcess;
use crate::relayer::tests::helper::{build_chain, new_transaction, MockProtocalContext};
use crate::StatusCode;
use ckb_core::transaction::ProposalShortId;
use ckb_protocol::{cast, get_root, BlockProposal, RelayMessage};
use flatbuffers::FlatBufferBuilder;
use std::sync::Arc;

#[test]
fn test_no_unknown() {
    let (relayer, always_success_out_point) = build_chain(5);

    let transaction = new_transaction(&relayer, 1, &always_success_out_point);

    let transactions = vec![transaction.clone()];

    // known tx
    {
        relayer
            .shared
            .mark_as_known_tx(transaction.hash().to_owned());
    }

    let builder = &mut FlatBufferBuilder::new();
    let b = RelayMessage::build_block_proposal(builder, transactions.as_ref());
    builder.finish(b, None);

    let message = get_root::<RelayMessage>(builder.finished_data()).unwrap();
    let block_proposal: BlockProposal = cast!(message.payload_as_block_proposal()).unwrap();

    let mock_protocal_context = MockProtocalContext::default();
    let nc = Arc::new(mock_protocal_context);

    let process = BlockProposalProcess::new(
        &block_proposal,
        &relayer,
        Arc::<MockProtocalContext>::clone(&nc),
    );
    assert_eq!(process.execute().code, StatusCode::Ignored);
}

#[test]
fn test_no_asked() {
    let (relayer, always_success_out_point) = build_chain(5);

    let transaction = new_transaction(&relayer, 1, &always_success_out_point);

    let transactions = vec![transaction.clone()];

    let builder = &mut FlatBufferBuilder::new();
    let b = RelayMessage::build_block_proposal(builder, transactions.as_ref());
    builder.finish(b, None);

    let message = get_root::<RelayMessage>(builder.finished_data()).unwrap();
    let block_proposal: BlockProposal = cast!(message.payload_as_block_proposal()).unwrap();

    let mock_protocal_context = MockProtocalContext::default();
    let nc = Arc::new(mock_protocal_context);

    let process = BlockProposalProcess::new(
        &block_proposal,
        &relayer,
        Arc::<MockProtocalContext>::clone(&nc),
    );
    assert_eq!(process.execute().code, StatusCode::Ignored);

    let known = relayer.shared.already_known_tx(transaction.hash());
    assert_eq!(known, false);
}

#[test]
fn test_ok() {
    let (relayer, always_success_out_point) = build_chain(5);

    let transaction = new_transaction(&relayer, 1, &always_success_out_point);
    let transactions = vec![transaction.clone()];
    let proposals: Vec<ProposalShortId> = transactions
        .iter()
        .map(|tx| ProposalShortId::from_tx_hash(tx.hash()))
        .collect();

    // Before asked proposals
    {
        relayer.shared.insert_inflight_proposals(proposals);
    }

    let builder = &mut FlatBufferBuilder::new();
    let b = RelayMessage::build_block_proposal(builder, transactions.as_ref());
    builder.finish(b, None);

    let message = get_root::<RelayMessage>(builder.finished_data()).unwrap();
    let block_proposal: BlockProposal = cast!(message.payload_as_block_proposal()).unwrap();

    let mock_protocal_context = MockProtocalContext::default();
    let nc = Arc::new(mock_protocal_context);

    let process = BlockProposalProcess::new(
        &block_proposal,
        &relayer,
        Arc::<MockProtocalContext>::clone(&nc),
    );
    assert!(process.execute().is_ok());

    let known = relayer.shared.already_known_tx(transaction.hash());
    assert_eq!(known, true);
}
