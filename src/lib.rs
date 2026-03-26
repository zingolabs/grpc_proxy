//! A gRPC proxy implementing the `CompactTxStreamer` service.
//!
//! Provides two mock implementations:
//!
//! - [`MockCompactTxStreamer`] — every method panics with `unimplemented!()`.
//! - [`ConfigurableMockStreamer`] — per-method behavior configured via [`MockConfig`].

use zaino_proto::tonic;

use zaino_proto::proto::{
    compact_formats::{CompactBlock, CompactTx},
    service::{
        Address, AddressList, Balance, BlockId, BlockRange, ChainSpec, Duration, Empty,
        GetAddressUtxosArg, GetAddressUtxosReply, GetAddressUtxosReplyList, GetMempoolTxRequest,
        GetSubtreeRootsArg, LightdInfo, PingResponse, RawTransaction, SendResponse, SubtreeRoot,
        TransparentAddressBlockFilter, TreeState, TxFilter,
        compact_tx_streamer_server::CompactTxStreamer,
    },
};

/// Re-export for consumers that need to build a `tonic::transport::Server`.
pub use zaino_proto::proto::service::compact_tx_streamer_server::CompactTxStreamerServer;

/// Re-export so consumers can use `zingo_grpc_proxy::tonic_reexport::Code` etc.
/// without adding tonic as a direct dependency.
pub use zaino_proto::tonic as tonic_reexport;

// ---------------------------------------------------------------------------
// MethodBehavior
// ---------------------------------------------------------------------------

/// Describes how a single gRPC method should behave in the mock.
#[derive(Clone, Debug)]
pub enum MethodBehavior {
    /// Return `Err(tonic::Status)` with the given code and message.
    Error(tonic::Code, String),
    /// Sleep forever — simulates a hung/unresponsive server.
    Hang,
    /// Panic with `unimplemented!()` — the original default.
    Unimplemented,
}

async fn apply_behavior(name: &str, behavior: &MethodBehavior) -> tonic::Status {
    match behavior {
        MethodBehavior::Error(code, msg) => tonic::Status::new(*code, msg.clone()),
        MethodBehavior::Hang => {
            std::future::pending::<()>().await;
            unreachable!()
        }
        MethodBehavior::Unimplemented => unimplemented!("{name}"),
    }
}

// ---------------------------------------------------------------------------
// MockConfig
// ---------------------------------------------------------------------------

/// Per-method configuration for [`ConfigurableMockStreamer`].
#[derive(Clone, Debug)]
pub struct MockConfig {
    pub get_latest_block: MethodBehavior,
    pub get_block: MethodBehavior,
    pub get_block_nullifiers: MethodBehavior,
    pub get_block_range: MethodBehavior,
    pub get_block_range_nullifiers: MethodBehavior,
    pub get_transaction: MethodBehavior,
    pub send_transaction: MethodBehavior,
    pub get_taddress_txids: MethodBehavior,
    pub get_taddress_transactions: MethodBehavior,
    pub get_taddress_balance: MethodBehavior,
    pub get_taddress_balance_stream: MethodBehavior,
    pub get_mempool_tx: MethodBehavior,
    pub get_mempool_stream: MethodBehavior,
    pub get_tree_state: MethodBehavior,
    pub get_latest_tree_state: MethodBehavior,
    pub get_subtree_roots: MethodBehavior,
    pub get_address_utxos: MethodBehavior,
    pub get_address_utxos_stream: MethodBehavior,
    pub get_lightd_info: MethodBehavior,
    pub ping: MethodBehavior,
}

impl MockConfig {
    /// All methods panic with `unimplemented!()` (backward-compatible default).
    pub fn unimplemented() -> Self {
        Self {
            get_latest_block: MethodBehavior::Unimplemented,
            get_block: MethodBehavior::Unimplemented,
            get_block_nullifiers: MethodBehavior::Unimplemented,
            get_block_range: MethodBehavior::Unimplemented,
            get_block_range_nullifiers: MethodBehavior::Unimplemented,
            get_transaction: MethodBehavior::Unimplemented,
            send_transaction: MethodBehavior::Unimplemented,
            get_taddress_txids: MethodBehavior::Unimplemented,
            get_taddress_transactions: MethodBehavior::Unimplemented,
            get_taddress_balance: MethodBehavior::Unimplemented,
            get_taddress_balance_stream: MethodBehavior::Unimplemented,
            get_mempool_tx: MethodBehavior::Unimplemented,
            get_mempool_stream: MethodBehavior::Unimplemented,
            get_tree_state: MethodBehavior::Unimplemented,
            get_latest_tree_state: MethodBehavior::Unimplemented,
            get_subtree_roots: MethodBehavior::Unimplemented,
            get_address_utxos: MethodBehavior::Unimplemented,
            get_address_utxos_stream: MethodBehavior::Unimplemented,
            get_lightd_info: MethodBehavior::Unimplemented,
            ping: MethodBehavior::Unimplemented,
        }
    }

    /// All methods return the same gRPC error.
    pub fn all_error(code: tonic::Code, message: impl Into<String>) -> Self {
        let msg = message.into();
        Self {
            get_latest_block: MethodBehavior::Error(code, msg.clone()),
            get_block: MethodBehavior::Error(code, msg.clone()),
            get_block_nullifiers: MethodBehavior::Error(code, msg.clone()),
            get_block_range: MethodBehavior::Error(code, msg.clone()),
            get_block_range_nullifiers: MethodBehavior::Error(code, msg.clone()),
            get_transaction: MethodBehavior::Error(code, msg.clone()),
            send_transaction: MethodBehavior::Error(code, msg.clone()),
            get_taddress_txids: MethodBehavior::Error(code, msg.clone()),
            get_taddress_transactions: MethodBehavior::Error(code, msg.clone()),
            get_taddress_balance: MethodBehavior::Error(code, msg.clone()),
            get_taddress_balance_stream: MethodBehavior::Error(code, msg.clone()),
            get_mempool_tx: MethodBehavior::Error(code, msg.clone()),
            get_mempool_stream: MethodBehavior::Error(code, msg.clone()),
            get_tree_state: MethodBehavior::Error(code, msg.clone()),
            get_latest_tree_state: MethodBehavior::Error(code, msg.clone()),
            get_subtree_roots: MethodBehavior::Error(code, msg.clone()),
            get_address_utxos: MethodBehavior::Error(code, msg.clone()),
            get_address_utxos_stream: MethodBehavior::Error(code, msg.clone()),
            get_lightd_info: MethodBehavior::Error(code, msg.clone()),
            ping: MethodBehavior::Error(code, msg),
        }
    }

    // Builder-style overrides ------------------------------------------------

    pub fn with_get_latest_block(mut self, b: MethodBehavior) -> Self {
        self.get_latest_block = b;
        self
    }
    pub fn with_get_block(mut self, b: MethodBehavior) -> Self {
        self.get_block = b;
        self
    }
    pub fn with_get_block_nullifiers(mut self, b: MethodBehavior) -> Self {
        self.get_block_nullifiers = b;
        self
    }
    pub fn with_get_block_range(mut self, b: MethodBehavior) -> Self {
        self.get_block_range = b;
        self
    }
    pub fn with_get_block_range_nullifiers(mut self, b: MethodBehavior) -> Self {
        self.get_block_range_nullifiers = b;
        self
    }
    pub fn with_get_transaction(mut self, b: MethodBehavior) -> Self {
        self.get_transaction = b;
        self
    }
    pub fn with_send_transaction(mut self, b: MethodBehavior) -> Self {
        self.send_transaction = b;
        self
    }
    pub fn with_get_taddress_txids(mut self, b: MethodBehavior) -> Self {
        self.get_taddress_txids = b;
        self
    }
    pub fn with_get_taddress_transactions(mut self, b: MethodBehavior) -> Self {
        self.get_taddress_transactions = b;
        self
    }
    pub fn with_get_taddress_balance(mut self, b: MethodBehavior) -> Self {
        self.get_taddress_balance = b;
        self
    }
    pub fn with_get_taddress_balance_stream(mut self, b: MethodBehavior) -> Self {
        self.get_taddress_balance_stream = b;
        self
    }
    pub fn with_get_mempool_tx(mut self, b: MethodBehavior) -> Self {
        self.get_mempool_tx = b;
        self
    }
    pub fn with_get_mempool_stream(mut self, b: MethodBehavior) -> Self {
        self.get_mempool_stream = b;
        self
    }
    pub fn with_get_tree_state(mut self, b: MethodBehavior) -> Self {
        self.get_tree_state = b;
        self
    }
    pub fn with_get_latest_tree_state(mut self, b: MethodBehavior) -> Self {
        self.get_latest_tree_state = b;
        self
    }
    pub fn with_get_subtree_roots(mut self, b: MethodBehavior) -> Self {
        self.get_subtree_roots = b;
        self
    }
    pub fn with_get_address_utxos(mut self, b: MethodBehavior) -> Self {
        self.get_address_utxos = b;
        self
    }
    pub fn with_get_address_utxos_stream(mut self, b: MethodBehavior) -> Self {
        self.get_address_utxos_stream = b;
        self
    }
    pub fn with_get_lightd_info(mut self, b: MethodBehavior) -> Self {
        self.get_lightd_info = b;
        self
    }
    pub fn with_ping(mut self, b: MethodBehavior) -> Self {
        self.ping = b;
        self
    }
}

// ---------------------------------------------------------------------------
// ConfigurableMockStreamer
// ---------------------------------------------------------------------------

/// A configurable mock of the `CompactTxStreamer` gRPC service.
///
/// Each method delegates to [`apply_behavior`] using the corresponding
/// [`MockConfig`] field, allowing per-method control of errors, hangs,
/// or panics.
pub struct ConfigurableMockStreamer {
    config: MockConfig,
}

impl ConfigurableMockStreamer {
    pub fn new(config: MockConfig) -> Self {
        Self { config }
    }
}

/// A mock implementation of the `CompactTxStreamer` gRPC service.
///
/// Every method is `unimplemented!()`. The purpose of this server is to
/// detect unexpected indexer contact during offline wallet operations.
pub struct MockCompactTxStreamer;

type Stream<T> = std::pin::Pin<
    Box<dyn tonic::codegen::tokio_stream::Stream<Item = Result<T, tonic::Status>> + Send>,
>;

#[tonic::async_trait]
impl CompactTxStreamer for MockCompactTxStreamer {
    async fn get_latest_block(
        &self,
        _request: tonic::Request<ChainSpec>,
    ) -> Result<tonic::Response<BlockId>, tonic::Status> {
        unimplemented!("get_latest_block")
    }

    async fn get_block(
        &self,
        _request: tonic::Request<BlockId>,
    ) -> Result<tonic::Response<CompactBlock>, tonic::Status> {
        unimplemented!("get_block")
    }

    async fn get_block_nullifiers(
        &self,
        _request: tonic::Request<BlockId>,
    ) -> Result<tonic::Response<CompactBlock>, tonic::Status> {
        unimplemented!("get_block_nullifiers")
    }

    type GetBlockRangeStream = Stream<CompactBlock>;

    async fn get_block_range(
        &self,
        _request: tonic::Request<BlockRange>,
    ) -> Result<tonic::Response<Self::GetBlockRangeStream>, tonic::Status> {
        unimplemented!("get_block_range")
    }

    type GetBlockRangeNullifiersStream = Stream<CompactBlock>;

    async fn get_block_range_nullifiers(
        &self,
        _request: tonic::Request<BlockRange>,
    ) -> Result<tonic::Response<Self::GetBlockRangeNullifiersStream>, tonic::Status> {
        unimplemented!("get_block_range_nullifiers")
    }

    async fn get_transaction(
        &self,
        _request: tonic::Request<TxFilter>,
    ) -> Result<tonic::Response<RawTransaction>, tonic::Status> {
        unimplemented!("get_transaction")
    }

    async fn send_transaction(
        &self,
        _request: tonic::Request<RawTransaction>,
    ) -> Result<tonic::Response<SendResponse>, tonic::Status> {
        unimplemented!("send_transaction")
    }

    type GetTaddressTxidsStream = Stream<RawTransaction>;

    async fn get_taddress_txids(
        &self,
        _request: tonic::Request<TransparentAddressBlockFilter>,
    ) -> Result<tonic::Response<Self::GetTaddressTxidsStream>, tonic::Status> {
        unimplemented!("get_taddress_txids")
    }

    type GetTaddressTransactionsStream = Stream<RawTransaction>;

    async fn get_taddress_transactions(
        &self,
        _request: tonic::Request<TransparentAddressBlockFilter>,
    ) -> Result<tonic::Response<Self::GetTaddressTransactionsStream>, tonic::Status> {
        unimplemented!("get_taddress_transactions")
    }

    async fn get_taddress_balance(
        &self,
        _request: tonic::Request<AddressList>,
    ) -> Result<tonic::Response<Balance>, tonic::Status> {
        unimplemented!("get_taddress_balance")
    }

    async fn get_taddress_balance_stream(
        &self,
        _request: tonic::Request<tonic::Streaming<Address>>,
    ) -> Result<tonic::Response<Balance>, tonic::Status> {
        unimplemented!("get_taddress_balance_stream")
    }

    type GetMempoolTxStream = Stream<CompactTx>;

    async fn get_mempool_tx(
        &self,
        _request: tonic::Request<GetMempoolTxRequest>,
    ) -> Result<tonic::Response<Self::GetMempoolTxStream>, tonic::Status> {
        unimplemented!("get_mempool_tx")
    }

    type GetMempoolStreamStream = Stream<RawTransaction>;

    async fn get_mempool_stream(
        &self,
        _request: tonic::Request<Empty>,
    ) -> Result<tonic::Response<Self::GetMempoolStreamStream>, tonic::Status> {
        unimplemented!("get_mempool_stream")
    }

    async fn get_tree_state(
        &self,
        _request: tonic::Request<BlockId>,
    ) -> Result<tonic::Response<TreeState>, tonic::Status> {
        unimplemented!("get_tree_state")
    }

    async fn get_latest_tree_state(
        &self,
        _request: tonic::Request<Empty>,
    ) -> Result<tonic::Response<TreeState>, tonic::Status> {
        unimplemented!("get_latest_tree_state")
    }

    type GetSubtreeRootsStream = Stream<SubtreeRoot>;

    async fn get_subtree_roots(
        &self,
        _request: tonic::Request<GetSubtreeRootsArg>,
    ) -> Result<tonic::Response<Self::GetSubtreeRootsStream>, tonic::Status> {
        unimplemented!("get_subtree_roots")
    }

    async fn get_address_utxos(
        &self,
        _request: tonic::Request<GetAddressUtxosArg>,
    ) -> Result<tonic::Response<GetAddressUtxosReplyList>, tonic::Status> {
        unimplemented!("get_address_utxos")
    }

    type GetAddressUtxosStreamStream = Stream<GetAddressUtxosReply>;

    async fn get_address_utxos_stream(
        &self,
        _request: tonic::Request<GetAddressUtxosArg>,
    ) -> Result<tonic::Response<Self::GetAddressUtxosStreamStream>, tonic::Status> {
        unimplemented!("get_address_utxos_stream")
    }

    async fn get_lightd_info(
        &self,
        _request: tonic::Request<Empty>,
    ) -> Result<tonic::Response<LightdInfo>, tonic::Status> {
        unimplemented!("get_lightd_info")
    }

    async fn ping(
        &self,
        _request: tonic::Request<Duration>,
    ) -> Result<tonic::Response<PingResponse>, tonic::Status> {
        unimplemented!("ping")
    }
}

#[tonic::async_trait]
impl CompactTxStreamer for ConfigurableMockStreamer {
    async fn get_latest_block(
        &self,
        _request: tonic::Request<ChainSpec>,
    ) -> Result<tonic::Response<BlockId>, tonic::Status> {
        Err(apply_behavior("get_latest_block", &self.config.get_latest_block).await)
    }

    async fn get_block(
        &self,
        _request: tonic::Request<BlockId>,
    ) -> Result<tonic::Response<CompactBlock>, tonic::Status> {
        Err(apply_behavior("get_block", &self.config.get_block).await)
    }

    async fn get_block_nullifiers(
        &self,
        _request: tonic::Request<BlockId>,
    ) -> Result<tonic::Response<CompactBlock>, tonic::Status> {
        Err(apply_behavior("get_block_nullifiers", &self.config.get_block_nullifiers).await)
    }

    type GetBlockRangeStream = Stream<CompactBlock>;

    async fn get_block_range(
        &self,
        _request: tonic::Request<BlockRange>,
    ) -> Result<tonic::Response<Self::GetBlockRangeStream>, tonic::Status> {
        Err(apply_behavior("get_block_range", &self.config.get_block_range).await)
    }

    type GetBlockRangeNullifiersStream = Stream<CompactBlock>;

    async fn get_block_range_nullifiers(
        &self,
        _request: tonic::Request<BlockRange>,
    ) -> Result<tonic::Response<Self::GetBlockRangeNullifiersStream>, tonic::Status> {
        Err(apply_behavior(
            "get_block_range_nullifiers",
            &self.config.get_block_range_nullifiers,
        )
        .await)
    }

    async fn get_transaction(
        &self,
        _request: tonic::Request<TxFilter>,
    ) -> Result<tonic::Response<RawTransaction>, tonic::Status> {
        Err(apply_behavior("get_transaction", &self.config.get_transaction).await)
    }

    async fn send_transaction(
        &self,
        _request: tonic::Request<RawTransaction>,
    ) -> Result<tonic::Response<SendResponse>, tonic::Status> {
        Err(apply_behavior("send_transaction", &self.config.send_transaction).await)
    }

    type GetTaddressTxidsStream = Stream<RawTransaction>;

    async fn get_taddress_txids(
        &self,
        _request: tonic::Request<TransparentAddressBlockFilter>,
    ) -> Result<tonic::Response<Self::GetTaddressTxidsStream>, tonic::Status> {
        Err(apply_behavior("get_taddress_txids", &self.config.get_taddress_txids).await)
    }

    type GetTaddressTransactionsStream = Stream<RawTransaction>;

    async fn get_taddress_transactions(
        &self,
        _request: tonic::Request<TransparentAddressBlockFilter>,
    ) -> Result<tonic::Response<Self::GetTaddressTransactionsStream>, tonic::Status> {
        Err(apply_behavior(
            "get_taddress_transactions",
            &self.config.get_taddress_transactions,
        )
        .await)
    }

    async fn get_taddress_balance(
        &self,
        _request: tonic::Request<AddressList>,
    ) -> Result<tonic::Response<Balance>, tonic::Status> {
        Err(apply_behavior("get_taddress_balance", &self.config.get_taddress_balance).await)
    }

    async fn get_taddress_balance_stream(
        &self,
        _request: tonic::Request<tonic::Streaming<Address>>,
    ) -> Result<tonic::Response<Balance>, tonic::Status> {
        Err(apply_behavior(
            "get_taddress_balance_stream",
            &self.config.get_taddress_balance_stream,
        )
        .await)
    }

    type GetMempoolTxStream = Stream<CompactTx>;

    async fn get_mempool_tx(
        &self,
        _request: tonic::Request<GetMempoolTxRequest>,
    ) -> Result<tonic::Response<Self::GetMempoolTxStream>, tonic::Status> {
        Err(apply_behavior("get_mempool_tx", &self.config.get_mempool_tx).await)
    }

    type GetMempoolStreamStream = Stream<RawTransaction>;

    async fn get_mempool_stream(
        &self,
        _request: tonic::Request<Empty>,
    ) -> Result<tonic::Response<Self::GetMempoolStreamStream>, tonic::Status> {
        Err(apply_behavior("get_mempool_stream", &self.config.get_mempool_stream).await)
    }

    async fn get_tree_state(
        &self,
        _request: tonic::Request<BlockId>,
    ) -> Result<tonic::Response<TreeState>, tonic::Status> {
        Err(apply_behavior("get_tree_state", &self.config.get_tree_state).await)
    }

    async fn get_latest_tree_state(
        &self,
        _request: tonic::Request<Empty>,
    ) -> Result<tonic::Response<TreeState>, tonic::Status> {
        Err(apply_behavior("get_latest_tree_state", &self.config.get_latest_tree_state).await)
    }

    type GetSubtreeRootsStream = Stream<SubtreeRoot>;

    async fn get_subtree_roots(
        &self,
        _request: tonic::Request<GetSubtreeRootsArg>,
    ) -> Result<tonic::Response<Self::GetSubtreeRootsStream>, tonic::Status> {
        Err(apply_behavior("get_subtree_roots", &self.config.get_subtree_roots).await)
    }

    async fn get_address_utxos(
        &self,
        _request: tonic::Request<GetAddressUtxosArg>,
    ) -> Result<tonic::Response<GetAddressUtxosReplyList>, tonic::Status> {
        Err(apply_behavior("get_address_utxos", &self.config.get_address_utxos).await)
    }

    type GetAddressUtxosStreamStream = Stream<GetAddressUtxosReply>;

    async fn get_address_utxos_stream(
        &self,
        _request: tonic::Request<GetAddressUtxosArg>,
    ) -> Result<tonic::Response<Self::GetAddressUtxosStreamStream>, tonic::Status> {
        Err(apply_behavior(
            "get_address_utxos_stream",
            &self.config.get_address_utxos_stream,
        )
        .await)
    }

    async fn get_lightd_info(
        &self,
        _request: tonic::Request<Empty>,
    ) -> Result<tonic::Response<LightdInfo>, tonic::Status> {
        Err(apply_behavior("get_lightd_info", &self.config.get_lightd_info).await)
    }

    async fn ping(
        &self,
        _request: tonic::Request<Duration>,
    ) -> Result<tonic::Response<PingResponse>, tonic::Status> {
        Err(apply_behavior("ping", &self.config.ping).await)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unimplemented_config_sets_all_fields() {
        let cfg = MockConfig::unimplemented();
        for behavior in [
            &cfg.get_latest_block,
            &cfg.get_block,
            &cfg.get_block_nullifiers,
            &cfg.get_block_range,
            &cfg.get_block_range_nullifiers,
            &cfg.get_transaction,
            &cfg.send_transaction,
            &cfg.get_taddress_txids,
            &cfg.get_taddress_transactions,
            &cfg.get_taddress_balance,
            &cfg.get_taddress_balance_stream,
            &cfg.get_mempool_tx,
            &cfg.get_mempool_stream,
            &cfg.get_tree_state,
            &cfg.get_latest_tree_state,
            &cfg.get_subtree_roots,
            &cfg.get_address_utxos,
            &cfg.get_address_utxos_stream,
            &cfg.get_lightd_info,
            &cfg.ping,
        ] {
            assert!(
                matches!(behavior, MethodBehavior::Unimplemented),
                "expected Unimplemented, got {behavior:?}"
            );
        }
    }

    #[test]
    fn all_error_config_sets_all_fields() {
        let cfg = MockConfig::all_error(tonic::Code::DeadlineExceeded, "timeout");
        for behavior in [
            &cfg.get_latest_block,
            &cfg.get_block,
            &cfg.get_block_nullifiers,
            &cfg.get_block_range,
            &cfg.get_block_range_nullifiers,
            &cfg.get_transaction,
            &cfg.send_transaction,
            &cfg.get_taddress_txids,
            &cfg.get_taddress_transactions,
            &cfg.get_taddress_balance,
            &cfg.get_taddress_balance_stream,
            &cfg.get_mempool_tx,
            &cfg.get_mempool_stream,
            &cfg.get_tree_state,
            &cfg.get_latest_tree_state,
            &cfg.get_subtree_roots,
            &cfg.get_address_utxos,
            &cfg.get_address_utxos_stream,
            &cfg.get_lightd_info,
            &cfg.ping,
        ] {
            match behavior {
                MethodBehavior::Error(code, msg) => {
                    assert_eq!(*code, tonic::Code::DeadlineExceeded);
                    assert_eq!(msg, "timeout");
                }
                other => panic!("expected Error, got {other:?}"),
            }
        }
    }

    #[test]
    fn with_override_changes_only_target_field() {
        let cfg = MockConfig::all_error(tonic::Code::Unavailable, "down").with_get_tree_state(
            MethodBehavior::Error(
                tonic::Code::DeadlineExceeded,
                "get_tree_state timeout".into(),
            ),
        );

        // The overridden field
        match &cfg.get_tree_state {
            MethodBehavior::Error(code, msg) => {
                assert_eq!(*code, tonic::Code::DeadlineExceeded);
                assert_eq!(msg, "get_tree_state timeout");
            }
            other => panic!("expected overridden Error, got {other:?}"),
        }

        // A non-overridden field stays at the original
        match &cfg.get_latest_block {
            MethodBehavior::Error(code, msg) => {
                assert_eq!(*code, tonic::Code::Unavailable);
                assert_eq!(msg, "down");
            }
            other => panic!("expected original Error, got {other:?}"),
        }
    }
}
