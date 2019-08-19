use crate::utils::wait_until;
use crate::{Net, Spec};
use log::info;

pub struct IndexerBasic;

impl Spec for IndexerBasic {
    crate::name!("indexer_basic");

    crate::setup!(num_nodes: 2, connect_all: false);

    fn run(&self, net: Net) {
        let node0 = &net.nodes[0];
        let node1 = &net.nodes[1];

        info!("Generate 1 block on node0");
        node0.generate_block();

        let tip_block = node0.get_tip_block();
        let lock_hash = tip_block.transactions()[0].outputs()[0].lock.hash();
        let rpc_client = node0;

        info!("Should return empty result before index the lock hash");
        let live_cells = rpc_client.get_live_cells_by_lock_hash(lock_hash.clone(), 0, 10, None);
        let cell_transactions =
            rpc_client.get_transactions_by_lock_hash(lock_hash.clone(), 0, 10, None);
        assert_eq!(0, live_cells.len());
        assert_eq!(0, cell_transactions.len());

        info!("Live cells size should be 1, cell transactions size should be 1");
        rpc_client.index_lock_hash(lock_hash.clone(), Some(0));
        let result = wait_until(5, || {
            let live_cells = rpc_client.get_live_cells_by_lock_hash(lock_hash.clone(), 0, 20, None);
            let cell_transactions =
                rpc_client.get_transactions_by_lock_hash(lock_hash.clone(), 0, 20, None);
            live_cells.len() == 1 && cell_transactions.len() == 1
        });
        if !result {
            panic!("Wrong indexer store index data");
        }

        info!("Generate 6 txs on node0");
        let mut txs_hash = Vec::new();
        let mut hash = node0.generate_transaction();
        txs_hash.push(hash.clone());

        (0..5).for_each(|_| {
            let tx = node0.new_transaction(hash.clone());
            hash = rpc_client.send_transaction(&tx);
            txs_hash.push(hash.clone());
        });

        info!("Generate 3 more blocks on node0 to commit 6 txs");
        node0.generate_blocks(3);
        info!(
            "Live cells size should be 4 (1 + 3), cell transactions size should be 10 (1 + 6 + 3)"
        );
        let result = wait_until(5, || {
            let live_cells = rpc_client.get_live_cells_by_lock_hash(lock_hash.clone(), 0, 20, None);
            let cell_transactions =
                rpc_client.get_transactions_by_lock_hash(lock_hash.clone(), 0, 20, None);
            live_cells.len() == 4 && cell_transactions.len() == 10
        });
        if !result {
            panic!("Wrong indexer store index data");
        }

        info!("Get live cells and transactions in reverse order");
        let live_cells =
            rpc_client.get_live_cells_by_lock_hash(lock_hash.clone(), 0, 20, Some(true));
        let cell_transactions =
            rpc_client.get_transactions_by_lock_hash(lock_hash.clone(), 0, 20, Some(true));
        let tip_number = rpc_client.get_tip_header().number();
        assert_eq!(tip_number, live_cells[0].created_by.block_number.0);
        assert_eq!(tip_number, cell_transactions[0].created_by.block_number.0);

        info!("Generate 5 blocks on node1 and connect node0 to switch fork");
        node1.generate_blocks(5);
        node0.connect(node1);
        node0.waiting_for_sync(node1, 5);
        info!("Live cells size should be 5, cell transactions size should be 5");
        let result = wait_until(5, || {
            let live_cells = rpc_client.get_live_cells_by_lock_hash(lock_hash.clone(), 0, 20, None);
            let cell_transactions =
                rpc_client.get_transactions_by_lock_hash(lock_hash.clone(), 0, 20, None);
            live_cells.len() == 5 && cell_transactions.len() == 5
        });
        if !result {
            panic!("Wrong indexer store index data");
        }

        info!("Should remove data after deindex");
        rpc_client.deindex_lock_hash(lock_hash.clone());
        let live_cells = rpc_client.get_live_cells_by_lock_hash(lock_hash.clone(), 0, 10, None);
        let cell_transactions =
            rpc_client.get_transactions_by_lock_hash(lock_hash.clone(), 0, 10, None);
        assert_eq!(0, live_cells.len());
        assert_eq!(0, cell_transactions.len());

        info!("The block number and hash of index status should be same as tip when gives a higher index from");
        let index_state = rpc_client.index_lock_hash(lock_hash.clone(), Some(100));
        let tip_header = rpc_client.get_tip_header();
        assert_eq!(index_state.block_number.0, tip_header.number());
        assert_eq!(&index_state.block_hash, tip_header.hash());
    }
}
