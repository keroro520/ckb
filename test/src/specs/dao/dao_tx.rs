use crate::specs::dao::utils::{
    deposit_transaction, deposit_type_script, ensure_committed, goto_target_point,
    minimal_unlock_point, withdraw_transaction,
};
use crate::utils::{assert_send_transaction_fail, since_from_absolute_epoch_number};
use crate::{Net, Spec, DEFAULT_TX_PROPOSAL_WINDOW};
use ckb_chain_spec::ChainSpec;
use ckb_types::core::EpochNumberWithFraction;
use ckb_types::{core::Capacity, packed::OutPoint, prelude::*};

pub struct WithdrawDAO;

impl Spec for WithdrawDAO {
    crate::name!("withdraw_dao");

    fn modify_chain_spec(&self) -> Box<dyn Fn(&mut ChainSpec) -> ()> {
        Box::new(|spec_config| {
            spec_config.params.genesis_epoch_length = 2;
            spec_config.params.epoch_duration_target = 2;
            spec_config.params.permanent_difficulty_in_dummy = true;
        })
    }

    fn run(&self, net: &mut Net) {
        let node = &net.nodes[0];
        node.generate_blocks((DEFAULT_TX_PROPOSAL_WINDOW.1 + 2) as usize);

        let deposited = {
            let unspent = node
                .new_transaction_spend_tip_cellbase()
                .inputs()
                .get(0)
                .unwrap()
                .previous_output();
            let transaction = deposit_transaction(node, unspent);
            ensure_committed(node, &transaction)
        };
        let withdrawal = {
            let tip_hash = node.rpc_client().get_tip_header().hash.pack();
            withdraw_transaction(node, deposited.clone(), tip_hash)
        };
        let since = EpochNumberWithFraction::from_full_value(
            withdrawal.inputs().get(0).unwrap().since().unpack(),
        );
        goto_target_point(node, since);
        ensure_committed(node, &withdrawal);
    }
}

pub struct WithdrawImmatureDAO;

impl Spec for WithdrawImmatureDAO {
    crate::name!("withdraw_immature_dao");

    fn modify_chain_spec(&self) -> Box<dyn Fn(&mut ChainSpec) -> ()> {
        Box::new(|spec_config| {
            spec_config.params.genesis_epoch_length = 2;
            spec_config.params.epoch_duration_target = 2;
            spec_config.params.permanent_difficulty_in_dummy = true;
        })
    }

    fn run(&self, net: &mut Net) {
        let node = &mut net.nodes[0];
        node.generate_blocks((DEFAULT_TX_PROPOSAL_WINDOW.1 + 2) as usize);

        let deposit_out_point = {
            let unspent = node
                .new_transaction_spend_tip_cellbase()
                .inputs()
                .get(0)
                .unwrap()
                .previous_output();
            let transaction = deposit_transaction(node, unspent);
            ensure_committed(node, &transaction)
        };
        let minimal_unlock_point = minimal_unlock_point(node, &deposit_out_point);
        let immature_point = EpochNumberWithFraction::new(
            minimal_unlock_point.number() - 1,
            minimal_unlock_point.index(),
            minimal_unlock_point.length(),
        );

        // Try to send an immature withdrawal
        let immature_withdrawal = {
            let tip_hash = node.rpc_client().get_tip_header().hash.pack();
            let tx = withdraw_transaction(node, deposit_out_point, tip_hash);
            let immature_input = {
                let input = tx.inputs().get(0).unwrap();
                let immature_since = since_from_absolute_epoch_number(immature_point.full_value());
                input.as_builder().since(immature_since.pack()).build()
            };
            tx.as_advanced_builder()
                .set_inputs(vec![immature_input])
                .build()
        };
        goto_target_point(node, immature_point);
        assert_send_transaction_fail(node, &immature_withdrawal, "ValidationFailure(-17)");
    }
}

pub struct WithdrawAndDepositDAOWithinSameTx;

impl Spec for WithdrawAndDepositDAOWithinSameTx {
    crate::name!("withdraw_and_deposit_dao_within_same_tx");

    fn modify_chain_spec(&self) -> Box<dyn Fn(&mut ChainSpec) -> ()> {
        Box::new(|spec_config| {
            spec_config.params.genesis_epoch_length = 2;
            spec_config.params.epoch_duration_target = 2;
            spec_config.params.permanent_difficulty_in_dummy = true;
        })
    }

    fn run(&self, net: &mut Net) {
        let node = &net.nodes[0];
        node.generate_blocks((DEFAULT_TX_PROPOSAL_WINDOW.1 + 2) as usize);

        let deposited = {
            let unspent = node
                .new_transaction_spend_tip_cellbase()
                .inputs()
                .get(0)
                .unwrap()
                .previous_output();
            let transaction = deposit_transaction(node, unspent);
            ensure_committed(node, &transaction)
        };
        let withdrawal = {
            let tip_hash = node.rpc_client().get_tip_header().hash.pack();
            withdraw_transaction(node, deposited.clone(), tip_hash)
        };
        let re_deposited = {
            let outputs: Vec<_> = withdrawal
                .outputs()
                .into_iter()
                .map(|cell_output| {
                    cell_output
                        .as_builder()
                        .type_(Some(deposit_type_script(node)).pack())
                        .build()
                })
                .collect();
            withdrawal
                .as_advanced_builder()
                .set_outputs(outputs)
                .build()
        };
        let since = EpochNumberWithFraction::from_full_value(
            re_deposited.inputs().get(0).unwrap().since().unpack(),
        );
        goto_target_point(node, since);
        ensure_committed(node, &re_deposited);

        let withdrawal = {
            let tip_hash = node.rpc_client().get_tip_header().hash.pack();
            let re_deposited_out_point = OutPoint::new(re_deposited.hash(), 0);
            withdraw_transaction(node, re_deposited_out_point, tip_hash)
        };
        let since = EpochNumberWithFraction::from_full_value(
            withdrawal.inputs().get(0).unwrap().since().unpack(),
        );
        goto_target_point(node, since);
        ensure_committed(node, &withdrawal);
    }
}

pub struct WithdrawDAOWithOverflowCapacity;

impl Spec for WithdrawDAOWithOverflowCapacity {
    crate::name!("withdraw_dao_with_overflow_capacity");

    fn modify_chain_spec(&self) -> Box<dyn Fn(&mut ChainSpec) -> ()> {
        Box::new(|spec_config| {
            spec_config.params.genesis_epoch_length = 2;
            spec_config.params.epoch_duration_target = 2;
            spec_config.params.permanent_difficulty_in_dummy = true;
        })
    }

    fn run(&self, net: &mut Net) {
        let node = &net.nodes[0];
        node.generate_blocks((DEFAULT_TX_PROPOSAL_WINDOW.1 + 2) as usize);

        let deposited = {
            let unspent = node
                .new_transaction_spend_tip_cellbase()
                .inputs()
                .get(0)
                .unwrap()
                .previous_output();
            let transaction = deposit_transaction(node, unspent);
            ensure_committed(node, &transaction)
        };
        let withdrawal = {
            let tip_hash = node.rpc_client().get_tip_header().hash.pack();
            let transaction = withdraw_transaction(node, deposited, tip_hash);
            let outputs: Vec<_> = transaction
                .outputs()
                .into_iter()
                .map(|cell_output| {
                    let old_capacity: Capacity = cell_output.capacity().unpack();
                    let new_capacity = old_capacity.safe_add(Capacity::one()).unwrap();
                    cell_output
                        .as_builder()
                        .capacity(new_capacity.pack())
                        .build()
                })
                .collect();
            transaction
                .as_advanced_builder()
                .set_outputs(outputs)
                .build()
        };
        assert_send_transaction_fail(node, &withdrawal, "CapacityOverflow");
    }
}
