use crate::utils::{exit_ibd_mode, wait_until};
use crate::{Net, Node, Spec};
use log::info;

pub struct BlockRelayBasic;

impl Spec for BlockRelayBasic {
    crate::name!("block_relay_basic");

    crate::setup!(num_nodes: 3);

    fn run(&self, _net: Net, nodes: Vec<Node>) {
        exit_ibd_mode(&nodes);
        let node0 = &nodes[0];
        let node1 = &nodes[1];
        let node2 = &nodes[2];

        info!("Generate new block on node1");
        let hash = node1.mine_block();

        let rpc_client = node0;
        let ret = wait_until(10, || rpc_client.get_block(hash.clone()).is_some());
        assert!(ret, "Block should be relayed to node0");

        let rpc_client = node2;
        let ret = wait_until(10, || rpc_client.get_block(hash.clone()).is_some());
        assert!(ret, "Block should be relayed to node2");
    }
}
