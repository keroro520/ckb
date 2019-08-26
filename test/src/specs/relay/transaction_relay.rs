use crate::utils::{assert_tx_pool_size, exit_ibd_mode, wait_until, waiting_for_sync};
use crate::{Net, Spec};
use ckb_types::{
    core::{Capacity, TransactionBuilder},
    packed::{CellInput, OutPoint},
    prelude::*,
};
use log::info;

pub struct TransactionRelayBasic;

impl Spec for TransactionRelayBasic {
    crate::name!("transaction_relay_basic");

    crate::setup!(num_nodes: 3);

    fn run(&self, net: Net) {
        exit_ibd_mode(&net.nodes);

        let node0 = &net.nodes[0];
        let node1 = &net.nodes[1];
        let node2 = &net.nodes[2];

        info!("Generate new transaction on node1");
        node1.generate_block();
        let hash = node1.generate_transaction();

        info!("Waiting for relay");
        let rpc_client = node0.rpc_client();
        let ret = wait_until(10, || {
            if let Some(transaction) = rpc_client.get_transaction(hash.clone()) {
                transaction.tx_status.block_hash.is_none()
            } else {
                false
            }
        });
        assert!(ret, "Transaction should be relayed to node0");

        let rpc_client = node2.rpc_client();
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
        let block = exit_ibd_mode(&net.nodes);
        let node0 = &net.nodes[0];
        info!("Use generated block's cellbase as tx input");
        let reward: Capacity = block.transactions()[0]
            .outputs()
            .as_reader()
            .get(0)
            .unwrap()
            .to_entity()
            .capacity()
            .unpack();
        let txs_num = reward.as_u64() / MIN_CAPACITY;

        let parent_hash = block.transactions()[0].hash().to_owned();
        let temp_transaction = node0.new_transaction(parent_hash.unpack());
        let output = temp_transaction
            .outputs()
            .as_reader()
            .get(0)
            .unwrap()
            .to_entity()
            .as_builder()
            .capacity(Capacity::shannons(reward.as_u64() / txs_num).pack())
            .build();
        let mut tb = temp_transaction
            .as_advanced_builder()
            .set_outputs(Vec::new());
        for _ in 0..txs_num {
            tb = tb.output(output.clone());
        }
        let transaction = tb.build();
        node0
            .rpc_client()
            .send_transaction(transaction.data().into());
        node0.generate_block();
        node0.generate_block();
        node0.generate_block();
        waiting_for_sync(&net.nodes, 4);

        info!("Send multiple transactions to node0");
        let tx_hash = transaction.hash().to_owned();
        transaction
            .outputs()
            .into_iter()
            .enumerate()
            .for_each(|(i, output)| {
                let tx = TransactionBuilder::default()
                    .cell_dep(
                        transaction
                            .cell_deps()
                            .as_reader()
                            .get(0)
                            .unwrap()
                            .to_entity(),
                    )
                    .output(output.clone())
                    .input(CellInput::new(
                        OutPoint::new(tx_hash.clone().unpack(), i as u32),
                        0,
                    ))
                    .build();
                node0.rpc_client().send_transaction(tx.data().into());
            });

        node0.generate_block();
        node0.generate_block();
        node0.generate_block();
        waiting_for_sync(&net.nodes, 7);

        info!("All transactions should be relayed and mined");
        assert_tx_pool_size(node0, 0, 0);

        net.nodes.iter().for_each(|node| {
            assert_eq!(
                node.get_tip_block().transactions().len() as u64,
                txs_num + 1
            )
        });
    }
}
