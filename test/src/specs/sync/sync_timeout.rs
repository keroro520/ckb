use crate::utils::{disconnect_all, waiting_for_sync, waiting_for_sync2};
use crate::{Net, Spec};
use log::info;

pub struct SyncTimeout;

impl Spec for SyncTimeout {
    crate::name!("sync_timeout");

    crate::setup!(num_nodes: 5, connect_all: false);

    fn run(&self, net: Net) {
        let node0 = &net.nodes[0];
        let node1 = &net.nodes[1];
        let node2 = &net.nodes[2];
        let node3 = &net.nodes[3];
        let node4 = &net.nodes[4];

        info!("Generate 2 blocks on node0");
        node0.generate_blocks(2);

        info!("Connect all nodes");
        node1.connect(node0);
        node2.connect(node0);
        node3.connect(node0);
        node4.connect(node0);
        waiting_for_sync(&net.nodes, 2);

        info!("Disconnect all nodes");
        disconnect_all(&net.nodes);

        info!("Generate 200 blocks on node0");
        node0.generate_blocks(200);

        node0.connect(node1);
        info!("Waiting for node0 and node1 sync");
        waiting_for_sync2(node0, node1, 202);

        info!("Generate 200 blocks on node1");
        node1.generate_blocks(200);

        node2.connect(node0);
        node2.connect(node1);
        node3.connect(node0);
        node3.connect(node1);
        node4.connect(node0);
        node4.connect(node1);
        info!("Waiting for all nodes sync");
        waiting_for_sync(&net.nodes, 402);
    }
}
