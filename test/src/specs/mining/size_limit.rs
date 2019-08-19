use crate::{Net, Spec};
use log::info;

pub struct TemplateSizeLimit;

impl Spec for TemplateSizeLimit {
    crate::name!("template_size_limit");

    fn run(&self, net: Net) {
        let node = &net.nodes[0];

        info!("Generate 1 block");
        node.mine_block();

        info!("Generate 6 txs");
        let mut txs_hash = Vec::new();
        let mut hash = node.send_transaction_with_tip_cellbase();
        txs_hash.push(hash.clone());

        (0..5).for_each(|_| {
            let tx = node.build_transaction_with_hash(hash.clone());
            hash = node.send_transaction(&tx);
            txs_hash.push(hash.clone());
        });

        let _ = node.mine_block();
        let _ = node.mine_block(); // skip

        let new_block = node.build_block(None, None, None);
        assert_eq!(new_block.serialized_size(0), 1542);
        assert_eq!(new_block.transactions().len(), 7);

        let new_block = node.build_block(Some(1000), None, None);
        assert_eq!(new_block.transactions().len(), 4);
    }
}
