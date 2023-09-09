use colored::Colorize;
use reth_primitives::H256;
use tracing::info;

use crate::PoirotMetricEvents;

/// metric event for traces
#[derive(Clone, Debug)]
pub enum TraceMetricEvent {
    /// recorded a new block trace
    BlockMetricRecieved(BlockStats),
    /// recorded a new tx trace
    TransactionMetricRecieved(TransactionStats),
    /// recorded a new individual tx trace
    TraceMetricRecieved(TraceStats),
}

impl Into<PoirotMetricEvents> for TraceMetricEvent {
    fn into(self) -> PoirotMetricEvents {
        PoirotMetricEvents::TraceMetricRecieved(self)
    }
}

#[derive(Clone, Debug)]
pub struct BlockStats {
    pub block_num: u64,
    pub txs: Vec<TransactionStats>,
    pub err: Option<TraceParseErrorKind>,
}

impl BlockStats {
    pub fn new(block_num: u64, err: Option<TraceParseErrorKind>) -> Self {
        Self { block_num, txs: Vec::new(), err }
    }

    pub fn trace(&self) {
        let message = format!(
            "Successfuly Parsed Block {}",
            format!("{}", self.block_num).bright_blue().bold()
        );
        info!(message = message);
    }
}

#[derive(Clone, Debug)]
pub struct TransactionStats {
    pub block_num: u64,
    pub tx_hash: H256,
    pub tx_idx: u16,
    pub traces: Vec<TraceStats>,
    pub err: Option<TraceParseErrorKind>,
}

impl TransactionStats {
    pub fn new(
        block_num: u64,
        tx_hash: H256,
        tx_idx: u16,
        err: Option<TraceParseErrorKind>,
    ) -> Self {
        Self { block_num, tx_hash, tx_idx, traces: Vec::new(), err }
    }

    pub fn trace(&self) {
        let tx_hash = format!("{:#x}", self.tx_hash);
        info!("result = \"Successfully Parsed Transaction\", tx_hash = {}\n", tx_hash);
    }
}

#[derive(Clone, Copy, Debug)]
pub struct TraceStats {
    pub block_num: u64,
    pub tx_hash: H256,
    pub tx_idx: u16,
    pub trace_idx: u16,
    pub err: Option<TraceParseErrorKind>,
}

impl TraceStats {
    pub fn new(
        block_num: u64,
        tx_hash: H256,
        tx_idx: u16,
        trace_idx: u16,
        err: Option<TraceParseErrorKind>,
    ) -> Self {
        Self { block_num, tx_hash, tx_idx, trace_idx, err }
    }

    pub fn trace(&self, total_len: usize) {
        let tx_hash = format!("{:#x}", self.tx_hash);
        let message = format!(
            "{}",
            format!("Starting Transaction Trace {} / {}", self.trace_idx + 1, &total_len)
                .bright_blue()
                .bold()
        );
        info!(message = message, tx_hash = tx_hash);
    }
}

/// enum for error
#[derive(Debug, Clone, Copy)]
pub enum TraceParseErrorKind {
    TracesMissingBlock,
    TracesMissingTx,
    EmptyInput,
    AbiParseError,
    EthApiError,
    InvalidFunctionSelector,
    AbiDecodingFailed,
    ChannelSendError,
    EtherscanChainNotSupported,
    EtherscanExecutionFailed,
    EtherscanBalanceFailed,
    EtherscanNotProxy,
    EtherscanMissingImplementationAddress,
    EtherscanBlockNumberByTimestampFailed,
    EtherscanTransactionReceiptFailed,
    EtherscanGasEstimationFailed,
    EtherscanBadStatusCode,
    EtherscanEnvVarNotFound,
    EtherscanReqwest,
    EtherscanSerde,
    EtherscanContractCodeNotVerified,
    EtherscanEmptyResult,
    EtherscanRateLimitExceeded,
    EtherscanIO,
    EtherscanLocalNetworksNotSupported,
    EtherscanErrorResponse,
    EtherscanUnknown,
    EtherscanBuilder,
    EtherscanMissingSolcVersion,
    EtherscanInvalidApiKey,
    EtherscanBlockedByCloudflare,
    EtherscanCloudFlareSecurityChallenge,
    EtherscanPageNotFound,
    EtherscanCacheError,
    EthApiEmptyRawTransactionData,
    EthApiFailedToDecodeSignedTransaction,
    EthApiInvalidTransactionSignature,
    EthApiPoolError,
    EthApiUnknownBlockNumber,
    EthApiUnknownBlockOrTxIndex,
    EthApiInvalidBlockRange,
    EthApiPrevrandaoNotSet,
    EthApiConflictingFeeFieldsInRequest,
    EthApiInvalidTransaction,
    EthApiInvalidBlockData,
    EthApiBothStateAndStateDiffInOverride,
    EthApiInternal,
    EthApiSigning,
    EthApiTransactionNotFound,
    EthApiUnsupported,
    EthApiInvalidParams,
    EthApiInvalidTracerConfig,
    EthApiInvalidRewardPercentiles,
    EthApiInternalTracingError,
    EthApiInternalEthError,
    EthApiInternalJsTracerError,
}
