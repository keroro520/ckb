use ckb_core::{
    alert::Alert,
    block::Block,
    header::Header,
    transaction::{OutPoint, Transaction},
    BlockNumber, Capacity, EpochNumber, Version,
};
use ckb_jsonrpc_types::{
    Alert as JsonAlert, BannedAddress as JsonBannedAddress, BlockNumber as JsonBlockNumber,
    BlockTemplate as JsonBlockTemplate, BlockView as JsonBlock, Capacity as JsonCapacity,
    CellOutputWithOutPoint as JsonCellOutputWithOutPoint, CellTransaction as JsonCellTransaction,
    CellWithStatus as JsonCellWithStatus, ChainInfo as JsonChainInfo,
    DryRunResult as JsonDryRunResult, EpochNumber as JsonEpochNumber, EpochView as JsonEpoch,
    HeaderView as JsonHeader, LiveCell as JsonLiveCell,
    LockHashIndexState as JsonLockHashIndexState, Node as JsonNode, OutPoint as JsonOutPoint,
    PeerState as JsonPeerState, Timestamp as JsonTimestamp, Transaction as JsonTransaction,
    TransactionWithStatus as JsonTransactionWithStatus, TxPoolInfo as JsonTxPoolInfo,
    Unsigned as JsonUnsigned, Version as JsonVersion,
};
use ckb_util::{Mutex, MutexGuard};
use jsonrpc_client_core::{expand_params, jsonrpc_client, Result as JsonRpcResult};
use jsonrpc_client_http::{HttpHandle, HttpTransport};
use numext_fixed_hash::H256;

pub struct RpcClient {
    inner: Mutex<Inner<HttpHandle>>,
}

impl RpcClient {
    pub fn new(uri: &str) -> Self {
        let transport = HttpTransport::new().standalone().unwrap();
        let transport = transport
            .handle(uri)
            .expect("ckb uri, e.g. \"http://127.0.0.1:8114\"");
        Self {
            inner: Mutex::new(Inner::new(transport)),
        }
    }

    pub fn inner(&self) -> MutexGuard<Inner<HttpHandle>> {
        self.inner.lock()
    }

    pub fn get_block(&self, hash: H256) -> Option<Block> {
        self.inner()
            .get_block(hash)
            .call()
            .expect("rpc call get_block")
            .map(Into::into)
    }

    pub fn get_block_by_number(&self, number: BlockNumber) -> Block {
        self.inner()
            .get_block_by_number(JsonBlockNumber(number))
            .call()
            .expect("rpc call get_block_by_number")
            .expect("get_block_by_number return none")
            .into()
    }

    pub fn get_header(&self, hash: H256) -> Header {
        self.inner()
            .get_header(hash)
            .call()
            .expect("rpc call get_header")
            .expect("get_header return none")
            .into()
    }

    pub fn get_header_by_number(&self, number: BlockNumber) -> Header {
        self.inner()
            .get_header_by_number(JsonBlockNumber(number))
            .call()
            .expect("rpc call get_header_by_number")
            .expect("get_header_by_number return none")
            .into()
    }

    pub fn get_transaction(&self, hash: H256) -> Option<JsonTransactionWithStatus> {
        self.inner()
            .get_transaction(hash)
            .call()
            .expect("rpc call get_transaction")
    }

    pub fn get_block_hash(&self, number: BlockNumber) -> Option<H256> {
        self.inner()
            .get_block_hash(JsonBlockNumber(number))
            .call()
            .expect("rpc call get_block_hash")
    }

    pub fn get_tip_header(&self) -> Header {
        self.inner()
            .get_tip_header()
            .call()
            .expect("rpc call get_block_hash")
            .into()
    }

    pub fn get_cells_by_lock_hash(
        &self,
        lock_hash: H256,
        from: BlockNumber,
        to: BlockNumber,
    ) -> Vec<JsonCellOutputWithOutPoint> {
        self.inner()
            .get_cells_by_lock_hash(lock_hash, JsonBlockNumber(from), JsonBlockNumber(to))
            .call()
            .expect("rpc call get_cells_by_lock_hash")
    }

    pub fn get_live_cell(&self, out_point: OutPoint) -> JsonCellWithStatus {
        self.inner()
            .get_live_cell(out_point.into())
            .call()
            .expect("rpc call get_live_cell")
    }

    pub fn get_tip_block_number(&self) -> BlockNumber {
        self.inner()
            .get_tip_block_number()
            .call()
            .expect("rpc call get_tip_block_number")
            .0
    }

    pub fn get_current_epoch(&self) -> JsonEpoch {
        self.inner()
            .get_current_epoch()
            .call()
            .expect("rpc call get_current_epoch")
    }

    pub fn get_epoch_by_number(&self, number: EpochNumber) -> Option<JsonEpoch> {
        self.inner()
            .get_epoch_by_number(JsonEpochNumber(number))
            .call()
            .expect("rpc call get_epoch_by_number")
    }

    pub fn local_node_info(&self) -> JsonNode {
        self.inner()
            .local_node_info()
            .call()
            .expect("rpc call local_node_info")
    }

    pub fn get_peers(&self) -> Vec<JsonNode> {
        self.inner().get_peers().call().expect("rpc call get_peers")
    }

    pub fn get_banned_addresses(&self) -> Vec<JsonBannedAddress> {
        self.inner()
            .get_banned_addresses()
            .call()
            .expect("rpc call get_banned_addresses")
    }

    pub fn set_ban(
        &self,
        address: String,
        command: String,
        ban_time: Option<u64>,
        absolute: Option<bool>,
        reason: Option<String>,
    ) {
        self.inner()
            .set_ban(
                address,
                command,
                ban_time.map(JsonTimestamp),
                absolute,
                reason,
            )
            .call()
            .expect("rpc call set_ban")
    }

    pub fn get_block_template(
        &self,
        bytes_limit: Option<u64>,
        proposals_limit: Option<u64>,
        max_version: Option<Version>,
    ) -> JsonBlockTemplate {
        let bytes_limit = bytes_limit.map(JsonUnsigned);
        let proposals_limit = proposals_limit.map(JsonUnsigned);
        let max_version = max_version.map(JsonVersion);
        self.inner()
            .get_block_template(bytes_limit, proposals_limit, max_version)
            .call()
            .expect("rpc call get_block_template")
    }

    pub fn submit_block(&self, work_id: String, block: &Block) -> Option<H256> {
        self.inner()
            .submit_block(work_id, block.into())
            .call()
            .expect("rpc call submit_block")
    }

    pub fn get_blockchain_info(&self) -> JsonChainInfo {
        self.inner()
            .get_blockchain_info()
            .call()
            .expect("rpc call get_blockchain_info")
    }

    pub fn send_transaction(&self, tx: &Transaction) -> H256 {
        self.inner()
            .send_transaction(tx.into())
            .call()
            .expect("rpc call send_transaction")
    }

    pub fn send_transaction_result(&self, tx: &Transaction) -> JsonRpcResult<H256> {
        self.inner().send_transaction(tx.into()).call()
    }

    pub fn send_alert(&self, alert: Alert) {
        self.inner()
            .send_alert(alert.into())
            .call()
            .expect("rpc call send_alert")
    }

    pub fn tx_pool_info(&self) -> JsonTxPoolInfo {
        self.inner()
            .tx_pool_info()
            .call()
            .expect("rpc call tx_pool_info")
    }

    pub fn add_node(&self, peer_id: String, address: String) {
        self.inner()
            .add_node(peer_id, address)
            .call()
            .expect("rpc call add_node");
    }

    pub fn remove_node(&self, peer_id: String) {
        self.inner()
            .remove_node(peer_id)
            .call()
            .expect("rpc call remove_node")
    }

    pub fn process_block_without_verify(&self, block: &Block) -> H256 {
        self.inner()
            .process_block_without_verify(block.into())
            .call()
            .expect("rpc call process_block_without verify")
            .expect("process_block_without should be ok")
    }

    pub fn get_live_cells_by_lock_hash(
        &self,
        lock_hash: H256,
        page: u64,
        per_page: u64,
        reverse_order: Option<bool>,
    ) -> Vec<JsonLiveCell> {
        self.inner()
            .get_live_cells_by_lock_hash(
                lock_hash,
                JsonUnsigned(page),
                JsonUnsigned(per_page),
                reverse_order,
            )
            .call()
            .expect("rpc call get_live_cells_by_lock_hash")
    }

    pub fn get_transactions_by_lock_hash(
        &self,
        lock_hash: H256,
        page: u64,
        per_page: u64,
        reverse_order: Option<bool>,
    ) -> Vec<JsonCellTransaction> {
        self.inner()
            .get_transactions_by_lock_hash(
                lock_hash,
                JsonUnsigned(page),
                JsonUnsigned(per_page),
                reverse_order,
            )
            .call()
            .expect("rpc call get_transactions_by_lock_hash")
    }

    pub fn index_lock_hash(
        &self,
        lock_hash: H256,
        index_from: Option<BlockNumber>,
    ) -> JsonLockHashIndexState {
        self.inner()
            .index_lock_hash(lock_hash, index_from.map(JsonBlockNumber))
            .call()
            .expect("rpc call index_lock_hash")
    }

    pub fn deindex_lock_hash(&self, lock_hash: H256) {
        self.inner()
            .deindex_lock_hash(lock_hash)
            .call()
            .expect("rpc call deindex_lock_hash")
    }

    pub fn get_lock_hash_index_states(&self) -> Vec<JsonLockHashIndexState> {
        self.inner()
            .get_lock_hash_index_states()
            .call()
            .expect("rpc call get_lock_hash_index_states")
    }

    pub fn calculate_dao_maximum_withdraw(&self, out_point: OutPoint, hash: H256) -> Capacity {
        self.inner()
            .calculate_dao_maximum_withdraw(out_point.into(), hash)
            .call()
            .expect("rpc call calculate_dao_maximum_withdraw")
            .0
    }
}

jsonrpc_client!(pub struct Inner {
    pub fn get_block(&mut self, _hash: H256) -> RpcRequest<Option<JsonBlock>>;
    pub fn get_block_by_number(&mut self, _number: JsonBlockNumber) -> RpcRequest<Option<JsonBlock>>;
    pub fn get_header(&mut self, _hash: H256) -> RpcRequest<Option<JsonHeader>>;
    pub fn get_header_by_number(&mut self, _number: JsonBlockNumber) -> RpcRequest<Option<JsonHeader>>;
    pub fn get_transaction(&mut self, _hash: H256) -> RpcRequest<Option<JsonTransactionWithStatus>>;
    pub fn get_block_hash(&mut self, _number: JsonBlockNumber) -> RpcRequest<Option<H256>>;
    pub fn get_tip_header(&mut self) -> RpcRequest<JsonHeader>;
    pub fn get_cells_by_lock_hash(
        &mut self,
        _lock_hash: H256,
        _from: JsonBlockNumber,
        _to: JsonBlockNumber
    ) -> RpcRequest<Vec<JsonCellOutputWithOutPoint>>;
    pub fn get_live_cell(&mut self, _out_point: JsonOutPoint) -> RpcRequest<JsonCellWithStatus>;
    pub fn get_tip_block_number(&mut self) -> RpcRequest<JsonBlockNumber>;
    pub fn get_current_epoch(&mut self) -> RpcRequest<JsonEpoch>;
    pub fn get_epoch_by_number(&mut self, number: JsonEpochNumber) -> RpcRequest<Option<JsonEpoch>>;

    pub fn local_node_info(&mut self) -> RpcRequest<JsonNode>;
    pub fn get_peers(&mut self) -> RpcRequest<Vec<JsonNode>>;
    pub fn get_banned_addresses(&mut self) -> RpcRequest<Vec<JsonBannedAddress>>;
    pub fn set_ban(
        &mut self,
        address: String,
        command: String,
        ban_time: Option<JsonTimestamp>,
        absolute: Option<bool>,
        reason: Option<String>
    ) -> RpcRequest<()>;

    pub fn get_block_template(
        &mut self,
        bytes_limit: Option<JsonUnsigned>,
        proposals_limit: Option<JsonUnsigned>,
        max_version: Option<JsonVersion>
    ) -> RpcRequest<JsonBlockTemplate>;
    pub fn submit_block(&mut self, _work_id: String, _data: JsonBlock) -> RpcRequest<Option<H256>>;
    pub fn get_blockchain_info(&mut self) -> RpcRequest<JsonChainInfo>;
    pub fn get_peers_state(&mut self) -> RpcRequest<Vec<JsonPeerState>>;
    pub fn compute_transaction_hash(&mut self, tx: JsonTransaction) -> RpcRequest<H256>;
    pub fn dry_run_transaction(&mut self, _tx: JsonTransaction) -> RpcRequest<JsonDryRunResult>;
    pub fn send_transaction(&mut self, tx: JsonTransaction) -> RpcRequest<H256>;
    pub fn tx_pool_info(&mut self) -> RpcRequest<JsonTxPoolInfo>;

    pub fn send_alert(&mut self, alert: JsonAlert) -> RpcRequest<()>;

    pub fn add_node(&mut self, peer_id: String, address: String) -> RpcRequest<()>;
    pub fn remove_node(&mut self, peer_id: String) -> RpcRequest<()>;
    pub fn process_block_without_verify(&mut self, _data: JsonBlock) -> RpcRequest<Option<H256>>;

    pub fn get_live_cells_by_lock_hash(&mut self, lock_hash: H256, page: JsonUnsigned, per_page: JsonUnsigned, reverse_order: Option<bool>) -> RpcRequest<Vec<JsonLiveCell>>;
    pub fn get_transactions_by_lock_hash(&mut self, lock_hash: H256, page: JsonUnsigned, per_page: JsonUnsigned, reverse_order: Option<bool>) -> RpcRequest<Vec<JsonCellTransaction>>;
    pub fn index_lock_hash(&mut self, lock_hash: H256, index_from: Option<JsonBlockNumber>) -> RpcRequest<JsonLockHashIndexState>;
    pub fn deindex_lock_hash(&mut self, lock_hash: H256) -> RpcRequest<()>;
    pub fn get_lock_hash_index_states(&mut self) -> RpcRequest<Vec<JsonLockHashIndexState>>;
    pub fn calculate_dao_maximum_withdraw(&mut self, _out_point: JsonOutPoint, _hash: H256) -> RpcRequest<JsonCapacity>;
});
