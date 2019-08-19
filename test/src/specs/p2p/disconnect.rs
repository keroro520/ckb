use crate::utils::wait_until;
use crate::{Net, Node, Spec};

pub struct Disconnect;

impl Spec for Disconnect {
    crate::name!("disconnect");

    crate::setup!(num_nodes: 2);

    fn run(&self, _net: Net, mut nodes: Vec<Node>) {
        let node1 = nodes.pop().unwrap();
        std::mem::drop(node1);

        let ret = wait_until(10, || nodes[0].get_peers().is_empty());
        assert!(
            ret,
            "The address of node1 should be removed from node0's peers",
        )
    }
}
