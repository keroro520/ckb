use crate::utils::{build_block, build_header, exit_ibd_mode, new_block_with_template, wait_until};
use crate::{Net, Node, Spec, TestProtocol};
use ckb_core::block::Block;
use ckb_jsonrpc_types::{ChainInfo, Timestamp};
use ckb_network::PeerIndex;
use ckb_protocol::{get_root, SyncMessage, SyncPayload};
use ckb_sync::NetworkProtocol;
use std::collections::HashSet;
use std::thread::sleep;
use std::time::Duration;

pub struct BlockSyncFromOne;

impl Spec for BlockSyncFromOne {
    crate::name!("block_sync_from_one");

    crate::setup!(
        num_nodes: 2,
        connect_all: false,
        protocols: vec![TestProtocol::sync()],
    );

    // NOTE: ENSURE node0 and nodes1 is in genesis state.
    fn run(&self, _net: Net, nodes: Vec<Node>) {
        let node0 = &nodes[0];
        let node1 = &nodes[1];
        let (rpc_client0, rpc_client1) = (node0, node1);
        assert_eq!(0, rpc_client0.tip_number());
        assert_eq!(0, rpc_client1.tip_number());

        (0..3).for_each(|_| {
            node0.mine_block();
        });

        node1.connect(node0);

        let ret = wait_until(10, || {
            let header0 = rpc_client0.get_tip_header();
            let header1 = rpc_client1.get_tip_header();
            header0 == header1 && header0.number() == 3
        });
        assert!(
            ret,
            "Node0 and node1 should sync with each other until same tip chain",
        );
    }
}

pub struct BlockSyncForks;

impl Spec for BlockSyncForks {
    crate::name!("block_sync_forks");

    crate::setup!(
        num_nodes: 2,
        connect_all: false,
        protocols: vec![TestProtocol::sync()],
    );

    // NOTE: ENSURE node0 and nodes1 is in genesis state.
    fn run(&self, _net: Net, nodes: Vec<Node>) {
        let node0 = &nodes[0];
        let node1 = &nodes[1];
        let (rpc_client0, rpc_client1) = (node0, node1);
        assert_eq!(0, rpc_client0.tip_number());
        assert_eq!(0, rpc_client1.tip_number());

        build_forks(node0, &[2, 0, 0, 0, 0, 0, 0, 0, 0]);
        build_forks(node1, &[1, 0, 0, 0, 0, 0, 0, 0, 0]);
        let info0: ChainInfo = rpc_client0.get_blockchain_info();
        let info1: ChainInfo = rpc_client1.get_blockchain_info();
        let tip0 = rpc_client0.get_tip_header();
        let tip1 = rpc_client1.get_tip_header();
        assert_eq!(tip0.number(), tip1.number());
        assert_ne!(tip0.hash(), tip1.hash());

        // Connect node0 and node1, so that they can sync with each other
        node0.connect(node1);
        let ret = wait_until(10, || {
            let header0 = rpc_client0.get_tip_header();
            let header1 = rpc_client1.get_tip_header();
            header0 == header1
        });
        assert!(
            ret,
            "Node0 and node1 should sync with each other until same tip chain",
        );
        for number in 1u64..tip0.number() {
            let block0 = rpc_client0.get_block_by_number(number);
            let block1 = rpc_client1.get_block_by_number(number);
            assert_eq!(
                block0, block1,
                "nodes should have same best chain after synchronizing",
            );
        }
        let info00: ChainInfo = rpc_client0.get_blockchain_info();
        let info11: ChainInfo = rpc_client1.get_blockchain_info();
        let medians = vec![
            info0.median_time.0,
            info00.median_time.0,
            info1.median_time.0,
            info11.median_time.0,
        ]
        .into_iter()
        .collect::<HashSet<u64>>();
        assert_eq!(info00.median_time, info11.median_time);
        assert_eq!(medians.len(), 2);
    }
}

pub struct BlockSyncDuplicatedAndReconnect;

impl Spec for BlockSyncDuplicatedAndReconnect {
    crate::name!("block_sync_duplicated_and_reconnect");

    crate::setup!(protocols: vec![TestProtocol::sync()]);

    // Case: Sync a header, sync a duplicated header, reconnect and sync a duplicated header
    fn run(&self, net: Net, nodes: Vec<Node>) {
        let node = &nodes[0];
        let rpc_client = node;
        exit_ibd_mode(&nodes);
        net.connect(node);
        let (peer_id, _, _) = net
            .recv_timeout(Duration::new(10, 0))
            .expect("build connection with node");

        // Sync a new header to `node`, `node` should send back a corresponding GetBlocks message
        let block = node.build_block(None, None, None);
        sync_header(&net, peer_id, &block);
        let (_, _, data) = net
            .recv_timeout(Duration::new(10, 0))
            .expect("Expect SyncMessage");
        let message = get_root::<SyncMessage>(&data).unwrap();
        assert_eq!(
            message.payload_type(),
            SyncPayload::GetBlocks,
            "Node should send back GetBlocks message for the block {:x}",
            block.header().hash(),
        );

        // Sync duplicated header again, `node` should discard the duplicated one.
        // So we will not receive any response messages
        sync_header(&net, peer_id, &block);
        assert!(
            net.recv_timeout(Duration::new(10, 0)).is_err(),
            "node should discard duplicated sync headers",
        );

        // Disconnect and reconnect node, and then sync the same header
        // `node` should send back a corresponding GetBlocks message
        let ctrl = net.controller();
        let peer = ctrl.0.connected_peers()[peer_id.value() - 1].clone();
        ctrl.0.remove_node(&peer.0);
        wait_until(5, || {
            rpc_client.get_peers().is_empty() && ctrl.0.connected_peers().is_empty()
        });

        net.connect(node);
        let (peer_id, _, _) = net
            .recv_timeout(Duration::new(10, 0))
            .expect("build connection with node");
        sync_header(&net, peer_id, &block);
        let (_, _, data) = net
            .recv_timeout(Duration::new(10, 0))
            .expect("Expect SyncMessage");
        let message = get_root::<SyncMessage>(&data).unwrap();
        assert_eq!(
            message.payload_type(),
            SyncPayload::GetBlocks,
            "Node should send back GetBlocks message for the block {:x}",
            block.header().hash(),
        );

        // Sync corresponding block entity, `node` should accept the block as tip block
        sync_block(&net, peer_id, &block);
        let hash = block.header().hash().clone();
        wait_until(10, || rpc_client.get_tip_header().hash() == &hash);
    }
}

pub struct BlockSyncOrphanBlocks;

impl Spec for BlockSyncOrphanBlocks {
    crate::name!("block_sync_orphan_blocks");

    crate::setup!(
        num_nodes: 2,
        connect_all: false,
        protocols: vec![TestProtocol::sync()],
    );

    fn run(&self, net: Net, nodes: Vec<Node>) {
        let node0 = &nodes[0];
        let node1 = &nodes[1];
        exit_ibd_mode(&nodes);
        net.connect(node0);
        let (peer_id, _, _) = net
            .recv_timeout(Duration::new(10, 0))
            .expect("net receive timeout");
        let rpc_client = node0;
        let tip_number = rpc_client.tip_number();

        // Generate some blocks from node1
        let mut blocks: Vec<Block> = (1..=5)
            .map(|_| {
                let block = node1.build_block(None, None, None);
                node1.submit_block(&block);
                block
            })
            .collect();

        // Send headers to node0, keep blocks body
        blocks.iter().for_each(|block| {
            sync_header(&net, peer_id, block);
        });

        // Wait for block fetch timer
        sleep(Duration::from_secs(5));

        // Skip the next block, send the rest blocks to node0
        let first = blocks.remove(0);
        blocks.into_iter().for_each(|block| {
            sync_block(&net, peer_id, &block);
        });
        let ret = wait_until(5, || rpc_client.tip_number() > tip_number);
        assert!(!ret, "node0 should stay the same");

        // Send that skipped first block to node0
        sync_block(&net, peer_id, &first);
        let ret = wait_until(10, || rpc_client.tip_number() > tip_number + 2);
        assert!(ret, "node0 should grow up");
    }
}

fn build_forks(node: &Node, offsets: &[u64]) {
    let rpc_client = node;
    for offset in offsets.iter() {
        let mut template = rpc_client.get_block_template(None, None, None);
        template.current_time = Timestamp(template.current_time.0 + offset);
        let block = new_block_with_template(template);
        node.submit_block(&block);
    }
}

fn sync_header(net: &Net, peer_id: PeerIndex, block: &Block) {
    net.send(
        NetworkProtocol::SYNC.into(),
        peer_id,
        build_header(block.header()),
    );
}

fn sync_block(net: &Net, peer_id: PeerIndex, block: &Block) {
    net.send(NetworkProtocol::SYNC.into(), peer_id, build_block(block));
}
