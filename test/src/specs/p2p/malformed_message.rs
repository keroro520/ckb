use crate::utils::wait_until;
use crate::{Net, Spec, TestProtocol};
use ckb_protocol::{get_root, SyncMessage, SyncPayload};
use ckb_sync::NetworkProtocol;
use log::info;

pub struct MalformedMessage;

impl Spec for MalformedMessage {
    crate::name!("malformed_message");

    crate::setup!(protocols: vec![TestProtocol::sync()]);

    fn run(&self, net: Net) {
        info!("Connect node0");
        let node0 = &net.nodes[0];
        net.exit_ibd_mode();
        net.connect(node0);

        info!("Test node should receive GetHeaders message from node0");
        let (peer_id, _, data) = net.receive();
        let msg = get_root::<SyncMessage>(&data).expect("parse message failed");
        assert_eq!(SyncPayload::GetHeaders, msg.payload_type());

        info!("Send malformed message to node0 twice");
        net.send(
            NetworkProtocol::SYNC.into(),
            peer_id,
            vec![0, 0, 0, 0].into(),
        );
        net.send(
            NetworkProtocol::SYNC.into(),
            peer_id,
            vec![0, 1, 2, 3].into(),
        );
        let ret = wait_until(10, || net.nodes[0].get_peers().is_empty());
        assert!(ret, "Node0 should disconnect test node");
        let ret = wait_until(10, || {
            net.nodes[0]
                .get_banned_addresses()
                .iter()
                .any(|ban| ban.address == "127.0.0.1/32")
        });
        assert!(ret, "Node0 should ban test node");
    }
}
