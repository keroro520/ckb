use super::new_alert_config;
use crate::utils::wait_until;
use crate::{Net, Spec};
use ckb_app_config::CKBAppConfig;
use ckb_core::{alert::AlertBuilder, Bytes};
use ckb_crypto::secp::{Message, Privkey};
use ckb_network_alert::config::SignatureConfig as AlertSignatureConfig;
use ckb_rpc::Module as RPCModule;
use log::info;

pub struct AlertPropagation {
    alert_config: AlertSignatureConfig,
    privkeys: Vec<Privkey>,
}

impl Default for AlertPropagation {
    fn default() -> Self {
        let (alert_config, privkeys) = new_alert_config(2, 3);
        Self {
            alert_config,
            privkeys,
        }
    }
}

impl Spec for AlertPropagation {
    crate::name!("alert_propagation");

    crate::setup!(num_nodes: 3);

    fn run(&self, net: Net) {
        let node0 = &net.nodes[0];
        let warning1 = "pretend we are in dangerous status";
        let id1 = 42;
        let notice_until = faketime::unix_time_as_millis() + 100_000;

        // send alert
        let mut alert = AlertBuilder::default()
            .id(id1)
            .message(warning1.to_string())
            .notice_until(notice_until)
            .build();
        let msg: Message = alert.hash();
        let signatures = self
            .privkeys
            .iter()
            .take(2)
            .map(|key| key.sign_recoverable(&msg))
            .collect::<Result<Vec<_>, _>>()
            .expect("sign alert");
        alert.signatures = signatures
            .iter()
            .map(|s| Bytes::from(s.serialize()))
            .collect();
        // send alert
        node0.send_alert(alert.clone());
        info!("Waiting for alert relay");
        let ret = wait_until(20, || {
            net.nodes
                .iter()
                .all(|node| !node.get_blockchain_info().alerts.is_empty())
        });
        assert!(ret, "alert is relayed");
        for node in net.nodes.iter() {
            let alerts = node.get_blockchain_info().alerts;
            assert_eq!(alerts.len(), 1);
            assert_eq!(alerts[0].message, warning1);
        }

        // cancel previous alert
        let warning2 = "alert is canceled";
        let mut alert2 = AlertBuilder::default()
            .id(2)
            .cancel(id1)
            .message(warning2.to_string())
            .notice_until(notice_until)
            .build();
        let msg: Message = alert2.hash();
        let signatures = self
            .privkeys
            .iter()
            .map(|key| key.sign_recoverable(&msg))
            .collect::<Result<Vec<_>, _>>()
            .expect("sign alert");
        alert2.signatures = signatures
            .iter()
            .map(|s| Bytes::from(s.serialize()))
            .collect();
        node0.send_alert(alert2);
        info!("Waiting for alert relay");
        let ret = wait_until(20, || {
            net.nodes.iter().all(|node| {
                node
                    .get_blockchain_info()
                    .alerts
                    .iter()
                    .all(|a| a.id.0 != id1)
            })
        });
        assert!(ret, "alert is relayed");
        for node in net.nodes.iter() {
            let alerts = node.get_blockchain_info().alerts;
            assert_eq!(alerts.len(), 1);
            assert_eq!(alerts[0].message, warning2);
        }

        // send canceled alert again, should ignore by all nodes
        node0.send_alert(alert);
        let alerts = node0.get_blockchain_info().alerts;
        assert_eq!(alerts.len(), 1);
        assert_eq!(alerts[0].message, warning2);
    }

    fn modify_ckb_config(&self) -> Box<dyn Fn(&mut CKBAppConfig) -> ()> {
        let alert_config = self.alert_config.to_owned();
        Box::new(move |config| {
            config.network.discovery_local_address = true;
            // set test alert config
            config.alert_signature = Some(alert_config.clone());
            // enable alert RPC
            config.rpc.modules.push(RPCModule::Alert);
        })
    }
}
