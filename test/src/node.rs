use crate::rpc::RpcClient;
use crate::utils::wait_until;
use ckb_app_config::{BlockAssemblerConfig, CKBAppConfig};
use ckb_chain_spec::consensus::Consensus;
use ckb_chain_spec::ChainSpec;
use ckb_core::block::{Block, BlockBuilder};
use ckb_core::script::{Script, ScriptHashType};
use ckb_core::transaction::{
    CellDep, CellInput, CellOutputBuilder, OutPoint, Transaction, TransactionBuilder,
};
use ckb_core::{capacity_bytes, BlockNumber, Bytes, Capacity};
use ckb_jsonrpc_types::JsonBytes;
use numext_fixed_hash::H256;
use std::convert::Into;
use std::fs;
use std::io::Error;
use std::ops::Deref;
use std::path::Path;
use std::process::{self, Child, Command, Stdio};

pub struct Node {
    binary: String,
    dir: String,
    p2p_port: u16,
    rpc_port: u16,
    rpc_client: RpcClient,
    node_id: String,
    consensus: Consensus,
    guard: Option<ProcessGuard>,
}

struct ProcessGuard(pub Child);

impl Drop for ProcessGuard {
    fn drop(&mut self) {
        match self.0.kill() {
            Err(e) => log::error!("Could not kill ckb process: {}", e),
            Ok(_) => log::debug!("Successfully killed ckb process"),
        }
        let _ = self.0.wait();
    }
}

impl Deref for Node {
    type Target = RpcClient;
    fn deref(&self) -> &Self::Target {
        &self.rpc_client
    }
}

impl Node {
    pub fn new(binary: &str, dir: &str, p2p_port: u16, rpc_port: u16) -> Self {
        let rpc_client = RpcClient::new(&format!("http://127.0.0.1:{}/", rpc_port));
        Self {
            binary: binary.to_string(),
            dir: dir.to_string(),
            p2p_port,
            rpc_port,
            rpc_client,
            node_id: Default::default(),
            consensus: Default::default(),
            guard: None,
        }
    }

    pub fn node_id(&self) -> &String {
        &self.node_id
    }

    pub fn p2p_port(&self) -> u16 {
        self.p2p_port
    }

    pub fn consensus(&self) -> &Consensus {
        &self.consensus
    }

    pub fn genesis_cellbase_hash(&self) -> H256 {
        self.consensus().genesis_block().transactions()[0]
            .hash()
            .to_owned()
    }

    pub fn always_success_code_hash(&self) -> H256 {
        self.consensus().genesis_block().transactions()[0].outputs()[1]
            .data_hash()
            .to_owned()
    }

    pub fn always_success_script(&self) -> Script {
        Script::new(
            Vec::new(),
            self.always_success_code_hash(),
            ScriptHashType::Data,
        )
    }

    pub fn always_success_out_point(&self) -> OutPoint {
        OutPoint::new(self.genesis_cellbase_hash(), 1)
    }

    pub fn start(
        &mut self,
        modify_chain_spec: Box<dyn Fn(&mut ChainSpec) -> ()>,
        modify_ckb_config: Box<dyn Fn(&mut CKBAppConfig) -> ()>,
    ) {
        self.init_config_file(modify_chain_spec, modify_ckb_config)
            .expect("failed to init config file");

        let child_process = Command::new(self.binary.to_owned())
            .env("RUST_BACKTRACE", "full")
            .args(&["-C", &self.dir, "run", "--ba-advanced"])
            .stdin(Stdio::null())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .spawn()
            .expect("failed to run binary");
        self.guard = Some(ProcessGuard(child_process));
        log::info!("Started node with working dir: {}", self.dir);

        loop {
            let result = { self.rpc_client().inner().local_node_info().call() };
            if let Ok(local_node_info) = result {
                self.node_id = local_node_info.node_id;
                let _ = self.tx_pool_info();
                break;
            } else if let Some(ref mut child) = self.guard {
                match child.0.try_wait() {
                    Ok(Some(exit)) => {
                        log::error!("Error: node crashed, {}", exit);
                        process::exit(exit.code().unwrap());
                    }
                    Ok(None) => {
                        std::thread::sleep(std::time::Duration::from_secs(1));
                    }
                    Err(error) => {
                        log::error!("Error: node crashed with reason: {}", error);
                        process::exit(255);
                    }
                }
            }
        }
    }

    pub fn connect(&self, outbound_peer: &Node) {
        let node_info = outbound_peer.local_node_info();

        let node_id = node_info.node_id;
        self.add_node(
            node_id.clone(),
            format!("/ip4/127.0.0.1/tcp/{}", outbound_peer.p2p_port),
        );

        let result = wait_until(5, || {
            let peers = self.get_peers();
            peers.iter().any(|peer| peer.node_id == node_id)
        });

        if !result {
            panic!("Connect outbound peer timeout, node id: {}", node_id);
        }
    }

    pub fn connect_uncheck(&self, outbound_peer: &Node) {
        let node_info = outbound_peer.local_node_info();

        let node_id = node_info.node_id;
        self.add_node(
            node_id.clone(),
            format!("/ip4/127.0.0.1/tcp/{}", outbound_peer.p2p_port),
        );
    }

    pub fn disconnect(&self, node: &Node) {
        let node_info = node.local_node_info();

        let node_id = node_info.node_id;
        self.remove_node(node_id.clone());

        let result = wait_until(5, || {
            let peers = self.get_peers();
            peers.iter().all(|peer| peer.node_id != node_id)
        });

        if !result {
            panic!("Disconnect timeout, node {}", node_id);
        }
    }

    pub fn rpc_client(&self) -> &RpcClient {
        &self.rpc_client
    }

    /// Return the tip block
    pub fn tip_block(&self) -> Block {
        self.get_block_by_number(self.tip_number())
    }

    /// Return the tip block number
    pub fn tip_number(&self) -> BlockNumber {
        self.get_tip_block_number()
    }

    /// Return the out-point of the cellbase of the tip block
    pub fn tip_cellbase_out_point(&self) -> OutPoint {
        let tx_hash = self.tip_block().transactions()[0].hash().to_owned();
        OutPoint::new(tx_hash, 0)
    }

    /// Build and submit an always-success transaction which spend the tip cellbase
    pub fn send_transaction_with_tip_cellbase(&self) -> H256 {
        let transaction = self.build_transaction_with_tip_cellbase();
        self.send_transaction(&transaction)
    }

    /// Build an always-success transaction which spend the tip cellbase
    pub fn build_transaction_with_tip_cellbase(&self) -> Transaction {
        let tx_hash = self.tip_cellbase_out_point().tx_hash;
        self.build_transaction_with_hash(tx_hash)
    }

    /// Build an always-success transaction which spend the 1st output of the given `tx_hash`,
    /// and with since = 0
    pub fn build_transaction_with_hash(&self, tx_hash: H256) -> Transaction {
        let out_point = OutPoint::new(tx_hash, 0);
        let previous_input = CellInput::new(out_point, 0);
        self.build_transaction(previous_input)
    }

    /// Build an always-success transaction which:
    ///   - input is the given parameter
    ///   - output's capacity is 100
    ///   - output's data is empty
    ///   - output's lock script is always-success script
    pub fn build_transaction(&self, previous_input: CellInput) -> Transaction {
        const ALWAYS_OUTPUT_CAPACITY: Capacity = capacity_bytes!(100);

        let always_success_out_point = self.always_success_out_point();
        let always_success_script = self.always_success_script();
        TransactionBuilder::default()
            .cell_dep(CellDep::new_cell(always_success_out_point))
            .output(
                CellOutputBuilder::default()
                    .capacity(ALWAYS_OUTPUT_CAPACITY)
                    .lock(always_success_script)
                    .build(),
            )
            .output_data(Bytes::new())
            .input(previous_input)
            .build()
    }

    /// Submit the given block
    pub fn submit_block(&self, block: &Block) -> H256 {
        self.rpc_client()
            .submit_block("".to_owned(), block)
            .expect("submit block")
    }

    /// Mine the next given number of blocks
    ///
    /// Poll block-templates from CKB and submit the new corresponding blocks continuously
    pub fn mine_blocks(&self, blocks_count: usize) -> Vec<H256> {
        (0..blocks_count).map(|_| self.mine_block()).collect()
    }

    /// Mine the next block
    ///
    /// Poll the next block-template from CKB and submit the new corresponding block
    pub fn mine_block(&self) -> H256 {
        self.submit_block(&self.build_block(None, None, None))
    }

    /// Build the next block
    ///
    /// Poll the next block-template and convert it into block
    pub fn build_block(
        &self,
        bytes_limit: Option<u64>,
        proposals_limit: Option<u64>,
        max_version: Option<u32>,
    ) -> Block {
        self.build_block_builder(bytes_limit, proposals_limit, max_version)
            .build()
    }

    /// Build the block-builder of the next block
    ///
    /// Poll the next block-template and convert it into BlockBuilder
    pub fn build_block_builder(
        &self,
        bytes_limit: Option<u64>,
        proposals_limit: Option<u64>,
        max_version: Option<u32>,
    ) -> BlockBuilder {
        self.get_block_template(bytes_limit, proposals_limit, max_version)
            .into()
    }

    fn prepare_chain_spec(
        &mut self,
        modify_chain_spec: Box<dyn Fn(&mut ChainSpec) -> ()>,
    ) -> Result<(), Error> {
        let integration_spec = include_bytes!("../integration.toml");
        let always_success_cell = include_bytes!("../../script/testdata/always_success");
        let always_success_path = Path::new(&self.dir).join("specs/cells/always_success");
        fs::create_dir_all(format!("{}/specs", self.dir))?;
        fs::create_dir_all(format!("{}/specs/cells", self.dir))?;
        fs::write(&always_success_path, &always_success_cell[..])?;

        let mut spec: ChainSpec =
            toml::from_slice(&integration_spec[..]).expect("chain spec config");
        for r in spec.genesis.system_cells.files.iter_mut() {
            r.absolutize(Path::new(&self.dir).join("specs"));
        }
        modify_chain_spec(&mut spec);

        self.consensus = spec.build_consensus().expect("build consensus");

        // write to dir
        fs::write(
            Path::new(&self.dir).join("specs/integration.toml"),
            toml::to_string(&spec).expect("chain spec serialize"),
        )
    }

    fn rewrite_spec(
        &self,
        modify_ckb_config: Box<dyn Fn(&mut CKBAppConfig) -> ()>,
    ) -> Result<(), Error> {
        // rewrite ckb.toml
        let ckb_config_path = format!("{}/ckb.toml", self.dir);
        let mut ckb_config: CKBAppConfig =
            toml::from_slice(&fs::read(&ckb_config_path)?).expect("ckb config");
        ckb_config.block_assembler = Some(BlockAssemblerConfig {
            code_hash: self.always_success_code_hash(),
            args: Default::default(),
            data: JsonBytes::default(),
            hash_type: ScriptHashType::Data,
        });

        if ::std::env::var("CI").is_ok() {
            ckb_config.logger.filter =
                Some(::std::env::var("CKB_LOG").unwrap_or_else(|_| "info".to_string()));
        }

        modify_ckb_config(&mut ckb_config);
        fs::write(
            &ckb_config_path,
            toml::to_string(&ckb_config).expect("ckb config serialize"),
        )
    }

    fn init_config_file(
        &mut self,
        modify_chain_spec: Box<dyn Fn(&mut ChainSpec) -> ()>,
        modify_ckb_config: Box<dyn Fn(&mut CKBAppConfig) -> ()>,
    ) -> Result<(), Error> {
        let rpc_port = format!("{}", self.rpc_port).to_string();
        let p2p_port = format!("{}", self.p2p_port).to_string();

        Command::new(self.binary.to_owned())
            .args(&[
                "-C",
                &self.dir,
                "init",
                "--chain",
                "integration",
                "--rpc-port",
                &rpc_port,
                "--p2p-port",
                &p2p_port,
            ])
            .output()
            .map(|_| ())?;

        self.prepare_chain_spec(modify_chain_spec)?;
        self.rewrite_spec(modify_ckb_config)?;
        Ok(())
    }
}
