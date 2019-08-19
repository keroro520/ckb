use crate::utils::waiting_for_sync;
use crate::{Net, Node, Spec};
use log::info;

pub struct PoolReconcile;

impl Spec for PoolReconcile {
    crate::name!("pool_reconcile");

    crate::setup!(connect_all: false, num_nodes: 2);

    fn run(&self, _net: Net, nodes: Vec<Node>) {
        let node0 = &nodes[0];
        let node1 = &nodes[1];

        info!("Generate 1 block on node0");
        node0.mine_block();

        info!("Use generated block's cellbase as tx input");
        let hash = node0.send_transaction_with_tip_cellbase();

        info!("Generate 3 more blocks on node0");
        node0.mine_blocks(3);

        info!("Pool should be empty");
        assert!(node0
            .get_transaction(hash.clone())
            .unwrap()
            .tx_status
            .block_hash
            .is_some());

        info!("Generate 5 blocks on node1");
        node1.mine_blocks(5);

        info!("Connect node0 to node1");
        node0.connect(node1);

        waiting_for_sync(&nodes, 5);

        info!("Tx should be re-added to node0's pool");
        assert!(node0
            .get_transaction(hash.clone())
            .unwrap()
            .tx_status
            .block_hash
            .is_none());
    }
}
