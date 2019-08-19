use crate::utils::wait_until;
use crate::{Net, Node, Spec};
use log::info;
use std::{thread::sleep, time::Duration};

pub struct IBDProcess;

impl Spec for IBDProcess {
    crate::name!("ibd_process");

    crate::setup!(num_nodes: 7, connect_all: false);

    fn run(&self, _net: Net, nodes: Vec<Node>) {
        info!("Running IBD process");

        let node0 = &nodes[0];
        let node1 = &nodes[1];
        let node2 = &nodes[2];
        let node3 = &nodes[3];
        let node4 = &nodes[4];
        let node5 = &nodes[5];
        let node6 = &nodes[6];

        node0.connect(node1);
        node0.connect(node2);
        node0.connect(node3);
        node0.connect(node4);
        // will never connect
        node0.connect_uncheck(node5);
        node0.connect_uncheck(node6);
        node0.mine_blocks(1);

        sleep(Duration::from_secs(5));

        let rpc_client = node0;
        let ret = wait_until(10, || {
            let peers = rpc_client.get_peers();
            peers.len() == 4
        });

        if !ret {
            panic!("refuse to connect fail");
        }
    }
}
