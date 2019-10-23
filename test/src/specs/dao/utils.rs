use crate::utils::{is_committed, since_from_absolute_epoch_number};
use crate::Node;
use ckb_chain_spec::OUTPUT_INDEX_DAO;
use ckb_types::core::EpochNumberWithFraction;
use ckb_types::{
    bytes::Bytes,
    core::{Capacity, ScriptHashType, TransactionBuilder, TransactionView},
    packed::{Byte32, CellDep, CellInput, CellOutput, OutPoint, Script},
    prelude::*,
};

// https://github.com/nervosnetwork/ckb-system-scripts/blob/1fd4cd3e2ab7e5ffbafce1f60119b95937b3c6eb/c/dao.c#L81
pub(crate) const LOCK_PERIOD_EPOCHES: u64 = 180;

/// Deposit Transaction:
///
/// ```ignore
/// Transaction {
///   inputs: [unspent_input],
///   outputs: [deposit_output{lock: always, type: dao}],
///   cell_deps: [always, dao],
///   header_deps: [],
///   outputs_data: [[]],
/// }
/// ```
pub(crate) fn deposit_transaction(node: &Node, unspent: OutPoint) -> TransactionView {
    let input = deposit_input(node, unspent);
    let output = deposit_output(node, &input);
    let cell_deps = deposit_cell_deps(node);
    TransactionBuilder::default()
        .cell_deps(cell_deps)
        .input(input)
        .output(output)
        .witness(Default::default())
        .output_data(Default::default())
        .build()
}

/// Withdraw Transaction:
///
/// ```ignore
/// Transaction {
///   inputs: [deposit_input], // deposit_input.since = deposit epoch + LOCK_PERIOD_EPOCHES
///   outputs: [unspent_output],
///   cell_deps: [always, dao],
///   header_deps: [deposit_header_hash, withdraw_header_hash], // used in dao.c and DaOCalculator to calculate interest
///   witnesses: [ [1] ],  // the index of `withdraw_header_hash` in `header_deps`
///   outputs_data: [[]],
/// }
/// ```
pub(crate) fn withdraw_transaction(
    node: &Node,
    deposit_out_point: OutPoint,
    withdraw_header_hash: Byte32,
) -> TransactionView {
    let input = withdraw_input(node, deposit_out_point.clone());
    let output = withdraw_output(
        node,
        deposit_out_point.clone(),
        withdraw_header_hash.clone(),
    );
    let cell_deps = withdraw_cell_deps(node);
    let header_deps = {
        let deposit_header_hash = node
            .rpc_client()
            .get_transaction(deposit_out_point.tx_hash())
            .unwrap()
            .tx_status
            .block_hash
            .unwrap();
        withdraw_header_deps(
            node,
            deposit_header_hash.pack(),
            withdraw_header_hash.clone(),
        )
    };
    let withdraw_header_index = header_deps
        .iter()
        .position(|hash| hash == &withdraw_header_hash)
        .unwrap();
    let witness = Bytes::from((withdraw_header_index as u64).to_le_bytes().to_vec()).pack();
    TransactionBuilder::default()
        .input(input)
        .output(output)
        .cell_deps(cell_deps)
        .header_deps(header_deps)
        .witness(witness)
        .output_data(Default::default())
        .build()
}

/// Send the given transaction and make it committed
pub(crate) fn ensure_committed(node: &Node, transaction: &TransactionView) -> OutPoint {
    let commit_elapsed = node.consensus().tx_proposal_window().closest() as usize + 2;
    let tx_hash = node
        .rpc_client()
        .send_transaction(transaction.data().into());
    node.generate_blocks(commit_elapsed);
    let tx_status = node
        .rpc_client()
        .get_transaction(tx_hash.clone())
        .expect("get sent transaction");
    assert!(
        is_committed(&tx_status),
        "ensure_committed failed {}",
        tx_hash
    );
    OutPoint::new(tx_hash, 0)
}

/// A helper function keep the node growing until into the target EpochNumberWithFraction.
pub(crate) fn goto_target_point(node: &Node, target_point: EpochNumberWithFraction) {
    loop {
        let tip_epoch = node.rpc_client().get_tip_header().inner.epoch;
        let tip_point = EpochNumberWithFraction::from_full_value(tip_epoch.value());

        // Here is our target EpochNumberWithFraction.
        if tip_point >= target_point {
            break;
        }

        node.generate_block();
    }
}

fn deposit_cell_deps(node: &Node) -> Vec<CellDep> {
    let always_success_cell_dep = node.always_success_cell_dep();
    let dao_cell_dep = CellDep::new_builder()
        .out_point(OutPoint::new(
            node.consensus()
                .genesis_block()
                .transaction(0)
                .unwrap()
                .hash(),
            OUTPUT_INDEX_DAO as u32,
        ))
        .build();
    vec![always_success_cell_dep, dao_cell_dep]
}

fn deposit_input(_node: &Node, out_point: OutPoint) -> CellInput {
    CellInput::new(out_point, 0)
}

pub(crate) fn deposit_type_script(node: &Node) -> Script {
    Script::new_builder()
        .code_hash(node.consensus().dao_type_hash().unwrap())
        .hash_type(ScriptHashType::Type.into())
        .build()
}

fn deposit_output(node: &Node, input: &CellInput) -> CellOutput {
    let capacity = node
        .rpc_client()
        .get_live_cell(input.previous_output().into(), false)
        .cell
        .expect("live cell")
        .output
        .capacity
        .value();
    CellOutput::new_builder()
        .capacity(Capacity::shannons(capacity).pack())
        .lock(node.always_success_script())
        .type_(Some(deposit_type_script(node)).pack())
        .build()
}

fn withdraw_cell_deps(node: &Node) -> Vec<CellDep> {
    deposit_cell_deps(node)
}

fn withdraw_header_deps(
    _node: &Node,
    deposit_header_hash: Byte32,
    withdraw_header_hash: Byte32,
) -> Vec<Byte32> {
    vec![deposit_header_hash, withdraw_header_hash]
}

fn withdraw_input(node: &Node, deposit_out_point: OutPoint) -> CellInput {
    let minimal_unlock_point = minimal_unlock_point(node, &deposit_out_point);
    let since = since_from_absolute_epoch_number(minimal_unlock_point.full_value());
    CellInput::new(deposit_out_point, since)
}

fn withdraw_output(
    node: &Node,
    deposit_out_point: OutPoint,
    withdraw_header_hash: Byte32,
) -> CellOutput {
    // capacity = deposit_input_capacity + dao_interest
    let capacity = node
        .rpc_client()
        .calculate_dao_maximum_withdraw(deposit_out_point.into(), withdraw_header_hash);
    CellOutput::new_builder()
        .capacity(capacity.pack())
        .lock(node.always_success_script())
        .build()
}

pub fn minimal_unlock_point(node: &Node, deposit_out_point: &OutPoint) -> EpochNumberWithFraction {
    let deposit_point = {
        let deposit_hash = node
            .rpc_client()
            .get_transaction(deposit_out_point.tx_hash())
            .unwrap()
            .tx_status
            .block_hash
            .unwrap();
        let deposit_header = node.rpc_client().get_header(deposit_hash.pack()).unwrap();
        EpochNumberWithFraction::from_full_value(deposit_header.inner.epoch.value())
    };
    EpochNumberWithFraction::new(
        deposit_point.number() + LOCK_PERIOD_EPOCHES,
        deposit_point.index(),
        deposit_point.length(),
    )
}
