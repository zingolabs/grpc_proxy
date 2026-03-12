//! A gRPC proxy implementing the `CompactTxStreamer` service.
//!
//! All endpoints return `unimplemented!()`. Used to verify that offline
//! wallet operations do not contact the indexer.

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
