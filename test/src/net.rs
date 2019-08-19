use crate::utils::wait_until;
use crate::{Node, Setup};
use bytes::Bytes;
use ckb_core::{block::Block, BlockNumber};
use ckb_network::{
    CKBProtocol, CKBProtocolContext, CKBProtocolHandler, NetworkConfig, NetworkController,
    NetworkService, NetworkState, PeerIndex, ProtocolId,
};
use crossbeam_channel::{self, Receiver, RecvTimeoutError, Sender};
use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;
use tempfile::tempdir;

pub type NetMessage = (PeerIndex, ProtocolId, Bytes);

pub struct Net {
    pub nodes: Vec<Node>,
    controller: Option<(NetworkController, Receiver<NetMessage>)>,
    setup: Setup,
    start_port: u16,
}

impl Net {
    pub fn new(binary: &str, setup: Setup, start_port: u16) -> Self {
        let nodes: Vec<Node> = (0..setup.num_nodes)
            .map(|n| {
                Node::new(
                    binary,
                    tempdir()
                        .expect("create tempdir failed")
                        .path()
                        .to_str()
                        .unwrap(),
                    start_port + (n * 2 + 1) as u16,
                    start_port + (n * 2 + 2) as u16,
                )
            })
            .collect();

        Self {
            nodes,
            controller: None,
            start_port,
            setup,
        }
    }

    pub fn controller(&self) -> &(NetworkController, Receiver<NetMessage>) {
        self.controller.as_ref().unwrap()
    }

    fn init_controller(&self, node: &Node) {
        assert!(
            !self.setup.protocols.is_empty(),
            "It should not involve Net::init_controller when setup test_protocols is empty.\
             Please specify non-empty test_protocols"
        );

        let (tx, rx) = crossbeam_channel::unbounded();
        let config = NetworkConfig {
            listen_addresses: vec![format!("/ip4/127.0.0.1/tcp/{}", self.start_port)
                .parse()
                .expect("invalid address")],
            public_addresses: vec![],
            bootnodes: vec![],
            dns_seeds: vec![],
            whitelist_peers: vec![],
            whitelist_only: false,
            max_peers: self.setup.num_nodes as u32,
            max_outbound_peers: self.setup.num_nodes as u32,
            path: tempdir()
                .expect("create tempdir failed")
                .path()
                .to_path_buf(),
            ping_interval_secs: 15,
            ping_timeout_secs: 20,
            connect_outbound_interval_secs: 0,
            discovery_local_address: true,
            upnp: false,
            bootnode_mode: false,
            max_send_buffer: None,
        };

        let network_state =
            Arc::new(NetworkState::from_config(config).expect("Init network state failed"));

        let protocols = self
            .setup
            .protocols
            .clone()
            .into_iter()
            .map(|tp| {
                let tx = tx.clone();
                CKBProtocol::new(
                    tp.protocol_name,
                    tp.id,
                    &tp.supported_versions,
                    move || Box::new(DummyProtocolHandler { tx: tx.clone() }),
                    Arc::clone(&network_state),
                )
            })
            .collect();

        let controller = Some((
            NetworkService::new(
                Arc::clone(&network_state),
                protocols,
                node.consensus().identify_name(),
                "0.1.0".to_string(),
            )
            .start(Default::default(), Some("NetworkService"))
            .expect("Start network service failed"),
            rx,
        ));

        let ptr = self as *const Self as *mut Self;
        unsafe {
            ::std::mem::replace(&mut (*ptr).controller, controller);
        }
    }

    pub fn connect(&self, node: &Node) {
        if self.controller.is_none() {
            self.init_controller(node);
        }

        let node_info = node.local_node_info();
        self.controller().0.add_node(
            &node_info.node_id.parse().expect("invalid peer_id"),
            format!("/ip4/127.0.0.1/tcp/{}", node.p2p_port())
                .parse()
                .expect("invalid address"),
        );
    }

    pub fn connect_all(&self) {
        self.nodes
            .windows(2)
            .for_each(|nodes| nodes[0].connect(&nodes[1]));
    }

    pub fn disconnect_all(&self) {
        self.nodes.iter().for_each(|node_a| {
            self.nodes.iter().for_each(|node_b| {
                if node_a.node_id() != node_b.node_id() {
                    node_a.disconnect(node_b)
                }
            })
        });
    }

    // generate a same block on all nodes, exit IBD mode and return the tip block
    pub fn exit_ibd_mode(&self) -> Block {
        let block = self.nodes[0].build_block(None, None, None);
        self.nodes.iter().for_each(|node| {
            node.submit_block(&block);
        });
        block
    }

    pub fn waiting_for_sync(&self, target: BlockNumber) {
        let rpc_clients: Vec<_> = self.nodes.iter().map(Node::rpc_client).collect();
        let mut tip_numbers: HashSet<BlockNumber> = HashSet::with_capacity(self.nodes.len());
        // 60 seconds is a reasonable timeout to sync, even for poor CI server
        let result = wait_until(60, || {
            tip_numbers = rpc_clients
                .iter()
                .map(|rpc_client| rpc_client.get_tip_block_number())
                .collect();
            tip_numbers.len() == 1 && tip_numbers.iter().next().cloned().unwrap() == target
        });

        if !result {
            panic!("timeout to wait for sync, tip_numbers: {:?}", tip_numbers);
        }
    }

    pub fn send(&self, protocol_id: ProtocolId, peer: PeerIndex, data: Bytes) {
        self.controller()
            .0
            .send_message_to(peer, protocol_id, data)
            .expect("Send message to p2p network failed");
    }

    pub fn receive(&self) -> NetMessage {
        self.controller().1.recv().unwrap()
    }

    pub fn receive_timeout(&self, timeout: Duration) -> Result<NetMessage, RecvTimeoutError> {
        self.controller().1.recv_timeout(timeout)
    }
}

pub struct DummyProtocolHandler {
    tx: Sender<NetMessage>,
}

impl CKBProtocolHandler for DummyProtocolHandler {
    fn init(&mut self, _nc: Arc<dyn CKBProtocolContext + Sync>) {}

    fn received(
        &mut self,
        nc: Arc<dyn CKBProtocolContext + Sync>,
        peer_index: PeerIndex,
        data: bytes::Bytes,
    ) {
        let _ = self.tx.send((peer_index, nc.protocol_id(), data));
    }
}
