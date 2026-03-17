use crate::db::{Db, current_unix_timestamp};

impl Db {
    pub async fn get_wizard_state(&self, tg_user_id: i64) -> Result<Option<String>, anyhow::Error> {
        let row = sqlx::query_as::<_, (String, i64)>(
            "SELECT state_key, updated_at
             FROM bot_wizard_states
             WHERE tg_user_id = ?
             LIMIT 1",
        )
        .bind(tg_user_id)
        .fetch_optional(&self.pool)
        .await?;

        let Some((state_key, updated_at)) = row else {
            return Ok(None);
        };

        if self.is_wizard_state_expired(updated_at)? {
            self.clear_wizard_state(tg_user_id).await?;
            return Ok(None);
        }

        Ok(Some(state_key))
    }

    pub async fn set_wizard_state(
        &self,
        tg_user_id: i64,
        state_key: &str,
    ) -> Result<(), anyhow::Error> {
        let now = current_unix_timestamp()?;
        sqlx::query(
            "INSERT INTO bot_wizard_states (tg_user_id, state_key, updated_at)
             VALUES (?, ?, ?)
             ON CONFLICT(tg_user_id) DO UPDATE SET
                 state_key = excluded.state_key,
                 updated_at = excluded.updated_at",
        )
        .bind(tg_user_id)
        .bind(state_key)
        .bind(now)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn clear_wizard_state(&self, tg_user_id: i64) -> Result<(), anyhow::Error> {
        sqlx::query("DELETE FROM bot_wizard_states WHERE tg_user_id = ?")
            .bind(tg_user_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn cleanup_expired_wizard_states(&self) -> Result<(), anyhow::Error> {
        let Some(ttl_seconds) = self.wizard_state_ttl_seconds else {
            return Ok(());
        };

        let now = current_unix_timestamp()?;
        let expires_before = now
            .checked_sub(ttl_seconds)
            .ok_or_else(|| anyhow::anyhow!("Некорректный TTL wizard-state"))?;
        let result = sqlx::query("DELETE FROM bot_wizard_states WHERE updated_at <= ?")
            .bind(expires_before)
            .execute(&self.pool)
            .await?;
        if result.rows_affected() > 0 {
            tracing::info!(
                removed = result.rows_affected(),
                ttl_seconds = ttl_seconds,
                "Удалены просроченные wizard-состояния"
            );
        }
        Ok(())
    }

    fn is_wizard_state_expired(&self, updated_at: i64) -> Result<bool, anyhow::Error> {
        let Some(ttl_seconds) = self.wizard_state_ttl_seconds else {
            return Ok(false);
        };
        let now = current_unix_timestamp()?;
        let expires_before = now
            .checked_sub(ttl_seconds)
            .ok_or_else(|| anyhow::anyhow!("Некорректный TTL wizard-state"))?;
        Ok(updated_at <= expires_before)
    }
}
