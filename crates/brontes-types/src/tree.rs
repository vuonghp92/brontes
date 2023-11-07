use std::{
    collections::{HashMap, HashSet},
    ops::Index,
};

use malachite::Rational;
use rayon::prelude::{IntoParallelRefIterator, IntoParallelRefMutIterator, ParallelIterator};
use reth_primitives::{Address, Header, H256, U256};
use serde::{Deserialize, Serialize};
use serde_with::serde_as;
use sorella_db_databases::clickhouse::{self, Row};

use crate::normalized_actions::NormalizedAction;

#[derive(Serialize, Deserialize)]
pub struct TimeTree<V: NormalizedAction> {
    pub roots: Vec<Root<V>>,
    pub header: Header,
    pub avg_priority_fee: u64,
    /// first is on block submission, second is when the block gets accepted
    pub eth_prices: (Rational, Rational),
}

impl<V: NormalizedAction> TimeTree<V> {
    pub fn new(header: Header, eth_prices: (Rational, Rational)) -> Self {
        Self { roots: Vec::with_capacity(150), header, eth_prices, avg_priority_fee: 0 }
    }

    pub fn get_root(&self, tx_hash: H256) -> Option<&Root<V>> {
        self.roots.par_iter().find_any(|r| r.tx_hash == tx_hash)
    }

    pub fn get_gas_details(&self, hash: H256) -> Option<&GasDetails> {
        self.roots
            .iter()
            .find(|h| h.tx_hash == hash)
            .map(|root| &root.gas_details)
    }

    pub fn insert_root(&mut self, root: Root<V>) {
        self.roots.push(root);
    }

    pub fn finalize_tree(&mut self) {
        self.avg_priority_fee = self
            .roots
            .iter()
            .map(|tx| tx.gas_details.effective_gas_price - self.header.base_fee_per_gas.unwrap())
            .sum::<u64>()
            / self.roots.len() as u64;

        self.roots.iter_mut().for_each(|root| root.finalize());
    }

    pub fn insert_node(&mut self, node: Node<V>) {
        self.roots
            .last_mut()
            .expect("no root_nodes inserted")
            .insert(node);
    }

    pub fn get_hashes(&self) -> Vec<H256> {
        self.roots.iter().map(|r| r.tx_hash).collect()
    }

    pub fn inspect<F>(&self, hash: H256, call: F) -> Vec<Vec<V>>
    where
        F: Fn(&Node<V>) -> bool,
    {
        if let Some(root) = self.roots.iter().find(|r| r.tx_hash == hash) {
            root.inspect(&call)
        } else {
            vec![]
        }
    }

    pub fn inspect_all<F>(&self, call: F) -> HashMap<H256, Vec<Vec<V>>>
    where
        F: Fn(&Node<V>) -> bool + Send + Sync,
    {
        self.roots
            .par_iter()
            .map(|r| (r.tx_hash, r.inspect(&call)))
            .collect()
    }

    /// the first function parses down through the tree to the point where we
    /// are at the lowest subset of the valid action. once we reach here,
    /// the call function gets executed in order to capture the data
    pub fn dyn_classify<T, F>(&mut self, find: T, call: F) -> Vec<(Address, (Address, Address))>
    where
        T: Fn(Address, Vec<V>) -> bool + Sync,
        F: Fn(&mut Node<V>) -> Option<(Address, (Address, Address))> + Send + Sync,
    {
        self.roots
            .par_iter_mut()
            .flat_map(|root| root.dyn_classify(&find, &call))
            .collect()
    }

    pub fn remove_duplicate_data<F, C, T, R>(&mut self, find: F, classify: C, info: T)
    where
        T: Fn(&Node<V>) -> R + Sync,
        C: Fn(&Vec<R>, &Node<V>) -> Vec<u64> + Sync,
        F: Fn(&Node<V>) -> bool + Sync,
    {
        self.roots
            .par_iter_mut()
            .for_each(|root| root.remove_duplicate_data(&find, &classify, &info));
    }
}

#[derive(Serialize, Deserialize)]
pub struct Root<V: NormalizedAction> {
    pub head: Node<V>,
    pub tx_hash: H256,
    pub private: bool,
    pub gas_details: GasDetails,
}

impl<V: NormalizedAction> Root<V> {
    pub fn insert(&mut self, node: Node<V>) {
        self.head.insert(node)
    }

    pub fn inspect<F>(&self, call: &F) -> Vec<Vec<V>>
    where
        F: Fn(&Node<V>) -> bool,
    {
        let mut result = Vec::new();
        self.head.inspect(&mut result, call);

        result
    }

    pub fn remove_duplicate_data<F, C, T, R>(&mut self, find: &F, classify: &C, info: &T)
    where
        T: Fn(&Node<V>) -> R,
        C: Fn(&Vec<R>, &Node<V>) -> Vec<u64>,
        F: Fn(&Node<V>) -> bool,
    {
        let mut indexes = HashSet::new();
        self.head
            .indexes_to_remove(&mut indexes, find, classify, info);
        indexes
            .into_iter()
            .for_each(|index| self.head.remove_index_and_childs(index));
    }

    pub fn dyn_classify<T, F>(&mut self, find: &T, call: &F) -> Vec<(Address, (Address, Address))>
    where
        T: Fn(Address, Vec<V>) -> bool,
        F: Fn(&mut Node<V>) -> Option<(Address, (Address, Address))> + Send + Sync,
    {
        // bool is used for recursion
        let mut results = Vec::new();
        let _ = self.head.dyn_classify(find, call, &mut results);

        results
    }

    pub fn finalize(&mut self) {
        self.head.finalize();
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Row, Default)]
pub struct GasDetails {
    pub coinbase_transfer: Option<u64>,
    pub priority_fee: u64,
    pub gas_used: u64,
    pub effective_gas_price: u64,
}

impl GasDetails {
    pub fn gas_paid(&self) -> u64 {
        let mut gas = self.gas_used * self.effective_gas_price;

        if let Some(coinbase) = self.coinbase_transfer {
            gas += coinbase as u64
        }

        gas
    }

    pub fn priority_fee(&self, base_fee: u64) -> u64 {
        self.effective_gas_price - base_fee
    }
}

#[derive(Serialize, Deserialize)]
pub struct Node<V: NormalizedAction> {
    pub inner: Vec<Node<V>>,
    pub finalized: bool,
    pub index: u64,

    /// This only has values when the node is frozen
    pub subactions: Vec<V>,
    pub trace_address: Vec<usize>,
    pub address: Address,
    pub data: V,
}

impl<V: NormalizedAction> Node<V> {
    pub fn is_finalized(&self) -> bool {
        self.finalized
    }

    pub fn finalize(&mut self) {
        self.subactions = self.get_all_sub_actions();
        self.finalized = true;

        self.inner.iter_mut().for_each(|f| f.finalize());
    }

    /// The address here is the from address for the trace
    pub fn insert(&mut self, n: Node<V>) {
        if self.finalized {
            return;
        }

        let trace_addr = n.trace_address.clone();
        self.get_all_inner_nodes(n, trace_addr);
    }

    pub fn get_all_inner_nodes(&mut self, n: Node<V>, mut trace_addr: Vec<usize>) {
        if trace_addr.len() == 1 {
            self.inner.push(n);
        } else {
            let inner = self.inner.get_mut(trace_addr.remove(0)).unwrap();
            inner.get_all_inner_nodes(n, trace_addr)
        }
    }

    pub fn get_all_sub_actions(&self) -> Vec<V> {
        if self.finalized {
            self.subactions.clone()
        } else {
            let mut inner = self
                .inner
                .iter()
                .flat_map(|inner| inner.get_all_sub_actions())
                .collect::<Vec<V>>();
            inner.push(self.data.clone());

            inner
        }
    }

    pub fn tree_right_path(&self) -> Vec<Address> {
        self.inner
            .last()
            .map(|last| {
                let mut last = last.tree_right_path();
                last.push(self.address);
                last
            })
            .unwrap_or(vec![self.address])
    }

    pub fn all_sub_addresses(&self) -> Vec<Address> {
        self.inner
            .iter()
            .flat_map(|i| i.all_sub_addresses())
            .chain(vec![self.address])
            .collect()
    }

    pub fn current_call_stack(&self) -> Vec<Address> {
        let Some(mut stack) = self.inner.last().map(|n| n.current_call_stack()) else {
            return vec![self.address];
        };

        stack.push(self.address);

        stack
    }

    pub fn indexes_to_remove<F, C, T, R>(
        &self,
        indexes: &mut HashSet<u64>,
        find: &F,
        classify: &C,
        info: &T,
    ) -> bool
    where
        F: Fn(&Node<V>) -> bool,
        C: Fn(&Vec<R>, &Node<V>) -> Vec<u64>,
        T: Fn(&Node<V>) -> R,
    {
        // prev better
        if !find(self) {
            return false;
        }
        let lower_has_better = self
            .inner
            .iter()
            .map(|i| i.indexes_to_remove(indexes, find, classify, info))
            .any(|f| f);

        if !lower_has_better {
            let mut data = Vec::new();
            self.get_bounded_info(0, self.index, &mut data, info);
            let classified_indexes = classify(&data, self);
            indexes.extend(classified_indexes);
        }

        return true;
    }

    pub fn get_bounded_info<F, R>(&self, lower: u64, upper: u64, res: &mut Vec<R>, info_fn: &F)
    where
        F: Fn(&Node<V>) -> R,
    {
        if self.inner.is_empty() {
            return;
        }

        let last = self.inner.last().unwrap();

        // fully in bounds
        if self.index >= lower && last.index <= upper {
            res.push(info_fn(self));
            self.inner
                .iter()
                .for_each(|node| node.get_bounded_info(lower, upper, res, info_fn));

            return;
        }

        // find bounded limit
        let mut iter = self.inner.iter().enumerate().peekable();
        let mut start = None;
        let mut end = None;

        while start.is_none() || end.is_none() {
            if let Some((our_index, next)) = iter.next() {
                if let Some((_, peek)) = iter.peek() {
                    // find lower
                    start = start.or(Some(our_index).filter(|_| next.index >= lower));
                    // find upper
                    end = end.or(Some(our_index).filter(|_| peek.index > upper));
                }
            } else {
                break;
            }
        }

        match (start, end) {
            (Some(start), Some(end)) => {
                self.inner[start..end]
                    .iter()
                    .for_each(|node| node.get_bounded_info(lower, upper, res, info_fn));
            }
            (Some(start), None) => {
                self.inner[start..]
                    .iter()
                    .for_each(|node| node.get_bounded_info(lower, upper, res, info_fn));
            }
            _ => {}
        }
    }

    pub fn remove_index_and_childs(&mut self, index: u64) {
        if self.inner.is_empty() {
            return;
        }

        let mut iter = self.inner.iter_mut().enumerate().peekable();

        let val = loop {
            if let Some((our_index, next)) = iter.next() {
                if index == next.index {
                    break Some(our_index);
                }

                if let Some(peek) = iter.peek() {
                    if index > next.index && index < peek.1.index {
                        next.remove_index_and_childs(index);
                        break None;
                    }
                } else {
                    break None;
                }
            }
        };

        if let Some(val) = val {
            self.inner.remove(val);
        }
    }

    pub fn inspect<F>(&self, result: &mut Vec<Vec<V>>, call: &F) -> bool
    where
        F: Fn(&Node<V>) -> bool,
    {
        println!(
            "Subdata: {:?}",
            &self
                .subactions
                .iter()
                .map(|s| s.get_action())
                .collect::<Vec<_>>()
        );

        println!(
            "\n\nINSPECTOR NODE - FROM ADDRESS: {:?}, DATA: {:?}",
            self.address,
            &self.data.get_action()
        );

        println!("INSPECTOR NODE - NOT SELF CALL: {}", !call(self));
        println!(
            "INSPECTOR NODE - SELF SUBACTIONS: {:?}",
            self.subactions
                .clone()
                .iter()
                .map(|sub| sub.get_action())
                .collect::<Vec<_>>()
        );

        // the previous sub-action was the last one to meet the criteria
        if !call(self) {
            return false;
        }

        let lower_has_better = self
            .inner
            .iter()
            .map(|i| i.inspect(result, call))
            .any(|f| f);

        println!("INSPECTOR NODE - LOWER HAS BETTER: {}", !lower_has_better);

        // if all child nodes don't have a best sub-action. Then the current node is the
        // best.
        if !lower_has_better {
            let mut res = self.get_all_sub_actions();
            res.push(self.data.clone());
            result.push(res);
        }

        println!(
            "INSPECTOR NODE - RESULTS: {:?}\n\n",
            result
                .iter()
                .map(|s| s.iter().map(|ss| ss.get_action()).collect::<Vec<_>>())
                .collect::<Vec<_>>()
        );
        // lower node has a better sub-action.
        true
    }

    pub fn dyn_classify<T, F>(
        &mut self,
        find: &T,
        call: &F,
        result: &mut Vec<(Address, (Address, Address))>,
    ) -> bool
    where
        T: Fn(Address, Vec<V>) -> bool,
        F: Fn(&mut Node<V>) -> Option<(Address, (Address, Address))> + Send + Sync,
    {
        let works = find(self.address, self.get_all_sub_actions());
        if !works {
            return false;
        }

        let lower_has_better = self
            .inner
            .iter_mut()
            .any(|i| i.dyn_classify(find, call, result));

        if !lower_has_better {
            if let Some(res) = call(self) {
                result.push(res);
            }
        }
        true
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use crate::test_utils::ComparisonNode;
    use brontes_classifier::test_utils::build_raw_test_tree;
    use brontes_core::test_utils::init_trace_parser;
    use brontes_database::database::Database;
    use reth_rpc_types::trace::parity::TraceType;
    use serial_test::serial;
    use tokio::sync::mpsc::unbounded_channel;

    #[tokio::test]
    #[serial]
    async fn test_raw_tree() {
        let block_num = 18180900;
        dotenv::dotenv().ok();

        let (tx, _rx) = unbounded_channel();

        let tracer = init_trace_parser(tokio::runtime::Handle::current().clone(), tx);
        let db = Database::default();
        let mut tree = build_raw_test_tree(&tracer, &db, block_num).await;

        let mut transaction_traces = tracer
            .tracer
            .trace
            .replay_block_transactions(block_num.into(), HashSet::from([TraceType::Trace]))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(tree.roots.len(), transaction_traces.len());

        let first_root = tree.roots.remove(0);
        let first_tx = transaction_traces.remove(0);

        assert_eq!(
            Into::<ComparisonNode>::into(&first_root.head),
            ComparisonNode::new(&first_tx.full_trace.trace[0], 0, 8)
        );

        assert_eq!(
            Into::<ComparisonNode>::into(&first_root.head.inner[0]),
            ComparisonNode::new(&first_tx.full_trace.trace[1], 1, 1)
        );

        assert_eq!(
            Into::<ComparisonNode>::into(&first_root.head.inner[0].inner[0]),
            ComparisonNode::new(&first_tx.full_trace.trace[2], 2, 0)
        );

        assert_eq!(
            Into::<ComparisonNode>::into(&first_root.head.inner[1]),
            ComparisonNode::new(&first_tx.full_trace.trace[3], 3, 0)
        );

        assert_eq!(
            Into::<ComparisonNode>::into(&first_root.head.inner[2]),
            ComparisonNode::new(&first_tx.full_trace.trace[4], 4, 0)
        );

        assert_eq!(
            Into::<ComparisonNode>::into(&first_root.head.inner[3]),
            ComparisonNode::new(&first_tx.full_trace.trace[5], 5, 0)
        );

        assert_eq!(
            Into::<ComparisonNode>::into(&first_root.head.inner[4]),
            ComparisonNode::new(&first_tx.full_trace.trace[6], 6, 0)
        );

        assert_eq!(
            Into::<ComparisonNode>::into(&first_root.head.inner[5]),
            ComparisonNode::new(&first_tx.full_trace.trace[7], 7, 3)
        );

        assert_eq!(
            Into::<ComparisonNode>::into(&first_root.head.inner[5].inner[0]),
            ComparisonNode::new(&first_tx.full_trace.trace[8], 8, 0)
        );

        assert_eq!(
            Into::<ComparisonNode>::into(&first_root.head.inner[5].inner[1]),
            ComparisonNode::new(&first_tx.full_trace.trace[9], 9, 0)
        );

        assert_eq!(
            Into::<ComparisonNode>::into(&first_root.head.inner[5].inner[2]),
            ComparisonNode::new(&first_tx.full_trace.trace[10], 10, 3)
        );

        assert_eq!(
            Into::<ComparisonNode>::into(&first_root.head.inner[5].inner[2].inner[0]),
            ComparisonNode::new(&first_tx.full_trace.trace[11], 11, 0)
        );

        assert_eq!(
            Into::<ComparisonNode>::into(&first_root.head.inner[5].inner[2].inner[1]),
            ComparisonNode::new(&first_tx.full_trace.trace[12], 12, 0)
        );

        assert_eq!(
            Into::<ComparisonNode>::into(&first_root.head.inner[5].inner[2].inner[2]),
            ComparisonNode::new(&first_tx.full_trace.trace[13], 13, 0)
        );

        assert_eq!(
            Into::<ComparisonNode>::into(&first_root.head.inner[6]),
            ComparisonNode::new(&first_tx.full_trace.trace[14], 14, 0)
        );

        assert_eq!(
            Into::<ComparisonNode>::into(&first_root.head.inner[7]),
            ComparisonNode::new(&first_tx.full_trace.trace[15], 15, 0)
        );
    }
}
