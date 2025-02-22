use crate::{Error, Transaction};
use parking_lot::RwLock;
use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap};
use std::sync::Arc;

/// A transaction in the mempool with priority
#[derive(Debug, Clone)]
pub struct MempoolTx {
    /// The actual transaction
    pub transaction: Transaction,
    /// Time added to mempool
    pub timestamp: u64,
    /// Priority score (higher = more priority)
    pub priority: u64,
}

impl PartialEq for MempoolTx {
    fn eq(&self, other: &Self) -> bool {
        self.transaction == other.transaction
    }
}

impl Eq for MempoolTx {}

impl PartialOrd for MempoolTx {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for MempoolTx {
    fn cmp(&self, other: &Self) -> Ordering {
        // Higher priority comes first
        self.priority.cmp(&other.priority).reverse()
    }
}

/// Thread-safe mempool
#[derive(Clone)]
pub struct Mempool {
    /// Transactions by hash
    txs: Arc<RwLock<HashMap<[u8; 32], MempoolTx>>>,
    /// Priority queue for ordering
    queue: Arc<RwLock<BinaryHeap<MempoolTx>>>,
    /// Maximum number of transactions
    max_size: usize,
}

impl Mempool {
    /// Create a new mempool
    pub fn new(max_size: usize) -> Self {
        Self {
            txs: Arc::new(RwLock::new(HashMap::new())),
            queue: Arc::new(RwLock::new(BinaryHeap::new())),
            max_size,
        }
    }

    /// Add a transaction to the mempool
    pub fn add_tx(&self, tx: Transaction, priority: u64) -> Result<(), Error> {
        let tx_hash = self.hash_tx(&tx);
        let mempool_tx = MempoolTx {
            transaction: tx,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            priority,
        };

        // Check if we already have this transaction
        let mut txs = self.txs.write();
        if txs.contains_key(&tx_hash) {
            return Ok(());
        }

        // Add to mempool if there's space
        if txs.len() >= self.max_size {
            return Err(Error::Internal("Mempool is full".to_string()));
        }

        txs.insert(tx_hash, mempool_tx.clone());
        self.queue.write().push(mempool_tx);

        Ok(())
    }

    /// Get the top N transactions by priority
    pub fn get_top(&self, n: usize) -> Vec<Transaction> {
        let txs = self.txs.read();
        let queue = self.queue.read();

        queue
            .iter()
            .take(n)
            .filter(|tx| txs.contains_key(&self.hash_tx(&tx.transaction)))
            .map(|tx| tx.transaction.clone())
            .collect()
    }

    /// Remove transactions that are included in a block
    pub fn remove_included(&self, txs: &[Transaction]) {
        let mut mempool_txs = self.txs.write();
        let mut queue = self.queue.write();

        for tx in txs {
            let tx_hash = self.hash_tx(tx);
            mempool_txs.remove(&tx_hash);
            queue.retain(|mempool_tx| mempool_tx.transaction != *tx);
        }
    }

    /// Calculate transaction hash
    fn hash_tx(&self, tx: &Transaction) -> [u8; 32] {
        let mut hasher = blake3::Hasher::new();
        hasher.update(&tx.sender);
        hasher.update(&tx.nonce.to_le_bytes());
        hasher.update(&tx.payload);
        hasher.finalize().into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Keypair, SigningKey};

    #[test]
    fn test_mempool_ordering() {
        let mempool = Mempool::new(1000);
        let keypair = SigningKey::generate(&mut rand::thread_rng());
        let public_key = keypair.verifying_key();

        // Create transactions with different gas prices
        let tx1 = Transaction {
            sender: public_key,
            nonce: 1,
            gas_price: 10,
            payload: vec![],
            signature: Signature::from_bytes(&[0; 64]).unwrap(),
        };

        let tx2 = Transaction {
            sender: public_key,
            nonce: 2,
            gas_price: 20,
            payload: vec![],
            signature: Signature::from_bytes(&[0; 64]).unwrap(),
        };

        // Add transactions
        mempool.add_tx(tx1.clone(), 10).unwrap();
        mempool.add_tx(tx2.clone(), 20).unwrap();

        // Check ordering
        let top_txs = mempool.get_top(2);
        assert_eq!(top_txs.len(), 2);
        assert_eq!(top_txs[0].gas_price, 20); // Higher gas price first
        assert_eq!(top_txs[1].gas_price, 10);
    }
}
