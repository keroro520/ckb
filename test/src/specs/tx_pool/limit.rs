use crate::utils::assert_tx_pool_statics;
use crate::{assert_regex_match, Net, Node, Spec, DEFAULT_TX_PROPOSAL_WINDOW};
use ckb_app_config::CKBAppConfig;
use log::info;

pub struct SizeLimit;

impl Spec for SizeLimit {
    crate::name!("size_limit");

    fn run(&self, _net: Net, nodes: Vec<Node>) {
        let node = &nodes[0];

        info!("Generate 1 block on node");
        node.mine_block();

        info!("Generate 5 txs on node");
        let mut txs_hash = Vec::new();
        let mut hash = node.send_transaction_with_tip_cellbase();
        txs_hash.push(hash.clone());

        (0..4).for_each(|_| {
            let tx = node.build_transaction_with_hash(hash.clone());
            info!("tx.size: {}", tx.serialized_size());
            hash = node.send_transaction(&tx);
            txs_hash.push(hash.clone());
        });

        info!("No.6 tx reach size limit");
        let tx = node.build_transaction_with_hash(hash.clone());

        let error = node.send_transaction_result(&tx).unwrap_err();
        assert_regex_match(&error.to_string(), r"LimitReached");

        // 186 * 5
        // 12 * 5
        assert_tx_pool_statics(node, 930, 60);
        (0..DEFAULT_TX_PROPOSAL_WINDOW.0).for_each(|_| {
            node.mine_block();
        });
        node.mine_block();
        assert_tx_pool_statics(node, 0, 0);
    }

    fn modify_ckb_config(&self) -> Box<dyn Fn(&mut CKBAppConfig) -> ()> {
        Box::new(|config| {
            config.tx_pool.max_mem_size = 930;
            config.tx_pool.max_cycles = 200_000_000_000;
        })
    }
}

pub struct CyclesLimit;

impl Spec for CyclesLimit {
    crate::name!("cycles_limit");

    fn run(&self, _net: Net, nodes: Vec<Node>) {
        let node = &nodes[0];

        info!("Generate 1 block on node");
        node.mine_block();

        info!("Generate 5 txs on node");
        let mut txs_hash = Vec::new();
        let mut hash = node.send_transaction_with_tip_cellbase();
        txs_hash.push(hash.clone());

        (0..4).for_each(|_| {
            let tx = node.build_transaction_with_hash(hash.clone());
            info!("tx.size: {}", tx.serialized_size());
            hash = node.send_transaction(&tx);
            txs_hash.push(hash.clone());
        });

        info!("No.6 tx reach cycles limit");
        let tx = node.build_transaction_with_hash(hash.clone());

        let error = node.send_transaction_result(&tx).unwrap_err();
        assert_regex_match(&error.to_string(), r"LimitReached");

        // 186 * 5
        // 12 * 5
        assert_tx_pool_statics(node, 930, 60);
        (0..DEFAULT_TX_PROPOSAL_WINDOW.0).for_each(|_| {
            node.mine_block();
        });
        node.mine_block();
        assert_tx_pool_statics(node, 0, 0);
    }

    fn modify_ckb_config(&self) -> Box<dyn Fn(&mut CKBAppConfig) -> ()> {
        Box::new(|config| {
            config.tx_pool.max_mem_size = 20_000_000;
            config.tx_pool.max_cycles = 60;
        })
    }
}
