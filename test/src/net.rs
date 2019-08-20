use crate::utils::temp_path;
use crate::{Node, Setup};
use bytes::Bytes;
use ckb_network::{
    CKBProtocol, CKBProtocolContext, CKBProtocolHandler, NetworkConfig, NetworkController,
    NetworkService, NetworkState, PeerIndex, ProtocolId,
};
use crossbeam_channel::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::Arc;
use std::time::Duration;

pub type NetMessage = (PeerIndex, ProtocolId, Bytes);

pub struct Net {
    setup: Setup,
    start_port: u16,
    working_dir: String,
    controller: Option<(NetworkController, Receiver<NetMessage>)>,
}

impl Net {
    pub fn new(setup: Setup, start_port: u16) -> Self {
        Self {
            start_port,
            setup,
            working_dir: temp_path(),
            controller: None,
        }
    }

    pub fn controller(&self) -> &(NetworkController, Receiver<NetMessage>) {
        self.controller.as_ref().unwrap()
    }

    pub fn working_dir(&self) -> &str {
        &self.working_dir
    }

    fn init_controller(&self, node: &Node) {
        assert!(
            !self.setup.protocols.is_empty(),
            "It should not involve Net::init_controller when setup::test_protocols is empty.\
             Please specify non-empty setup::test_protocols first"
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
            path: self.working_dir().into(),
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
        )
    }

    /// Blocks the current thread until a message is sent or panic if disconnected
    pub fn send(&self, protocol_id: ProtocolId, peer: PeerIndex, data: Bytes) {
        self.controller()
            .0
            .send_message_to(peer, protocol_id, data)
            .expect("Send message to p2p network failed");
    }

    /// Blocks the current thread until a message is received or panic if disconnected.
    pub fn recv(&self) -> NetMessage {
        self.controller()
            .1
            .recv()
            .expect("Receive message from p2p network failed")
    }

    /// Waits for a message to be received from the channel, but only for a limited time.
    pub fn recv_timeout(&self, timeout: Duration) -> Result<NetMessage, RecvTimeoutError> {
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
