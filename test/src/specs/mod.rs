mod alert;
mod indexer;
mod mining;
mod p2p;
mod relay;
mod sync;
mod tx_pool;

pub use alert::*;
pub use indexer::*;
pub use mining::*;
pub use p2p::*;
pub use relay::*;
pub use sync::*;
pub use tx_pool::*;

use crate::utils::connect_all;
use crate::{Net, Node};
use ckb_app_config::CKBAppConfig;
use ckb_chain_spec::ChainSpec;
use ckb_network::{ProtocolId, ProtocolVersion};
use ckb_sync::NetworkProtocol;

#[macro_export]
macro_rules! name {
    ($name:literal) => {
        fn name(&self) -> &'static str { $name }
    };
}

#[macro_export]
macro_rules! setup {
    ($($setup:tt)*) => {
        fn setup(&self) -> $crate::Setup{ crate::setup_internal!($($setup)*) }
    };
}

#[macro_export]
macro_rules! setup_internal {
    ($field:ident: $value:expr,) => {
        crate::setup_internal!($field: $value)
    };
    ($field:ident: $value:expr) => {
        $crate::Setup{ $field: $value, ..Default::default() }
    };
    ($field:ident: $value:expr, $($rest:tt)*) =>  {
        $crate::Setup{ $field: $value, ..crate::setup_internal!($($rest)*) }
    };
}

#[derive(Clone)]
pub struct Setup {
    pub num_nodes: usize,
    pub connect_all: bool,
    pub protocols: Vec<TestProtocol>,
}

impl Default for Setup {
    fn default() -> Self {
        Setup {
            num_nodes: 1,
            connect_all: true,
            protocols: vec![],
        }
    }
}

pub trait Spec {
    fn name(&self) -> &'static str;

    fn setup(&self) -> Setup {
        Setup::default()
    }

    fn modify_chain_spec(&self) -> Box<dyn Fn(&mut ChainSpec) -> ()> {
        Box::new(|_| ())
    }

    fn modify_ckb_config(&self) -> Box<dyn Fn(&mut CKBAppConfig) -> ()> {
        // disable outbound peer service
        Box::new(|config| {
            config.network.connect_outbound_interval_secs = 0;
            config.network.discovery_local_address = true;
        })
    }

    fn run(&self, net: Net, nodes: Vec<Node>);

    fn prepare(&self, binary: &str, start_port: u16) -> (Net, Vec<Node>) {
        // Start nodes
        let mut nodes: Vec<Node> = (0..self.setup().num_nodes)
            .map(|n| {
                Node::new(
                    binary,
                    start_port + (n * 2 + 1) as u16,
                    start_port + (n * 2 + 2) as u16,
                )
            })
            .collect();
        nodes.iter_mut().for_each(|node| {
            node.start(self.modify_chain_spec(), self.modify_ckb_config());
        });

        // Start net
        let net = Net::new(self.setup(), start_port);

        // TODO FIXME bilibili
        // connect the nodes as a linear chain: node0 <-> node1 <-> node2 <-> ...
        if self.setup().connect_all {
            connect_all(&nodes);
        }

        (net, nodes)
    }
}

#[derive(Clone)]
pub struct TestProtocol {
    pub id: ProtocolId,
    pub protocol_name: String,
    pub supported_versions: Vec<ProtocolVersion>,
}

impl TestProtocol {
    pub fn sync() -> Self {
        Self {
            id: NetworkProtocol::SYNC.into(),
            protocol_name: "syn".to_string(),
            supported_versions: vec!["1".to_string()],
        }
    }

    pub fn relay() -> Self {
        Self {
            id: NetworkProtocol::RELAY.into(),
            protocol_name: "rel".to_string(),
            supported_versions: vec!["1".to_string()],
        }
    }
}
