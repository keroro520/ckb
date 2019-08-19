use crate::{Net, Node};
use bytes::Bytes;
use ckb_core::block::{Block, BlockBuilder};
use ckb_core::header::{Header, HeaderBuilder, Seal};
use ckb_core::transaction::Transaction;
use ckb_core::BlockNumber;
use ckb_jsonrpc_types::{BlockTemplate, TransactionWithStatus, TxStatus};
use ckb_protocol::{RelayMessage, SyncMessage};
use flatbuffers::FlatBufferBuilder;
use numext_fixed_hash::H256;
use std::collections::HashSet;
use std::convert::Into;
use std::thread::sleep;
use std::time::{Duration, Instant};

pub const MEDIAN_TIME_BLOCK_COUNT: u64 = 11;
pub const FLAG_SINCE_RELATIVE: u64 =
    0b1000_0000_0000_0000_0000_0000_0000_0000_0000_0000_0000_0000_0000_0000_0000_0000;
pub const FLAG_SINCE_BLOCK_NUMBER: u64 =
    0b000_0000_0000_0000_0000_0000_0000_0000_0000_0000_0000_0000_0000_0000_0000_0000;
// pub const FLAG_SINCE_EPOCH_NUMBER: u64 =
//    0b010_0000_0000_0000_0000_0000_0000_0000_0000_0000_0000_0000_0000_0000_0000_0000;
pub const FLAG_SINCE_TIMESTAMP: u64 =
    0b100_0000_0000_0000_0000_0000_0000_0000_0000_0000_0000_0000_0000_0000_0000_0000;

// Build compact block based on core block, and specific prefilled indices
pub fn build_compact_block_with_prefilled(block: &Block, prefilled: Vec<usize>) -> Bytes {
    let prefilled = prefilled.into_iter().collect();
    let fbb = &mut FlatBufferBuilder::new();
    let message = RelayMessage::build_compact_block(fbb, &block, &prefilled);
    fbb.finish(message, None);
    fbb.finished_data().into()
}

// Build compact block based on core block
pub fn build_compact_block(block: &Block) -> Bytes {
    let fbb = &mut FlatBufferBuilder::new();
    let message = RelayMessage::build_compact_block(fbb, &block, &HashSet::new());
    fbb.finish(message, None);
    fbb.finished_data().into()
}

pub fn build_block_transactions(block: &Block) -> Bytes {
    let fbb = &mut FlatBufferBuilder::new();
    // compact block has always prefilled cellbase
    let message = RelayMessage::build_block_transactions(
        fbb,
        block.header().hash(),
        &block.transactions()[1..],
    );
    fbb.finish(message, None);
    fbb.finished_data().into()
}

pub fn build_header(header: &Header) -> Bytes {
    build_headers(&[header.clone()])
}

pub fn build_headers(headers: &[Header]) -> Bytes {
    let fbb = &mut FlatBufferBuilder::new();
    let message = SyncMessage::build_headers(fbb, headers);
    fbb.finish(message, None);
    fbb.finished_data().into()
}

pub fn build_block(block: &Block) -> Bytes {
    let fbb = &mut FlatBufferBuilder::new();
    let message = SyncMessage::build_block(fbb, block);
    fbb.finish(message, None);
    fbb.finished_data().into()
}

pub fn build_get_blocks(hashes: &[H256]) -> Bytes {
    let fbb = &mut FlatBufferBuilder::new();
    let message = SyncMessage::build_get_blocks(fbb, hashes);
    fbb.finish(message, None);
    fbb.finished_data().into()
}

pub fn new_block_with_template(template: BlockTemplate) -> Block {
    let cellbase = template.cellbase.data;
    let header_builder = HeaderBuilder::default()
        .version(template.version.0)
        .number(template.number.0)
        .difficulty(template.difficulty.clone())
        .timestamp(template.current_time.0)
        .parent_hash(template.parent_hash)
        .seal(Seal::new(rand::random(), Bytes::new()))
        .dao(template.dao.into_bytes());

    BlockBuilder::default()
        .uncles(template.uncles)
        .transaction(cellbase)
        .transactions(template.transactions)
        .proposals(template.proposals)
        .header_builder(header_builder)
        .build()
}

pub fn wait_until<F>(secs: u64, mut f: F) -> bool
where
    F: FnMut() -> bool,
{
    let start = Instant::now();
    let timeout = Duration::new(secs, 0);
    while Instant::now().duration_since(start) <= timeout {
        if f() {
            return true;
        }
        sleep(Duration::new(1, 0));
    }
    false
}

// Clear net message channel
pub fn clear_messages(net: &Net) {
    while let Ok(_) = net.receive_timeout(Duration::new(3, 0)) {}
}

pub fn since_from_relative_block_number(block_number: BlockNumber) -> u64 {
    FLAG_SINCE_RELATIVE | FLAG_SINCE_BLOCK_NUMBER | block_number
}

pub fn since_from_absolute_block_number(block_number: BlockNumber) -> u64 {
    FLAG_SINCE_BLOCK_NUMBER | block_number
}

// pub fn since_from_relative_epoch_number(epoch_number: EpochNumber) -> u64 {
//     FLAG_SINCE_RELATIVE | FLAG_SINCE_EPOCH_NUMBER | epoch_number
// }
//
// pub fn since_from_absolute_epoch_number(epoch_number: EpochNumber) -> u64 {
//     FLAG_SINCE_EPOCH_NUMBER | epoch_number
// }

pub fn since_from_relative_timestamp(timestamp: u64) -> u64 {
    FLAG_SINCE_RELATIVE | FLAG_SINCE_TIMESTAMP | timestamp
}

pub fn since_from_absolute_timestamp(timestamp: u64) -> u64 {
    FLAG_SINCE_TIMESTAMP | timestamp
}

pub fn assert_send_transaction_fail(node: &Node, transaction: &Transaction, message: &str) {
    let result = node
        .rpc_client()
        .inner()
        .send_transaction(transaction.into())
        .call();
    let error = result.expect_err(&format!("transaction is invalid since {}", message));
    let error_string = error.to_string();
    assert!(
        error_string.contains(message),
        "expect error \"{}\" but got \"{}\"",
        message,
        error_string,
    );
}

pub fn is_committed(tx_status: &TransactionWithStatus) -> bool {
    let committed_status = TxStatus::committed(H256::zero());
    tx_status.tx_status.status == committed_status.status
}

pub fn assert_tx_pool_size(node: &Node, pending_size: u64, proposed_size: u64) {
    let tx_pool_info = node.tx_pool_info();
    assert_eq!(tx_pool_info.pending.0, pending_size);
    assert_eq!(tx_pool_info.proposed.0, proposed_size);
}

pub fn assert_tx_pool_statics(node: &Node, total_tx_size: u64, total_tx_cycles: u64) {
    let tx_pool_info = node.tx_pool_info();
    assert_eq!(tx_pool_info.total_tx_size.0, total_tx_size);
    assert_eq!(tx_pool_info.total_tx_cycles.0, total_tx_cycles);
}

// workaround for banned address checking (because we are using loopback address)
// 1. checking banned addresses is empty
// 2. connecting outbound peer and checking banned addresses is not empty
// 3. clear banned addresses
pub fn connect_and_wait_ban(inbound: &Node, outbound: &Node) {
    let node_info = outbound.local_node_info();
    let node_id = node_info.node_id;

    assert!(
        inbound.get_banned_addresses().is_empty(),
        "banned addresses should be empty"
    );
    inbound.add_node(
        node_id.clone(),
        format!("/ip4/127.0.0.1/tcp/{}", outbound.p2p_port()),
    );

    let result = wait_until(10, || {
        let banned_addresses = inbound.get_banned_addresses();
        let result = banned_addresses.is_empty();
        banned_addresses.into_iter().for_each(|ban_address| {
            inbound.set_ban(ban_address.address, "delete".to_owned(), None, None, None)
        });
        result
    });

    if !result {
        panic!(
            "Connect and wait ban outbound peer timeout, node id: {}",
            node_id
        );
    }
}

pub fn waiting_for_sync(node0: &Node, node1: &Node, target: BlockNumber) {
    let (mut self_tip_number, mut node_tip_number) = (0, 0);
    // 60 seconds is a reasonable timeout to sync, even for poor CI server
    let result = wait_until(60, || {
        self_tip_number = node0.tip_number();
        node_tip_number = node1.tip_number();
        self_tip_number == node_tip_number && target == self_tip_number
    });

    if !result {
        panic!(
            "Waiting for sync timeout, self_tip_number: {}, node_tip_number: {}",
            self_tip_number, node_tip_number
        );
    }
}
