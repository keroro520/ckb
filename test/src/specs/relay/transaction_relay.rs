use crate::utils::{assert_tx_pool_size, wait_until};
use crate::{Net, Spec};
use ckb_core::transaction::{CellInput, OutPoint, TransactionBuilder};
use ckb_core::Capacity;
use log::info;

pub struct TransactionRelayBasic;

impl Spec for TransactionRelayBasic {
    crate::name!("transaction_relay_basic");

    crate::setup!(num_nodes: 3);

    fn run(&self, net: Net) {
        net.exit_ibd_mode();

        let node0 = &net.nodes[0];
        let node1 = &net.nodes[1];
        let node2 = &net.nodes[2];

        info!("Generate new transaction on node1");
        node1.mine_block();
        let hash = node1.send_transaction_with_tip_cellbase();

        info!("Waiting for relay");
        let rpc_client = node0;
        let ret = wait_until(10, || {
            if let Some(transaction) = rpc_client.get_transaction(hash.clone()) {
                transaction.tx_status.block_hash.is_none()
            } else {
                false
            }
        });
        assert!(ret, "Transaction should be relayed to node0");

        let rpc_client = node2;
        let ret = wait_until(10, || {
            if let Some(transaction) = rpc_client.get_transaction(hash.clone()) {
                transaction.tx_status.block_hash.is_none()
            } else {
                false
            }
        });
        assert!(ret, "Transaction should be relayed to node2");
    }
}

const MIN_CAPACITY: u64 = 60_0000_0000;

pub struct TransactionRelayMultiple;

impl Spec for TransactionRelayMultiple {
    crate::name!("transaction_relay_multiple");

    crate::setup!(num_nodes: 5);

    fn run(&self, net: Net) {
        let block = net.exit_ibd_mode();
        let node0 = &net.nodes[0];
        info!("Use generated block's cellbase as tx input");
        let reward = block.transactions()[0].outputs()[0].capacity;
        let txs_num = reward.as_u64() / MIN_CAPACITY;

        let parent_hash = block.transactions()[0].hash().to_owned();
        let temp_transaction = node0.build_transaction_with_hash(parent_hash);
        let mut output = temp_transaction.outputs()[0].clone();
        output.capacity = Capacity::shannons(reward.as_u64() / txs_num);
        let mut tb = TransactionBuilder::from_transaction(temp_transaction).outputs_clear();
        for _ in 0..txs_num {
            tb = tb.output(output.clone());
        }
        let transaction = tb.build();
        node0.send_transaction(&transaction);
        node0.mine_block();
        node0.mine_block();
        node0.mine_block();
        net.waiting_for_sync(4);

        info!("Send multiple transactions to node0");
        let tx_hash = transaction.hash().to_owned();
        transaction
            .outputs()
            .iter()
            .enumerate()
            .for_each(|(i, output)| {
                let tx = TransactionBuilder::default()
                    .cell_dep(transaction.cell_deps()[0].clone())
                    .output(output.clone())
                    .input(CellInput::new(OutPoint::new(tx_hash.clone(), i as u32), 0))
                    .build();
                node0.send_transaction(&tx);
            });

        node0.mine_block();
        node0.mine_block();
        node0.mine_block();
        net.waiting_for_sync(7);

        info!("All transactions should be relayed and mined");
        assert_tx_pool_size(node0, 0, 0);

        net.nodes
            .iter()
            .for_each(|node| assert_eq!(node.tip_block().transactions().len() as u64, txs_num + 1));
    }
}
