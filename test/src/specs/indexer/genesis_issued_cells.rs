use crate::utils::wait_until;
use crate::{Net, Spec};
use ckb_chain_spec::{ChainSpec, IssuedCell};
use ckb_types::{
    bytes::Bytes,
    core::{capacity_bytes, Capacity, ScriptHashType},
    h256,
    packed::Script,
    prelude::*,
    H256,
};
use log::info;

pub struct GenesisIssuedCells;

impl Spec for GenesisIssuedCells {
    crate::name!("genesis_issued_cells");

    fn run(&self, net: Net) {
        let node0 = &net.nodes[0];

        let lock_hash = Script::new_builder()
            .args(vec![Bytes::from(vec![1]).pack(), Bytes::from(vec![2]).pack()].pack())
            .code_hash(h256!("0xa1").pack())
            .hash_type(ScriptHashType::Data.pack())
            .build()
            .calc_script_hash();
        info!("{:x}", lock_hash);

        info!("Should return live cells and cell transactions of genesis issued cells");
        node0.index_lock_hash(lock_hash.clone(), Some(0));
        let result = wait_until(5, || {
            let live_cells = node0.get_live_cells_by_lock_hash(lock_hash.clone(), 0, 20, None);
            let cell_transactions =
                node0.get_transactions_by_lock_hash(lock_hash.clone(), 0, 20, None);
            live_cells.len() == 1 && cell_transactions.len() == 1
        });
        if !result {
            panic!("Wrong indexer store index data");
        }
    }

    fn modify_chain_spec(&self) -> Box<dyn Fn(&mut ChainSpec) -> ()> {
        Box::new(|spec_config| {
            spec_config.genesis.issued_cells = vec![IssuedCell {
                capacity: capacity_bytes!(5_000),
                lock: Script::new_builder()
                    .args(vec![Bytes::from(vec![1]).pack(), Bytes::from(vec![2]).pack()].pack())
                    .code_hash(h256!("0xa1").pack())
                    .hash_type(ScriptHashType::Data.pack())
                    .build()
                    .into(),
            }];
        })
    }
}
