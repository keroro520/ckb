use crate::{Net, Node, Spec};
use ckb_jsonrpc_types::BlockTemplate;
use ckb_types::{core::BlockView, packed::ProposalShortId, prelude::*, H256};
use log::info;
use std::convert::Into;
use std::thread::sleep;
use std::time::Duration;

pub struct MiningBasic;

impl Spec for MiningBasic {
    crate::name!("mining_basic");

    fn run(&self, net: Net) {
        let node = &net.nodes[0];

        self.test_basic(node);
        self.test_block_template_cache(node);
    }
}

impl MiningBasic {
    pub const BLOCK_TEMPLATE_TIMEOUT: u64 = 3;

    pub fn test_basic(&self, node: &Node) {
        node.generate_block();
        info!("Use generated block's cellbase as tx input");
        let transaction_hash = node.generate_transaction();
        let block1_hash = node.generate_block();
        let _ = node.generate_block(); // skip
        let block3_hash = node.generate_block();

        let block1: BlockView = node.get_block(block1_hash).unwrap().into();
        let block3: BlockView = node.get_block(block3_hash).unwrap().into();

        info!("Generated tx should be included in next block's proposal txs");
        assert!(block1
            .union_proposal_ids_iter()
            .any(|id| ProposalShortId::from_tx_hash(&transaction_hash).eq(&id)));

        info!("Generated tx should be included in next + n block's commit txs, current n = 2");
        assert!(block3
            .transactions()
            .into_iter()
            .any(|tx| transaction_hash.eq(&tx.hash().unpack())));
    }

    pub fn test_block_template_cache(&self, node: &Node) {
        let mut block1 = node.new_block(None, None, None);
        sleep(Duration::new(Self::BLOCK_TEMPLATE_TIMEOUT + 1, 0)); // Wait block timeout cache timeout
        let mut block2 = node
            .new_block_builder(None, None, None)
            .header(
                block1
                    .header()
                    .to_owned()
                    .as_advanced_builder()
                    .timestamp((block1.header().timestamp() + 1).pack())
                    .build(),
            )
            .build();
        assert_ne!(block1.header().timestamp(), block2.header().timestamp());

        // Expect block1.hash() > block2.hash(), so that when we submit block2 after block1,
        // block2 will replace block1 as tip block
        let block_hash1: H256 = block1.header().hash().unpack();
        let block_hash2: H256 = block2.header().hash().unpack();
        if block_hash1 < block_hash2 {
            std::mem::swap(&mut block1, &mut block2);
        }
        let block_hash1: H256 = block1.header().hash().unpack();
        assert_eq!(block_hash1, node.submit_block(&block1.data()));
        assert_eq!(block_hash1, node.get_tip_header().hash);

        let template1 = node.get_block_template(None, None, None);
        sleep(Duration::new(0, 200));
        let template2 = node.get_block_template(None, None, None);
        assert_eq!(block_hash1, template1.parent_hash);
        assert!(
            is_block_template_equal(&template1, &template2),
            "templates keep same since block template cache",
        );

        let block_hash2: H256 = block2.header().hash().clone().unpack();
        assert_eq!(block_hash2, node.submit_block(&block2.data()));
        assert_eq!(block_hash2, node.get_tip_header().hash);
        let template3 = node.get_block_template(None, None, None);
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
