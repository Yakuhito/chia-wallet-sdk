use chia_protocol::{Bytes32, Coin, CoinState};
use sqlx::SqlitePool;

/// A SQLite implementation of a coin store. Uses the table name `standard_coin_states`.
#[derive(Debug, Clone)]
pub struct SqliteCoinStore {
    db: SqlitePool,
}

impl SqliteCoinStore {
    /// Create a new `SqliteCoinStore` from a connection pool.
    pub fn new(db: SqlitePool) -> Self {
        Self { db }
    }

    /// Connect to a SQLite database and run migrations.
    pub async fn new_with_migrations(db: SqlitePool) -> Result<Self, sqlx::Error> {
        sqlx::migrate!().run(&db).await?;
        Ok(Self { db })
    }

    /// Apply a list of coin updates to the store.
    pub async fn apply_updates(&self, coin_states: Vec<CoinState>) {
        let mut tx = self.db.begin().await.unwrap();

        for coin_state in coin_states {
            let coin_id = coin_state.coin.coin_id().to_vec();
            let parent_coin_info = coin_state.coin.parent_coin_info.to_bytes().to_vec();
            let puzzle_hash = coin_state.coin.puzzle_hash.to_bytes().to_vec();
            let amount = coin_state.coin.amount as i64;

            sqlx::query!(
                "
                REPLACE INTO `standard_coin_states` (
                    `coin_id`,
                    `parent_coin_info`,
                    `puzzle_hash`,
                    `amount`,
                    `created_height`,
                    `spent_height`
                )
                VALUES (?, ?, ?, ?, ?, ?)
                ",
                coin_id,
                parent_coin_info,
                puzzle_hash,
                amount,
                coin_state.created_height,
                coin_state.spent_height
            )
            .execute(&mut *tx)
            .await
            .unwrap();
        }

        tx.commit().await.unwrap();
    }

    /// Get a list of all unspent coins in the store.
    pub async fn unspent_coins(&self) -> Vec<Coin> {
        let rows = sqlx::query!(
            "
            SELECT `parent_coin_info`, `puzzle_hash`, `amount`
            FROM `standard_coin_states`
            WHERE `spent_height` IS NULL
            "
        )
        .fetch_all(&self.db)
        .await
        .unwrap();

        rows.into_iter()
            .map(|row| {
                let parent_coin_info = row.parent_coin_info;
                let puzzle_hash = row.puzzle_hash;
                let amount = row.amount as u64;

                Coin {
                    parent_coin_info: parent_coin_info.try_into().unwrap(),
                    puzzle_hash: puzzle_hash.try_into().unwrap(),
                    amount,
                }
            })
            .collect()
    }

    /// Get the state of a coin by its id.
    pub async fn coin_state(&self, coin_id: Bytes32) -> Option<CoinState> {
        let coin_id = coin_id.to_vec();

        let row = sqlx::query!(
            "
            SELECT `parent_coin_info`, `puzzle_hash`, `amount`, `created_height`, `spent_height`
            FROM `standard_coin_states`
            WHERE `coin_id` = ?
            ",
            coin_id
        )
        .fetch_optional(&self.db)
        .await
        .unwrap()?;

        Some(CoinState {
            coin: Coin {
                parent_coin_info: row.parent_coin_info.try_into().unwrap(),
                puzzle_hash: row.puzzle_hash.try_into().unwrap(),
                amount: row.amount as u64,
            },
            created_height: row.created_height.map(|height| height as u32),
            spent_height: row.spent_height.map(|height| height as u32),
        })
    }

    /// Check if a puzzle hash is used in the store.
    pub async fn is_used(&self, puzzle_hash: Bytes32) -> bool {
        let puzzle_hash = puzzle_hash.to_vec();

        let row = sqlx::query!(
            "
            SELECT COUNT(*) AS `count`
            FROM `standard_coin_states`
            WHERE `puzzle_hash` = ?
            ",
            puzzle_hash
        )
        .fetch_one(&self.db)
        .await
        .unwrap();

        row.count > 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[sqlx::test]
    async fn test_unspent_coins(pool: SqlitePool) {
        let coin_store = SqliteCoinStore::new_with_migrations(pool.clone())
            .await
            .unwrap();

        // Insert a spent and unspent coin.
        coin_store
            .apply_updates(vec![
                CoinState {
                    coin: Coin {
                        parent_coin_info: Bytes32::default(),
                        puzzle_hash: Bytes32::default(),
                        amount: 100,
                    },
                    created_height: Some(10),
                    spent_height: None,
                },
                CoinState {
                    coin: Coin {
                        parent_coin_info: Bytes32::default(),
                        puzzle_hash: Bytes32::default(),
                        amount: 101,
                    },
                    created_height: Some(10),
                    spent_height: Some(15),
                },
            ])
            .await;

        // Make sure only one is unspent.
        let unspent_coins = coin_store.unspent_coins().await;
        assert_eq!(unspent_coins.len(), 1);
        assert_eq!(unspent_coins[0].amount, 100);
    }

    #[sqlx::test]
    async fn test_coin_state(pool: SqlitePool) {
        let coin_store = SqliteCoinStore::new_with_migrations(pool.clone())
            .await
            .unwrap();

        // Insert a coin state into the database.
        let coin_state = CoinState {
            coin: Coin {
                parent_coin_info: Bytes32::default(),
                puzzle_hash: Bytes32::default(),
                amount: 100,
            },
            created_height: Some(10),
            spent_height: None,
        };

        coin_store.apply_updates(vec![coin_state.clone()]).await;

        // Ensure the result is the same as when it was put in.
        let roundtrip = coin_store
            .coin_state(coin_state.coin.coin_id())
            .await
            .expect("coin state not found");
        assert_eq!(coin_state, roundtrip);
    }

    #[sqlx::test]
    async fn test_is_used(pool: SqlitePool) {
        let coin_store = SqliteCoinStore::new_with_migrations(pool.clone())
            .await
            .unwrap();

        // Insert a coin state into the database.
        let coin_state = CoinState {
            coin: Coin {
                parent_coin_info: Bytes32::default(),
                puzzle_hash: Bytes32::default(),
                amount: 100,
            },
            created_height: Some(10),
            spent_height: None,
        };

        coin_store.apply_updates(vec![coin_state.clone()]).await;

        // Ensure the puzzle hash we inserted is used.
        let is_used = coin_store.is_used(coin_state.coin.puzzle_hash).await;
        assert!(is_used);

        // Ensure a different puzzle hash is not used.
        let is_used = coin_store.is_used(Bytes32::new([1; 32])).await;
        assert!(!is_used);
    }
}
