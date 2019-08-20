use crate::{Net, Node, Spec};
use ckb_core::header::HeaderBuilder;
use ckb_core::transaction::ProposalShortId;
use ckb_jsonrpc_types::BlockTemplate;
use log::info;
use std::thread::sleep;
use std::time::Duration;

pub struct MiningBasic;

impl Spec for MiningBasic {
    crate::name!("mining_basic");

    fn run(&self, _net: Net, nodes: Vec<Node>) {
        let node = &nodes[0];

        self.test_basic(node);
        self.test_block_template_cache(node);
    }
}

impl MiningBasic {
    pub const BLOCK_TEMPLATE_TIMEOUT: u64 = 3;

    pub fn test_basic(&self, node: &Node) {
        node.mine_block();
        info!("Use generated block's cellbase as tx input");
        let transaction_hash = node.send_transaction_with_tip_cellbase();
        let block1_hash = node.mine_block();
        let _ = node.mine_block(); // skip
        let block3_hash = node.mine_block();

        let block1 = node.get_block(block1_hash).unwrap();
        let block3 = node.get_block(block3_hash).unwrap();

        info!("Generated tx should be included in next block's proposal txs");
        assert!(block1
            .proposals()
            .iter()
            .any(|id| ProposalShortId::from_tx_hash(&transaction_hash).eq(id)));

        info!("Generated tx should be included in next + n block's commit txs, current n = 2");
        assert!(block3
            .transactions()
            .iter()
            .any(|tx| transaction_hash.eq(&tx.hash())));
    }

    pub fn test_block_template_cache(&self, node: &Node) {
        let mut block1 = node.build_block(None, None, None);
        sleep(Duration::new(Self::BLOCK_TEMPLATE_TIMEOUT + 1, 0)); // Wait block timeout cache timeout
        let mut block2 = node
            .build_block_builder(None, None, None)
            .header_builder(
                HeaderBuilder::from_header(block1.header().to_owned())
                    .timestamp(block1.header().timestamp() + 1),
            )
            .build();
        assert_ne!(block1.header().timestamp(), block2.header().timestamp());

        // Expect block1.hash() > block2.hash(), so that when we submit block2 after block1,
        // block2 will replace block1 as tip block
        if block1.header().hash() < block2.header().hash() {
            std::mem::swap(&mut block1, &mut block2);
        }

        let rpc_client = node;
        let block_hash1 = block1.header().hash().clone();
        assert_eq!(block_hash1, node.submit_block(&block1));
        assert_eq!(&block_hash1, rpc_client.get_tip_header().hash());

        let template1 = rpc_client.get_block_template(None, None, None);
        sleep(Duration::new(0, 200));
        let template2 = rpc_client.get_block_template(None, None, None);
        assert_eq!(block_hash1, template1.parent_hash);
        assert!(
            is_block_template_equal(&template1, &template2),
            "templates keep same since block template cache",
        );

        let block_hash2 = block2.header().hash().clone();
        assert_eq!(block_hash2, node.submit_block(&block2));
        assert_eq!(&block_hash2, rpc_client.get_tip_header().hash());
        let template3 = rpc_client.get_block_template(None, None, None);
        assert_eq!(block_hash2, template3.parent_hash);
        assert!(
            template3.current_time.0 > template1.current_time.0,
            "New tip block, new template",
        );
    }
}

fn is_block_template_equal(template1: &BlockTemplate, template2: &BlockTemplate) -> bool {
    let mut temp = template1.clone();
    temp.current_time = template2.current_time.clone();
    &temp == template2
}
