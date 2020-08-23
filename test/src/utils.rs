use crate::{Net, Node, TXOSet};
use ckb_jsonrpc_types::{BlockTemplate, TransactionWithStatus, TxStatus};
use ckb_types::{
    core::{BlockNumber, BlockView, EpochNumber, TransactionView},
    h256, packed,
    prelude::*,
    H256,
};
use std::convert::Into;
use std::env;
use std::fs::read_to_string;
use std::path::PathBuf;
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

pub const FLAG_SINCE_RELATIVE: u64 =
    0b1000_0000_0000_0000_0000_0000_0000_0000_0000_0000_0000_0000_0000_0000_0000_0000;
pub const FLAG_SINCE_BLOCK_NUMBER: u64 =
    0b000_0000_0000_0000_0000_0000_0000_0000_0000_0000_0000_0000_0000_0000_0000_0000;
pub const FLAG_SINCE_EPOCH_NUMBER: u64 =
    0b010_0000_0000_0000_0000_0000_0000_0000_0000_0000_0000_0000_0000_0000_0000_0000;
pub const FLAG_SINCE_TIMESTAMP: u64 =
    0b100_0000_0000_0000_0000_0000_0000_0000_0000_0000_0000_0000_0000_0000_0000_0000;

pub fn new_block_with_template(template: BlockTemplate) -> BlockView {
    packed::Block::from(template)
        .as_advanced_builder()
        .nonce(rand::random::<u128>().pack())
        .build()
}

pub fn wait_until<F>(secs: u64, mut f: F) -> bool
where
    F: FnMut() -> bool,
{
    let timeout = tweaked_duration(secs);
    let start = Instant::now();
    while Instant::now().duration_since(start) <= timeout {
        if f() {
            return true;
        }
        thread::sleep(Duration::new(1, 0));
    }
    false
}

pub fn sleep(secs: u64) {
    thread::sleep(tweaked_duration(secs));
}

fn tweaked_duration(secs: u64) -> Duration {
    let sec_coefficient = env::var("CKB_TEST_SEC_COEFFICIENT")
        .unwrap_or_default()
        .parse()
        .unwrap_or(1.0);
    Duration::from_secs((secs as f64 * sec_coefficient) as u64)
}

// Clear net message channel
pub fn clear_messages(net: &Net) {
    while net.receive_timeout(Duration::new(3, 0)).is_ok() {}
}

pub fn since_from_relative_block_number(block_number: BlockNumber) -> u64 {
    FLAG_SINCE_RELATIVE | FLAG_SINCE_BLOCK_NUMBER | block_number
}

pub fn since_from_absolute_block_number(block_number: BlockNumber) -> u64 {
    FLAG_SINCE_BLOCK_NUMBER | block_number
}

pub fn since_from_relative_epoch_number(epoch_number: EpochNumber) -> u64 {
    FLAG_SINCE_RELATIVE | FLAG_SINCE_EPOCH_NUMBER | epoch_number
}

pub fn since_from_absolute_epoch_number(epoch_number: EpochNumber) -> u64 {
    FLAG_SINCE_EPOCH_NUMBER | epoch_number
}

pub fn since_from_relative_timestamp(timestamp: u64) -> u64 {
    FLAG_SINCE_RELATIVE | FLAG_SINCE_TIMESTAMP | timestamp
}

pub fn since_from_absolute_timestamp(timestamp: u64) -> u64 {
    FLAG_SINCE_TIMESTAMP | timestamp
}

pub fn assert_send_transaction_fail(node: &Node, transaction: &TransactionView, message: &str) {
    let result = node
        .rpc_client()
        .send_transaction_result(transaction.data().into());
    assert!(
        result.is_err(),
        "expect error \"{}\" but got \"Ok(())\"",
        message,
    );
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
    let committed_status = TxStatus::committed(h256!("0x0"));
    tx_status.tx_status.status == committed_status.status
}

/// Return a random path located on temp_dir
///
/// We use `tempdir` only for generating a random path, and expect the corresponding directory
/// that `tempdir` creates be deleted when go out of this function.
pub fn temp_path(case_name: &str, id: &str) -> String {
    let mut builder = tempfile::Builder::new();
    let prefix = ["ckb-it", case_name, id, ""].join("-");
    builder.prefix(&prefix);
    let tempdir = if let Ok(val) = env::var("CKB_INTEGRATION_TEST_TMP") {
        builder.tempdir_in(val)
    } else {
        builder.tempdir()
    }
    .expect("create tempdir failed");
    let path = tempdir.path().to_str().unwrap().to_owned();
    tempdir.close().expect("close tempdir failed");
    path
}

/// Generate new blocks and explode these cellbases into `n` live cells
pub fn generate_utxo_set(node: &Node, n: usize) -> TXOSet {
    // Ensure all the cellbases will be used later are already mature.
    let cellbase_maturity = node.consensus().cellbase_maturity();
    node.generate_blocks(cellbase_maturity.index() as usize);

    // Explode these mature cellbases into multiple cells
    let mut n_outputs = 0;
    let mut txs = Vec::new();
    while n > n_outputs {
        node.generate_block();
        let mature_number = node.get_tip_block_number() - cellbase_maturity.index();
        let mature_block = node.get_block_by_number(mature_number);
        let mature_cellbase = mature_block.transaction(0).unwrap();
        if mature_cellbase.outputs().is_empty() {
            continue;
        }

        let mature_utxos: TXOSet = TXOSet::from(&mature_cellbase);
        let tx = mature_utxos.boom(vec![node.always_success_cell_dep()]);
        n_outputs += tx.outputs().len();
        txs.push(tx);
    }

    // Ensure all the transactions were committed
    txs.iter().for_each(|tx| {
        node.submit_transaction(tx);
    });
    while txs
        .iter()
        .any(|tx| !is_committed(&node.rpc_client().get_transaction(tx.hash()).unwrap()))
    {
        node.generate_blocks(node.consensus().finalization_delay_length() as usize);
    }

    let mut utxos = TXOSet::default();
    txs.iter()
        .for_each(|tx| utxos.extend(Into::<TXOSet>::into(tx)));
    utxos.truncate(n);
    utxos
}

/// Return a blank block with additional committed transactions
pub fn commit(node: &Node, committed: &[&TransactionView]) -> BlockView {
    let committed = committed
        .iter()
        .map(|t| t.to_owned().to_owned())
        .collect::<Vec<_>>();
    blank(node)
        .as_advanced_builder()
        .transactions(committed)
        .build()
}

/// Return a blank block with additional proposed transactions
pub fn propose(node: &Node, proposals: &[&TransactionView]) -> BlockView {
    let proposals = proposals.iter().map(|tx| tx.proposal_short_id());
    blank(node)
        .as_advanced_builder()
        .proposals(proposals)
        .build()
}

/// Return a block with `proposals = [], transactions = [cellbase], uncles = []`
pub fn blank(node: &Node) -> BlockView {
    let example = node.new_block(None, None, None);
    example
        .as_advanced_builder()
        .set_proposals(vec![])
        .set_transactions(vec![example.transaction(0).unwrap()]) // cellbase
        .set_uncles(vec![])
        .build()
}

// grep "panicked at" $node_log_path
pub fn nodes_panicked(node_dirs: &[String]) -> bool {
    node_dirs.iter().any(|node_dir| {
        read_to_string(&node_log(&node_dir))
            .expect("failed to read node's log")
            .contains("panicked at")
    })
}

// node_log=$node_dir/data/logs/run.log
pub fn node_log(node_dir: &str) -> PathBuf {
    PathBuf::from(node_dir)
        .join("data")
        .join("logs")
        .join("run.log")
}

pub fn now_ms() -> u64 {
    let start = SystemTime::now();
    let since_the_epoch = start
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards");
    since_the_epoch.as_millis() as u64
}
