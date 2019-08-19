use crate::utils::assert_tx_pool_size;
use crate::{assert_regex_match, Net, Node, Spec, DEFAULT_TX_PROPOSAL_WINDOW};
use ckb_chain_spec::ChainSpec;
use ckb_core::BlockNumber;
use log::info;

const MATURITY: BlockNumber = 5;

pub struct CellbaseMaturity;

impl Spec for CellbaseMaturity {
    crate::name!("cellbase_maturity");

    fn run(&self, _net: Net, nodes: Vec<Node>) {
        let node = &nodes[0];

        info!("Generate 1 block");
        node.mine_block();

        info!("Use generated block's cellbase as tx input");
        let tip_block = node.tip_block();
        let tx = node.build_transaction_with_hash(tip_block.transactions()[0].hash().to_owned());

        (0..MATURITY - DEFAULT_TX_PROPOSAL_WINDOW.0).for_each(|i| {
            info!("Tx is not maturity in N + {} block", i);
            let error = node.send_transaction(&tx);
            assert_regex_match(&error.to_string(), r"InvalidTx\(CellbaseImmaturity\)");
            node.mine_block();
        });

        info!(
            "Tx will be added to pending pool in N + {} block",
            MATURITY - DEFAULT_TX_PROPOSAL_WINDOW.0
        );
        let tx_hash = node.send_transaction(&tx);
        assert_eq!(tx_hash, tx.hash().to_owned());
        assert_tx_pool_size(node, 1, 0);

        info!(
            "Tx will be added to proposed pool in N + {} block",
            MATURITY
        );
        (0..DEFAULT_TX_PROPOSAL_WINDOW.0).for_each(|_| {
            node.mine_block();
        });

        assert_tx_pool_size(node, 0, 1);
        node.mine_block();
        assert_tx_pool_size(node, 0, 0);
    }

    fn modify_chain_spec(&self) -> Box<dyn Fn(&mut ChainSpec) -> ()> {
        Box::new(|spec_config| {
            spec_config.params.cellbase_maturity = MATURITY;
        })
    }
}
