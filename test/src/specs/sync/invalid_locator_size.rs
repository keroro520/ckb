use crate::utils::{exit_ibd_mode, wait_until};
use crate::{Net, Node, Spec, TestProtocol};
use ckb_protocol::SyncMessage;
use ckb_sync::{NetworkProtocol, MAX_LOCATOR_SIZE};
use flatbuffers::FlatBufferBuilder;
use log::info;
use numext_fixed_hash::{h256, H256};

pub struct InvalidLocatorSize;

impl Spec for InvalidLocatorSize {
    crate::name!("invalid_locator_size");

    crate::setup!(protocols: vec![TestProtocol::sync()]);

    fn run(&self, net: Net, nodes: Vec<Node>) {
        info!("Connect node0");
        exit_ibd_mode(&nodes);
        let node0 = &nodes[0];
        net.connect(node0);
        // get peer_id from GetHeaders message
        let (peer_id, _, _) = net.recv();

        let hashes: Vec<_> = (0..=MAX_LOCATOR_SIZE).map(|_| h256!("0x1")).collect();
        let fbb = &mut FlatBufferBuilder::new();
        let message = SyncMessage::build_get_headers(fbb, &hashes);
        fbb.finish(message, None);
        net.send(
            NetworkProtocol::SYNC.into(),
            peer_id,
            fbb.finished_data().into(),
        );

        let ret = wait_until(10, || nodes[0].get_peers().is_empty());
        assert!(ret, "Node0 should disconnect test node");

        net.connect(node0);
        let ret = wait_until(10, || !nodes[0].get_peers().is_empty());
        assert!(!ret, "Node0 should ban test node");
    }
}
