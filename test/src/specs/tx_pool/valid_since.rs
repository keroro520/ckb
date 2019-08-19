use crate::utils::{
    assert_send_transaction_fail, assert_tx_pool_size, since_from_absolute_block_number,
    since_from_absolute_timestamp, since_from_relative_block_number, since_from_relative_timestamp,
    MEDIAN_TIME_BLOCK_COUNT,
};
use crate::{assert_regex_match, Net, Node, Spec, DEFAULT_TX_PROPOSAL_WINDOW};
use ckb_chain_spec::ChainSpec;
use ckb_core::transaction::{CellInput, OutPoint, Transaction};
use ckb_core::BlockNumber;
use log::info;
use numext_fixed_hash::H256;
use std::cmp::max;
use std::thread::sleep;
use std::time::Duration;

pub struct ValidSince;

// TODO add cases verify compact block(forks) including transaction of which since != 0
impl Spec for ValidSince {
    crate::name!("valid_since");

    fn run(&self, _net: Net, nodes: Vec<Node>) {
        self.test_since_relative_block_number(&nodes[0]);
        self.test_since_absolute_block_number(&nodes[0]);
        self.test_since_relative_median_time(&nodes[0]);
        self.test_since_absolute_median_time(&nodes[0]);

        // TODO: Uncomment this case after proposed/pending pool tip verfiry logic changing
        // self.test_since_and_proposal(&nodes[1]);
    }

    fn modify_chain_spec(&self) -> Box<dyn Fn(&mut ChainSpec) -> ()> {
        let cellbase_maturity = self.cellbase_maturity();
        Box::new(move |spec_config: &mut ChainSpec| {
            spec_config.params.cellbase_maturity = cellbase_maturity;
        })
    }
}

impl ValidSince {
    pub fn cellbase_maturity(&self) -> u64 {
        DEFAULT_TX_PROPOSAL_WINDOW.0 + 2
    }

    // (current, current+cellbase_maturity): Err(InvalidTx(CellbaseImmaturity))
    // [current+cellbase_maturity, current+relative_number): Err(InvalidTx(Immature))
    pub fn test_since_relative_block_number(&self, node: &Node) {
        node.mine_block();
        let relative: BlockNumber = self.cellbase_maturity() + 5;
        let since = since_from_relative_block_number(relative);
        let transaction = spend_tip_cellbase(node, since);

        // Failed to send transaction since CellbaseImmaturity
        for _ in 1..self.cellbase_maturity() {
            assert_send_transaction_fail(node, &transaction, "InvalidTx(CellbaseImmaturity)");
            node.mine_block();
        }

        // Failed to send transaction since SinceImmaturity
        for _ in self.cellbase_maturity()..relative {
            assert_send_transaction_fail(node, &transaction, "InvalidTx(Immature)");
            node.mine_block();
        }

        // Success to send transaction after cellbase immaturity and since immaturity
        assert!(
            node.rpc_client()
                .inner()
                .send_transaction((&transaction).into())
                .call()
                .is_ok(),
            "transaction is ok, tip is equal to relative since block number",
        );
    }

    // (current, current+cellbase_maturity): Err(InvalidTx(CellbaseImmaturity))
    // [current+cellbase_maturity, absolute_number): Err(InvalidTx(Immature))
    pub fn test_since_absolute_block_number(&self, node: &Node) {
        node.mine_block();
        let absolute: BlockNumber = node.tip_number() + self.cellbase_maturity() + 5;
        let since = since_from_absolute_block_number(absolute);
        let transaction = spend_tip_cellbase(node, since);

        // Failed to send transaction since CellbaseImmaturity
        for _ in 1..self.cellbase_maturity() {
            assert_send_transaction_fail(node, &transaction, "InvalidTx(CellbaseImmaturity)");
            node.mine_block();
        }

        // Failed to send transaction since SinceImmaturity
        let tip_number = node.tip_number();
        for _ in tip_number + 1..absolute {
            assert_send_transaction_fail(node, &transaction, "InvalidTx(Immature)");
            node.mine_block();
        }

        // Success to send transaction after cellbase immaturity and since immaturity
        assert!(
            node.rpc_client()
                .inner()
                .send_transaction((&transaction).into())
                .call()
                .is_ok(),
            "transaction is ok, tip is equal to absolute since block number",
        );
    }

    pub fn test_since_relative_median_time(&self, node: &Node) {
        node.mine_block();
        let cellbase = node.tip_block().transactions()[0].clone();
        let old_median_time = node.get_blockchain_info().median_time.0;
        sleep(Duration::from_secs(2));

        let n = max(self.cellbase_maturity(), MEDIAN_TIME_BLOCK_COUNT);
        (0..n).for_each(|_| {
            node.mine_block();
        });

        // Calculate the current block median time
        let tip_number = node.tip_number();
        let mut timestamps: Vec<u64> = (tip_number - MEDIAN_TIME_BLOCK_COUNT + 1..=tip_number)
            .map(|block_number| node.get_block_by_number(block_number).header().timestamp())
            .collect();
        timestamps.sort();
        let median_time = timestamps[timestamps.len() >> 1];

        // Absolute since timestamp in seconds
        let median_time_seconds = (median_time - old_median_time) / 1000;
        {
            let since = since_from_relative_timestamp(median_time_seconds + 1);
            let transaction = spend_with_hash(node, cellbase.hash(), since);
            assert_send_transaction_fail(node, &transaction, "InvalidTx(Immature)");
        }
        {
            let since = since_from_relative_timestamp(median_time_seconds - 1);
            let transaction = spend_with_hash(node, cellbase.hash(), since);
            assert!(
                node.rpc_client()
                    .inner()
                    .send_transaction((&transaction).into())
                    .call()
                    .is_ok(),
                "transaction's since is greater than tip's median time",
            );
        }
    }

    pub fn test_since_absolute_median_time(&self, node: &Node) {
        node.mine_block();
        let cellbase = node.tip_block().transactions()[0].clone();
        let n = max(self.cellbase_maturity(), MEDIAN_TIME_BLOCK_COUNT);
        (0..n).for_each(|_| {
            node.mine_block();
        });

        // Calculate current block median time
        let tip_number = node.tip_number();
        let mut timestamps: Vec<u64> = ((tip_number - MEDIAN_TIME_BLOCK_COUNT + 1)..=tip_number)
            .map(|block_number| node.get_block_by_number(block_number).header().timestamp())
            .collect();
        timestamps.sort();
        let median_time = timestamps[timestamps.len() >> 1];

        // Absolute since timestamp in seconds
        let median_time_seconds = median_time / 1000;
        {
            let since = since_from_absolute_timestamp(median_time_seconds + 1);
            let transaction = spend_with_hash(node, cellbase.hash(), since);
            assert_send_transaction_fail(node, &transaction, "InvalidTx(Immature)");
        }
        {
            let since = since_from_absolute_timestamp(median_time_seconds - 1);
            let transaction = spend_with_hash(node, cellbase.hash(), since);
            assert!(
                node.rpc_client()
                    .inner()
                    .send_transaction((&transaction).into())
                    .call()
                    .is_ok(),
                "transaction's since is greater than tip's median time",
            );
        }
    }

    #[allow(clippy::identity_op)]
    pub fn test_since_and_proposal(&self, node: &Node) {
        node.mine_block();

        // test relative block number since
        info!("Use tip block cellbase as tx input with a relative block number since");
        let relative_blocks: BlockNumber = 5;
        let since = (0b1000_0000 << 56) + relative_blocks;
        let tx = spend_tip_cellbase(node, since);

        (0..relative_blocks - DEFAULT_TX_PROPOSAL_WINDOW.0).for_each(|i| {
            info!("Tx is immature in block N + {}", i);
            let error = node.send_transaction(&tx);
            assert_regex_match(&error.to_string(), r"InvalidTx\(Immature\)");
            node.mine_block();
        });

        info!(
            "Tx will be added to pending pool in N + {} block",
            relative_blocks - DEFAULT_TX_PROPOSAL_WINDOW.0
        );
        let tx_hash = node.send_transaction(&tx);
        assert_eq!(tx_hash, tx.hash().to_owned());
        assert_tx_pool_size(node, 1, 0);

        info!(
            "Tx will be added to proposed pool in N + {} block",
            relative_blocks
        );
        (0..DEFAULT_TX_PROPOSAL_WINDOW.0).for_each(|_| {
            node.mine_block();
        });
        assert_tx_pool_size(node, 0, 1);

        node.mine_block();
        assert_tx_pool_size(node, 0, 0);

        // test absolute block number since
        let tip_number: BlockNumber = node.tip_number();
        info!(
            "Use tip block {} cellbase as tx input with an absolute block number since",
            tip_number
        );
        let absolute_block: BlockNumber = 10;
        let since = (0b0000_0000 << 56) + absolute_block;
        let tx = spend_tip_cellbase(node, since);

        (tip_number..absolute_block - DEFAULT_TX_PROPOSAL_WINDOW.0).for_each(|i| {
            info!("Tx is immature in block {}", i);
            let error = node.send_transaction(&tx);
            assert_regex_match(&error.to_string(), r"InvalidTx\(Immature\)");
            node.mine_block();
        });

        info!(
            "Tx will be added to pending pool in {} block",
            absolute_block - DEFAULT_TX_PROPOSAL_WINDOW.0
        );
        let tx_hash = node.send_transaction(&tx);
        assert_eq!(tx_hash, tx.hash().to_owned());
        assert_tx_pool_size(node, 1, 0);

        info!(
            "Tx will be added to proposed pool in {} block",
            absolute_block
        );
        (0..DEFAULT_TX_PROPOSAL_WINDOW.0).for_each(|_| {
            node.mine_block();
        });
        assert_tx_pool_size(node, 0, 1);

        node.mine_block();
        assert_tx_pool_size(node, 0, 0);
    }
}

fn spend_tip_cellbase(node: &Node, since: u64) -> Transaction {
    let out_point = node.tip_cellbase_out_point();
    let cell_input = CellInput::new(out_point, since);
    node.build_transaction(cell_input)
}

fn spend_with_hash(node: &Node, tx_hash: &H256, since: u64) -> Transaction {
    let out_point = OutPoint::new(tx_hash.to_owned(), 0);
    let cell_input = CellInput::new(out_point, since);
    node.build_transaction(cell_input)
}
