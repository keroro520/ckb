use super::helper::new_index_transaction;
use crate::relayer::block_transactions_verifier::BlockTransactionsVerifier;
use crate::relayer::error::{Error, Misbehavior};

use crate::relayer::compact_block::CompactBlock;
use ckb_core::transaction::IndexTransaction;

// block_short_ids: vec![None, Some(1), None, Some(3), Some(4), None]
fn build_compact_block() -> CompactBlock {
    let mut block = CompactBlock::default();

    let prefilled: Vec<IndexTransaction> = vec![0, 2, 5]
        .into_iter()
        .map(new_index_transaction)
        .collect();

    let short_ids = vec![1, 3, 4]
        .into_iter()
        .map(new_index_transaction)
        .clone()
        .map(|tx| tx.transaction.proposal_short_id())
        .collect();
    block.prefilled_transactions = prefilled;
    block.short_ids = short_ids;

    block
}

#[test]
fn test_invalid() {
    let block = build_compact_block();
    let indexes = vec![1, 3, 4];

    // Invalid len
    let block_txs: Vec<_> = vec![1, 3]
        .into_iter()
        .map(|i| new_index_transaction(i).transaction)
        .collect();

    let ret = BlockTransactionsVerifier::verify(&block, &indexes, block_txs.as_slice());

    assert_eq!(
        ret.err(),
        Some(Error::Misbehavior(
            Misbehavior::InvalidBlockTransactionsLength {
                expected: 3,
                got: 2
            }
        ))
    );

    // Unordered txs
    let block_txs: Vec<_> = vec![1, 4, 3]
        .into_iter()
        .map(|i| new_index_transaction(i).transaction)
        .collect();

    let expected = new_index_transaction(3).transaction.proposal_short_id();
    let got = new_index_transaction(4).transaction.proposal_short_id();

    let ret = BlockTransactionsVerifier::verify(&block, &indexes, &block_txs);

    assert_eq!(
        ret.err(),
        Some(Error::Misbehavior(Misbehavior::InvalidBlockTransactions {
            expected,
            got
        }))
    );
}

#[test]
fn test_ok() {
    let block = build_compact_block();

    let indexes = vec![1, 3, 4];
    let block_txs: Vec<_> = vec![1, 3, 4]
        .into_iter()
        .map(|i| new_index_transaction(i).transaction)
        .collect();

    let ret = BlockTransactionsVerifier::verify(&block, &indexes, &block_txs);

    assert!(ret.is_ok());
}
