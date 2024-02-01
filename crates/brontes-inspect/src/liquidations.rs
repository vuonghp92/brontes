use std::{collections::HashSet, sync::Arc};

use brontes_database::libmdbx::{Libmdbx, LibmdbxReader};
use brontes_types::{
    mev::{Bundle, BundleData, BundleHeader, Liquidation, MevType, TokenProfit, TokenProfits},
    normalized_actions::{Actions, NormalizedLiquidation, NormalizedSwap},
    pair::Pair,
    tree::{BlockTree, GasDetails, Node, Root},
    ToFloatNearest, TxInfo,
};
use hyper::header;
use malachite::{num::basic::traits::Zero, Rational};
use rayon::iter::{IntoParallelIterator, IntoParallelRefIterator, ParallelIterator};
use reth_primitives::{b256, Address, B256};

use crate::{shared_utils::SharedInspectorUtils, Inspector, MetadataCombined};

pub struct LiquidationInspector<'db, DB: LibmdbxReader> {
    inner: SharedInspectorUtils<'db, DB>,
}

impl<'db, DB: LibmdbxReader> LiquidationInspector<'db, DB> {
    pub fn new(quote: Address, db: &'db DB) -> Self {
        Self { inner: SharedInspectorUtils::new(quote, db) }
    }
}

#[async_trait::async_trait]
impl<DB: LibmdbxReader> Inspector for LiquidationInspector<'_, DB> {
    async fn process_tree(
        &self,
        tree: Arc<BlockTree<Actions>>,
        metadata: Arc<MetadataCombined>,
    ) -> Vec<Bundle> {
        let liq_txs = tree.collect_all(|node| {
            (
                node.data.is_liquidation() || node.data.is_swap(),
                node.subactions
                    .iter()
                    .any(|action| action.is_liquidation() || action.is_swap()),
            )
        });

        liq_txs
            .into_par_iter()
            .filter_map(|(tx_hash, liq)| {
                let info = tree.get_tx_info(tx_hash)?;

                self.calculate_liquidation(info, metadata.clone(), liq)
            })
            .collect::<Vec<_>>()
    }
}

impl<DB: LibmdbxReader> LiquidationInspector<'_, DB> {
    fn calculate_liquidation(
        &self,
        info: TxInfo,
        metadata: Arc<MetadataCombined>,
        actions: Vec<Actions>,
    ) -> Option<Bundle> {
        let swaps = actions
            .iter()
            .filter_map(|action| if let Actions::Swap(swap) = action { Some(swap) } else { None })
            .cloned()
            .collect::<Vec<_>>();

        let liqs = actions
            .iter()
            .filter_map(
                |action| {
                    if let Actions::Liquidation(liq) = action {
                        Some(liq)
                    } else {
                        None
                    }
                },
            )
            .cloned()
            .collect::<Vec<_>>();

        if liqs.is_empty() {
            return None
        }

        let liq_profit = liqs
            .par_iter()
            .filter_map(|liq| {
                let repaid_debt_usd = self.inner.calculate_dex_usd_amount(
                    info.tx_index as usize,
                    liq.debt_asset.address,
                    &liq.covered_debt,
                    &metadata,
                )?;
                let collected_collateral = self.inner.calculate_dex_usd_amount(
                    info.tx_index as usize,
                    liq.collateral_asset.address,
                    &liq.liquidated_collateral,
                    &metadata,
                )?;
                Some(collected_collateral - repaid_debt_usd)
            })
            .sum::<Rational>();

        let rev_usd =
            self.inner
                .get_dex_revenue_usd(info.tx_index, &vec![actions.clone()], metadata.clone())
                + liq_profit;

        let gas_finalized = metadata.get_gas_price_usd(info.gas_details.gas_paid());

        let profit_usd = (rev_usd - &gas_finalized).to_float();

        let header = self.inner.build_bundle_header(
            &info,
            profit_usd,
            &vec![actions],
            &vec![info.gas_details],
            metadata,
            MevType::Liquidation,
        );

        let new_liquidation = Liquidation {
            liquidation_tx_hash: info.tx_hash,
            trigger:             b256!(),
            liquidation_swaps:   swaps,
            liquidations:        liqs,
            gas_details:         info.gas_details.clone(),
        };

        Some(Bundle { header, data: BundleData::Liquidation(new_liquidation) })
    }
}

#[cfg(test)]
mod tests {
    use std::{collections::HashSet, str::FromStr, time::SystemTime};

    use alloy_primitives::{hex, U256};
    use brontes_classifier::Classifier;
    use serial_test::serial;

    use super::*;
    use crate::{
        test_utils::{InspectorTestUtils, InspectorTxRunConfig, USDC_ADDRESS},
        Inspectors,
    };

    #[tokio::test]
    #[serial]
    async fn test_aave_v3_liquidation() {
        let inspector_util = InspectorTestUtils::new(USDC_ADDRESS, 1.0);

        let config = InspectorTxRunConfig::new(Inspectors::Liquidations)
            .with_block(19042179)
            .with_dex_prices()
            .with_gas_paid_usd(2792.487)
            .with_expected_profit_usd(71.593);

        inspector_util.run_inspector(config, None).await.unwrap();
    }

    #[tokio::test]
    #[serial]
    async fn test_aave_v2_liquidation() {
        let inspector_util = InspectorTestUtils::new(USDC_ADDRESS, 1.0);

        let config = InspectorTxRunConfig::new(Inspectors::Liquidations)
            .with_block(18979710)
            .with_dex_prices()
            .with_gas_paid_usd(636.54)
            .with_expected_profit_usd(129.23);

        inspector_util.run_inspector(config, None).await.unwrap();
    }
}
