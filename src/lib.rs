//! A gRPC proxy implementing the `CompactTxStreamer` service.
//!
//! Provides two mock implementations:
//!
//! - [`MockCompactTxStreamer`] — every method panics with `unimplemented!()`.
//! - [`ConfigurableMockStreamer`] — per-method behavior configured via [`MockConfig`].
//!
//! Each [`MockConfig`] field is a [`MethodHandler`] — either a simple
//! [`MethodBehavior`] (error / hang / unimplemented) or an arbitrary async
//! callback that can return successful responses.

use std::sync::Arc;

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

type Stream<T> = std::pin::Pin<
    Box<dyn tonic::codegen::tokio_stream::Stream<Item = Result<T, tonic::Status>> + Send>,
>;

// ---------------------------------------------------------------------------
// MethodBehavior
// ---------------------------------------------------------------------------

/// Describes how a single gRPC method should behave in the mock.
///
/// This is a convenience type — it converts automatically into a
/// [`MethodHandler`] via [`IntoHandler`] when passed to the `MockConfig`
/// builder methods.
#[derive(Clone, Debug)]
pub enum MethodBehavior {
    /// Return `Err(tonic::Status)` with the given code and message.
    Error(tonic::Code, String),
    /// Sleep forever — simulates a hung/unresponsive server.
    Hang,
    /// Panic with `unimplemented!()` — the original default.
    Unimplemented,
}

// ---------------------------------------------------------------------------
// MethodHandler
// ---------------------------------------------------------------------------

type BoxFuture<T> = std::pin::Pin<Box<dyn std::future::Future<Output = T> + Send>>;

type HandlerFn<Req, Resp> = dyn Fn(tonic::Request<Req>) -> BoxFuture<Result<tonic::Response<Resp>, tonic::Status>>
    + Send
    + Sync;

/// A per-method handler for [`ConfigurableMockStreamer`].
///
/// Wraps an async callback that receives the gRPC request and returns either
/// a successful response or an error status. Create one via:
///
/// - [`MethodHandler::from_fn`] — full control with an async closure.
/// - [`MethodHandler::from_response`] — always returns a fixed successful response.
/// - Passing a [`MethodBehavior`] to a `MockConfig` builder method (auto-converts).
pub struct MethodHandler<Req: 'static, Resp: 'static> {
    f: Arc<HandlerFn<Req, Resp>>,
}

impl<Req, Resp> Clone for MethodHandler<Req, Resp> {
    fn clone(&self) -> Self {
        Self {
            f: Arc::clone(&self.f),
        }
    }
}

impl<Req, Resp> std::fmt::Debug for MethodHandler<Req, Resp> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("MethodHandler(..)")
    }
}

impl<Req: Send + 'static, Resp: Send + 'static> MethodHandler<Req, Resp> {
    /// Create a handler from an arbitrary async function.
    ///
    /// ```ignore
    /// MethodHandler::from_fn(|_req| async {
    ///     Ok(tonic::Response::new(BlockId::default()))
    /// })
    /// ```
    pub fn from_fn<F, Fut>(f: F) -> Self
    where
        F: Fn(tonic::Request<Req>) -> Fut + Send + Sync + 'static,
        Fut: std::future::Future<Output = Result<tonic::Response<Resp>, tonic::Status>>
            + Send
            + 'static,
    {
        Self {
            f: Arc::new(move |req| Box::pin(f(req))),
        }
    }

    /// Create a handler that always returns the given response (cloned per call).
    pub fn from_response(resp: Resp) -> Self
    where
        Resp: Clone + Sync,
    {
        Self {
            f: Arc::new(move |_req| {
                let resp = resp.clone();
                Box::pin(async move { Ok(tonic::Response::new(resp)) })
            }),
        }
    }

    fn from_behavior(behavior: MethodBehavior, name: &'static str) -> Self {
        Self {
            f: Arc::new(move |_req| {
                let behavior = behavior.clone();
                Box::pin(async move {
                    match &behavior {
                        MethodBehavior::Error(code, msg) => {
                            Err(tonic::Status::new(*code, msg.clone()))
                        }
                        MethodBehavior::Hang => {
                            std::future::pending::<()>().await;
                            unreachable!()
                        }
                        MethodBehavior::Unimplemented => unimplemented!("{name}"),
                    }
                })
            }),
        }
    }

    async fn call(&self, req: tonic::Request<Req>) -> Result<tonic::Response<Resp>, tonic::Status> {
        (self.f)(req).await
    }
}

// ---------------------------------------------------------------------------
// IntoHandler
// ---------------------------------------------------------------------------

/// Conversion trait so that both [`MethodBehavior`] and [`MethodHandler`] can
/// be passed to the `MockConfig::with_*` builder methods.
pub trait IntoHandler<Req: Send + 'static, Resp: Send + 'static> {
    fn into_handler(self, name: &'static str) -> MethodHandler<Req, Resp>;
}

impl<Req: Send + 'static, Resp: Send + 'static> IntoHandler<Req, Resp> for MethodBehavior {
    fn into_handler(self, name: &'static str) -> MethodHandler<Req, Resp> {
        MethodHandler::from_behavior(self, name)
    }
}

impl<Req: Send + 'static, Resp: Send + 'static> IntoHandler<Req, Resp>
    for MethodHandler<Req, Resp>
{
    fn into_handler(self, _name: &'static str) -> MethodHandler<Req, Resp> {
        self
    }
}

// ---------------------------------------------------------------------------
// MockConfig
// ---------------------------------------------------------------------------

/// Per-method configuration for [`ConfigurableMockStreamer`].
///
/// Each field is a [`MethodHandler`] that receives the gRPC request and
/// produces either a successful response or an error. Use the constructors
/// for common patterns and the `with_*` builders for per-method overrides.
pub struct MockConfig {
    pub get_latest_block: MethodHandler<ChainSpec, BlockId>,
    pub get_block: MethodHandler<BlockId, CompactBlock>,
    pub get_block_nullifiers: MethodHandler<BlockId, CompactBlock>,
    pub get_block_range: MethodHandler<BlockRange, Stream<CompactBlock>>,
    pub get_block_range_nullifiers: MethodHandler<BlockRange, Stream<CompactBlock>>,
    pub get_transaction: MethodHandler<TxFilter, RawTransaction>,
    pub send_transaction: MethodHandler<RawTransaction, SendResponse>,
    pub get_taddress_txids: MethodHandler<TransparentAddressBlockFilter, Stream<RawTransaction>>,
    pub get_taddress_transactions:
        MethodHandler<TransparentAddressBlockFilter, Stream<RawTransaction>>,
    pub get_taddress_balance: MethodHandler<AddressList, Balance>,
    pub get_taddress_balance_stream: MethodHandler<tonic::Streaming<Address>, Balance>,
    pub get_mempool_tx: MethodHandler<GetMempoolTxRequest, Stream<CompactTx>>,
    pub get_mempool_stream: MethodHandler<Empty, Stream<RawTransaction>>,
    pub get_tree_state: MethodHandler<BlockId, TreeState>,
    pub get_latest_tree_state: MethodHandler<Empty, TreeState>,
    pub get_subtree_roots: MethodHandler<GetSubtreeRootsArg, Stream<SubtreeRoot>>,
    pub get_address_utxos: MethodHandler<GetAddressUtxosArg, GetAddressUtxosReplyList>,
    pub get_address_utxos_stream: MethodHandler<GetAddressUtxosArg, Stream<GetAddressUtxosReply>>,
    pub get_lightd_info: MethodHandler<Empty, LightdInfo>,
    pub ping: MethodHandler<Duration, PingResponse>,
}

impl Clone for MockConfig {
    fn clone(&self) -> Self {
        Self {
            get_latest_block: self.get_latest_block.clone(),
            get_block: self.get_block.clone(),
            get_block_nullifiers: self.get_block_nullifiers.clone(),
            get_block_range: self.get_block_range.clone(),
            get_block_range_nullifiers: self.get_block_range_nullifiers.clone(),
            get_transaction: self.get_transaction.clone(),
            send_transaction: self.send_transaction.clone(),
            get_taddress_txids: self.get_taddress_txids.clone(),
            get_taddress_transactions: self.get_taddress_transactions.clone(),
            get_taddress_balance: self.get_taddress_balance.clone(),
            get_taddress_balance_stream: self.get_taddress_balance_stream.clone(),
            get_mempool_tx: self.get_mempool_tx.clone(),
            get_mempool_stream: self.get_mempool_stream.clone(),
            get_tree_state: self.get_tree_state.clone(),
            get_latest_tree_state: self.get_latest_tree_state.clone(),
            get_subtree_roots: self.get_subtree_roots.clone(),
            get_address_utxos: self.get_address_utxos.clone(),
            get_address_utxos_stream: self.get_address_utxos_stream.clone(),
            get_lightd_info: self.get_lightd_info.clone(),
            ping: self.ping.clone(),
        }
    }
}

impl std::fmt::Debug for MockConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MockConfig").finish_non_exhaustive()
    }
}

/// Helper: create a `MethodHandler` from a `MethodBehavior` for a given method name.
macro_rules! behavior_handler {
    ($behavior:expr, $name:literal) => {
        MethodHandler::from_behavior($behavior, $name)
    };
}

impl MockConfig {
    /// All methods panic with `unimplemented!()` (backward-compatible default).
    pub fn unimplemented() -> Self {
        let b = MethodBehavior::Unimplemented;
        Self {
            get_latest_block: behavior_handler!(b.clone(), "get_latest_block"),
            get_block: behavior_handler!(b.clone(), "get_block"),
            get_block_nullifiers: behavior_handler!(b.clone(), "get_block_nullifiers"),
            get_block_range: behavior_handler!(b.clone(), "get_block_range"),
            get_block_range_nullifiers: behavior_handler!(b.clone(), "get_block_range_nullifiers"),
            get_transaction: behavior_handler!(b.clone(), "get_transaction"),
            send_transaction: behavior_handler!(b.clone(), "send_transaction"),
            get_taddress_txids: behavior_handler!(b.clone(), "get_taddress_txids"),
            get_taddress_transactions: behavior_handler!(b.clone(), "get_taddress_transactions"),
            get_taddress_balance: behavior_handler!(b.clone(), "get_taddress_balance"),
            get_taddress_balance_stream: behavior_handler!(
                b.clone(),
                "get_taddress_balance_stream"
            ),
            get_mempool_tx: behavior_handler!(b.clone(), "get_mempool_tx"),
            get_mempool_stream: behavior_handler!(b.clone(), "get_mempool_stream"),
            get_tree_state: behavior_handler!(b.clone(), "get_tree_state"),
            get_latest_tree_state: behavior_handler!(b.clone(), "get_latest_tree_state"),
            get_subtree_roots: behavior_handler!(b.clone(), "get_subtree_roots"),
            get_address_utxos: behavior_handler!(b.clone(), "get_address_utxos"),
            get_address_utxos_stream: behavior_handler!(b.clone(), "get_address_utxos_stream"),
            get_lightd_info: behavior_handler!(b.clone(), "get_lightd_info"),
            ping: behavior_handler!(b, "ping"),
        }
    }

    /// All methods return the same gRPC error.
    pub fn all_error(code: tonic::Code, message: impl Into<String>) -> Self {
        let msg = message.into();
        macro_rules! err {
            ($name:literal) => {
                behavior_handler!(MethodBehavior::Error(code, msg.clone()), $name)
            };
        }
        Self {
            get_latest_block: err!("get_latest_block"),
            get_block: err!("get_block"),
            get_block_nullifiers: err!("get_block_nullifiers"),
            get_block_range: err!("get_block_range"),
            get_block_range_nullifiers: err!("get_block_range_nullifiers"),
            get_transaction: err!("get_transaction"),
            send_transaction: err!("send_transaction"),
            get_taddress_txids: err!("get_taddress_txids"),
            get_taddress_transactions: err!("get_taddress_transactions"),
            get_taddress_balance: err!("get_taddress_balance"),
            get_taddress_balance_stream: err!("get_taddress_balance_stream"),
            get_mempool_tx: err!("get_mempool_tx"),
            get_mempool_stream: err!("get_mempool_stream"),
            get_tree_state: err!("get_tree_state"),
            get_latest_tree_state: err!("get_latest_tree_state"),
            get_subtree_roots: err!("get_subtree_roots"),
            get_address_utxos: err!("get_address_utxos"),
            get_address_utxos_stream: err!("get_address_utxos_stream"),
            get_lightd_info: err!("get_lightd_info"),
            ping: err!("ping"),
        }
    }

    // Builder-style overrides ------------------------------------------------

    pub fn with_get_latest_block(mut self, h: impl IntoHandler<ChainSpec, BlockId>) -> Self {
        self.get_latest_block = h.into_handler("get_latest_block");
        self
    }
    pub fn with_get_block(mut self, h: impl IntoHandler<BlockId, CompactBlock>) -> Self {
        self.get_block = h.into_handler("get_block");
        self
    }
    pub fn with_get_block_nullifiers(mut self, h: impl IntoHandler<BlockId, CompactBlock>) -> Self {
        self.get_block_nullifiers = h.into_handler("get_block_nullifiers");
        self
    }
    pub fn with_get_block_range(
        mut self,
        h: impl IntoHandler<BlockRange, Stream<CompactBlock>>,
    ) -> Self {
        self.get_block_range = h.into_handler("get_block_range");
        self
    }
    pub fn with_get_block_range_nullifiers(
        mut self,
        h: impl IntoHandler<BlockRange, Stream<CompactBlock>>,
    ) -> Self {
        self.get_block_range_nullifiers = h.into_handler("get_block_range_nullifiers");
        self
    }
    pub fn with_get_transaction(mut self, h: impl IntoHandler<TxFilter, RawTransaction>) -> Self {
        self.get_transaction = h.into_handler("get_transaction");
        self
    }
    pub fn with_send_transaction(
        mut self,
        h: impl IntoHandler<RawTransaction, SendResponse>,
    ) -> Self {
        self.send_transaction = h.into_handler("send_transaction");
        self
    }
    pub fn with_get_taddress_txids(
        mut self,
        h: impl IntoHandler<TransparentAddressBlockFilter, Stream<RawTransaction>>,
    ) -> Self {
        self.get_taddress_txids = h.into_handler("get_taddress_txids");
        self
    }
    pub fn with_get_taddress_transactions(
        mut self,
        h: impl IntoHandler<TransparentAddressBlockFilter, Stream<RawTransaction>>,
    ) -> Self {
        self.get_taddress_transactions = h.into_handler("get_taddress_transactions");
        self
    }
    pub fn with_get_taddress_balance(mut self, h: impl IntoHandler<AddressList, Balance>) -> Self {
        self.get_taddress_balance = h.into_handler("get_taddress_balance");
        self
    }
    pub fn with_get_taddress_balance_stream(
        mut self,
        h: impl IntoHandler<tonic::Streaming<Address>, Balance>,
    ) -> Self {
        self.get_taddress_balance_stream = h.into_handler("get_taddress_balance_stream");
        self
    }
    pub fn with_get_mempool_tx(
        mut self,
        h: impl IntoHandler<GetMempoolTxRequest, Stream<CompactTx>>,
    ) -> Self {
        self.get_mempool_tx = h.into_handler("get_mempool_tx");
        self
    }
    pub fn with_get_mempool_stream(
        mut self,
        h: impl IntoHandler<Empty, Stream<RawTransaction>>,
    ) -> Self {
        self.get_mempool_stream = h.into_handler("get_mempool_stream");
        self
    }
    pub fn with_get_tree_state(mut self, h: impl IntoHandler<BlockId, TreeState>) -> Self {
        self.get_tree_state = h.into_handler("get_tree_state");
        self
    }
    pub fn with_get_latest_tree_state(mut self, h: impl IntoHandler<Empty, TreeState>) -> Self {
        self.get_latest_tree_state = h.into_handler("get_latest_tree_state");
        self
    }
    pub fn with_get_subtree_roots(
        mut self,
        h: impl IntoHandler<GetSubtreeRootsArg, Stream<SubtreeRoot>>,
    ) -> Self {
        self.get_subtree_roots = h.into_handler("get_subtree_roots");
        self
    }
    pub fn with_get_address_utxos(
        mut self,
        h: impl IntoHandler<GetAddressUtxosArg, GetAddressUtxosReplyList>,
    ) -> Self {
        self.get_address_utxos = h.into_handler("get_address_utxos");
        self
    }
    pub fn with_get_address_utxos_stream(
        mut self,
        h: impl IntoHandler<GetAddressUtxosArg, Stream<GetAddressUtxosReply>>,
    ) -> Self {
        self.get_address_utxos_stream = h.into_handler("get_address_utxos_stream");
        self
    }
    pub fn with_get_lightd_info(mut self, h: impl IntoHandler<Empty, LightdInfo>) -> Self {
        self.get_lightd_info = h.into_handler("get_lightd_info");
        self
    }
    pub fn with_ping(mut self, h: impl IntoHandler<Duration, PingResponse>) -> Self {
        self.ping = h.into_handler("ping");
        self
    }
}

// ---------------------------------------------------------------------------
// ConfigurableMockStreamer
// ---------------------------------------------------------------------------

/// A configurable mock of the `CompactTxStreamer` gRPC service.
///
/// Each method delegates to the corresponding [`MethodHandler`] in its
/// [`MockConfig`], allowing per-method control of responses, errors, hangs,
/// or panics.
pub struct ConfigurableMockStreamer {
    config: MockConfig,
}

impl ConfigurableMockStreamer {
    pub fn new(config: MockConfig) -> Self {
        Self { config }
    }
}

// ---------------------------------------------------------------------------
// MockCompactTxStreamer (unchanged — backward-compatible all-unimplemented mock)
// ---------------------------------------------------------------------------

/// A mock implementation of the `CompactTxStreamer` gRPC service.
///
/// Every method is `unimplemented!()`. The purpose of this server is to
/// detect unexpected indexer contact during offline wallet operations.
pub struct MockCompactTxStreamer;

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

// ---------------------------------------------------------------------------
// CompactTxStreamer impl for ConfigurableMockStreamer
// ---------------------------------------------------------------------------

#[tonic::async_trait]
impl CompactTxStreamer for ConfigurableMockStreamer {
    async fn get_latest_block(
        &self,
        request: tonic::Request<ChainSpec>,
    ) -> Result<tonic::Response<BlockId>, tonic::Status> {
        self.config.get_latest_block.call(request).await
    }

    async fn get_block(
        &self,
        request: tonic::Request<BlockId>,
    ) -> Result<tonic::Response<CompactBlock>, tonic::Status> {
        self.config.get_block.call(request).await
    }

    async fn get_block_nullifiers(
        &self,
        request: tonic::Request<BlockId>,
    ) -> Result<tonic::Response<CompactBlock>, tonic::Status> {
        self.config.get_block_nullifiers.call(request).await
    }

    type GetBlockRangeStream = Stream<CompactBlock>;

    async fn get_block_range(
        &self,
        request: tonic::Request<BlockRange>,
    ) -> Result<tonic::Response<Self::GetBlockRangeStream>, tonic::Status> {
        self.config.get_block_range.call(request).await
    }

    type GetBlockRangeNullifiersStream = Stream<CompactBlock>;

    async fn get_block_range_nullifiers(
        &self,
        request: tonic::Request<BlockRange>,
    ) -> Result<tonic::Response<Self::GetBlockRangeNullifiersStream>, tonic::Status> {
        self.config.get_block_range_nullifiers.call(request).await
    }

    async fn get_transaction(
        &self,
        request: tonic::Request<TxFilter>,
    ) -> Result<tonic::Response<RawTransaction>, tonic::Status> {
        self.config.get_transaction.call(request).await
    }

    async fn send_transaction(
        &self,
        request: tonic::Request<RawTransaction>,
    ) -> Result<tonic::Response<SendResponse>, tonic::Status> {
        self.config.send_transaction.call(request).await
    }

    type GetTaddressTxidsStream = Stream<RawTransaction>;

    async fn get_taddress_txids(
        &self,
        request: tonic::Request<TransparentAddressBlockFilter>,
    ) -> Result<tonic::Response<Self::GetTaddressTxidsStream>, tonic::Status> {
        self.config.get_taddress_txids.call(request).await
    }

    type GetTaddressTransactionsStream = Stream<RawTransaction>;

    async fn get_taddress_transactions(
        &self,
        request: tonic::Request<TransparentAddressBlockFilter>,
    ) -> Result<tonic::Response<Self::GetTaddressTransactionsStream>, tonic::Status> {
        self.config.get_taddress_transactions.call(request).await
    }

    async fn get_taddress_balance(
        &self,
        request: tonic::Request<AddressList>,
    ) -> Result<tonic::Response<Balance>, tonic::Status> {
        self.config.get_taddress_balance.call(request).await
    }

    async fn get_taddress_balance_stream(
        &self,
        request: tonic::Request<tonic::Streaming<Address>>,
    ) -> Result<tonic::Response<Balance>, tonic::Status> {
        self.config.get_taddress_balance_stream.call(request).await
    }

    type GetMempoolTxStream = Stream<CompactTx>;

    async fn get_mempool_tx(
        &self,
        request: tonic::Request<GetMempoolTxRequest>,
    ) -> Result<tonic::Response<Self::GetMempoolTxStream>, tonic::Status> {
        self.config.get_mempool_tx.call(request).await
    }

    type GetMempoolStreamStream = Stream<RawTransaction>;

    async fn get_mempool_stream(
        &self,
        request: tonic::Request<Empty>,
    ) -> Result<tonic::Response<Self::GetMempoolStreamStream>, tonic::Status> {
        self.config.get_mempool_stream.call(request).await
    }

    async fn get_tree_state(
        &self,
        request: tonic::Request<BlockId>,
    ) -> Result<tonic::Response<TreeState>, tonic::Status> {
        self.config.get_tree_state.call(request).await
    }

    async fn get_latest_tree_state(
        &self,
        request: tonic::Request<Empty>,
    ) -> Result<tonic::Response<TreeState>, tonic::Status> {
        self.config.get_latest_tree_state.call(request).await
    }

    type GetSubtreeRootsStream = Stream<SubtreeRoot>;

    async fn get_subtree_roots(
        &self,
        request: tonic::Request<GetSubtreeRootsArg>,
    ) -> Result<tonic::Response<Self::GetSubtreeRootsStream>, tonic::Status> {
        self.config.get_subtree_roots.call(request).await
    }

    async fn get_address_utxos(
        &self,
        request: tonic::Request<GetAddressUtxosArg>,
    ) -> Result<tonic::Response<GetAddressUtxosReplyList>, tonic::Status> {
        self.config.get_address_utxos.call(request).await
    }

    type GetAddressUtxosStreamStream = Stream<GetAddressUtxosReply>;

    async fn get_address_utxos_stream(
        &self,
        request: tonic::Request<GetAddressUtxosArg>,
    ) -> Result<tonic::Response<Self::GetAddressUtxosStreamStream>, tonic::Status> {
        self.config.get_address_utxos_stream.call(request).await
    }

    async fn get_lightd_info(
        &self,
        request: tonic::Request<Empty>,
    ) -> Result<tonic::Response<LightdInfo>, tonic::Status> {
        self.config.get_lightd_info.call(request).await
    }

    async fn ping(
        &self,
        request: tonic::Request<Duration>,
    ) -> Result<tonic::Response<PingResponse>, tonic::Status> {
        self.config.ping.call(request).await
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Call a handler with a default request and return the status code on error.
    async fn call_status<Req, Resp>(handler: &MethodHandler<Req, Resp>) -> tonic::Code
    where
        Req: Default + Send + 'static,
        Resp: Send + std::fmt::Debug + 'static,
    {
        handler
            .call(tonic::Request::new(Req::default()))
            .await
            .unwrap_err()
            .code()
    }

    #[tokio::test]
    async fn all_error_config_returns_expected_status() {
        let cfg = MockConfig::all_error(tonic::Code::DeadlineExceeded, "timeout");
        assert_eq!(
            call_status(&cfg.get_latest_block).await,
            tonic::Code::DeadlineExceeded
        );
        assert_eq!(
            call_status(&cfg.get_block).await,
            tonic::Code::DeadlineExceeded
        );
        assert_eq!(
            call_status(&cfg.get_tree_state).await,
            tonic::Code::DeadlineExceeded
        );
        assert_eq!(
            call_status(&cfg.get_lightd_info).await,
            tonic::Code::DeadlineExceeded
        );
        assert_eq!(call_status(&cfg.ping).await, tonic::Code::DeadlineExceeded);
    }

    #[tokio::test]
    async fn with_override_changes_only_target_field() {
        let cfg = MockConfig::all_error(tonic::Code::Unavailable, "down").with_get_tree_state(
            MethodBehavior::Error(
                tonic::Code::DeadlineExceeded,
                "get_tree_state timeout".into(),
            ),
        );

        // Overridden field
        let status = cfg
            .get_tree_state
            .call(tonic::Request::new(BlockId::default()))
            .await
            .unwrap_err();
        assert_eq!(status.code(), tonic::Code::DeadlineExceeded);
        assert_eq!(status.message(), "get_tree_state timeout");

        // Non-overridden field stays at the original
        assert_eq!(
            call_status(&cfg.get_latest_block).await,
            tonic::Code::Unavailable
        );
    }

    #[tokio::test]
    async fn from_response_returns_success() {
        let handler: MethodHandler<ChainSpec, BlockId> =
            MethodHandler::from_response(BlockId::default());
        let resp = handler
            .call(tonic::Request::new(ChainSpec::default()))
            .await
            .expect("should succeed");
        assert_eq!(resp.into_inner(), BlockId::default());
    }

    #[tokio::test]
    async fn from_fn_callback_has_full_control() {
        let handler = MethodHandler::from_fn(|req: tonic::Request<BlockId>| async move {
            let id = req.into_inner();
            if id.height == 42 {
                Ok(tonic::Response::new(TreeState {
                    height: 42,
                    ..Default::default()
                }))
            } else {
                Err(tonic::Status::not_found("no such block"))
            }
        });

        // height == 42 → success
        let resp = handler
            .call(tonic::Request::new(BlockId {
                height: 42,
                ..Default::default()
            }))
            .await
            .expect("should succeed for height 42");
        assert_eq!(resp.into_inner().height, 42);

        // height != 42 → error
        let status = handler
            .call(tonic::Request::new(BlockId::default()))
            .await
            .unwrap_err();
        assert_eq!(status.code(), tonic::Code::NotFound);
    }

    #[tokio::test]
    async fn method_behavior_still_works_in_builder() {
        // MethodBehavior auto-converts via IntoHandler
        let cfg = MockConfig::unimplemented()
            .with_get_lightd_info(MethodBehavior::Error(tonic::Code::Internal, "boom".into()));

        let status = cfg
            .get_lightd_info
            .call(tonic::Request::new(Empty::default()))
            .await
            .unwrap_err();
        assert_eq!(status.code(), tonic::Code::Internal);
        assert_eq!(status.message(), "boom");
    }
}
