use crate::db::{
    current_unix_timestamp, is_unique_violation, ConsumedInviteToken, Db, InviteToken,
    TokenConsumeError, TokenMode, ACTIVE_INVITE_TOKEN_PREDICATE, SELECT_INVITE_TOKEN,
};
use rand::distr::{Alphanumeric, SampleString};

fn map_internal_token_error(context: &str, err: impl std::fmt::Display) -> TokenConsumeError {
    TokenConsumeError::Internal(anyhow::anyhow!("{context}: {err}"))
}

fn generate_invite_token() -> String {
    Alphanumeric.sample_string(&mut rand::rng(), 10)
}

fn token_mode(auto_approve: bool) -> TokenMode {
    if auto_approve {
        TokenMode::AutoApprove
    } else {
        TokenMode::Manual
    }
}

impl Db {
    pub async fn create_invite_token(
        &self,
        days: i64,
        auto_approve: bool,
        max_usage: Option<i64>,
        created_by: Option<i64>,
    ) -> Result<InviteToken, anyhow::Error> {
        let now = current_unix_timestamp()?;
        let ttl_seconds = days
            .checked_mul(86_400)
            .ok_or_else(|| anyhow::anyhow!("Срок действия токена слишком большой"))?;
        let expires_at = now
            .checked_add(ttl_seconds)
            .ok_or_else(|| anyhow::anyhow!("Некорректное время истечения токена"))?;

        let mut created: Option<InviteToken> = None;
        for _ in 0..8 {
            let token = generate_invite_token();
            let result = sqlx::query(
                "INSERT INTO invite_tokens (token, created_at, expires_at, auto_approve, created_by, max_usage)
                 VALUES (?, ?, ?, ?, ?, ?)",
            )
            .bind(&token)
            .bind(now)
            .bind(expires_at)
            .bind(auto_approve)
            .bind(created_by)
            .bind(max_usage)
            .execute(&self.pool)
            .await;

            match result {
                Ok(_) => {
                    let sql = format!("{SELECT_INVITE_TOKEN} WHERE token = ?");
                    created = sqlx::query_as::<_, InviteToken>(&sql)
                        .bind(token)
                        .fetch_optional(&self.pool)
                        .await?;
                    if created.is_some() {
                        break;
                    }
                }
                Err(err) if is_unique_violation(&err) => continue,
                Err(err) => {
                    return Err(anyhow::anyhow!("Не удалось создать invite-токен: {}", err));
                }
            }
        }

        created.ok_or_else(|| anyhow::anyhow!("Не удалось сгенерировать уникальный токен"))
    }

    pub async fn count_active_invite_tokens(&self) -> Result<i64, anyhow::Error> {
        let now = current_unix_timestamp()?;
        let count = sqlx::query_scalar::<_, i64>(&format!(
            "SELECT COUNT(*) FROM invite_tokens WHERE {ACTIVE_INVITE_TOKEN_PREDICATE}"
        ))
        .bind(now)
        .fetch_one(&self.pool)
        .await?;
        Ok(count)
    }

    pub async fn list_active_invite_tokens_page(
        &self,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<InviteToken>, anyhow::Error> {
        let now = current_unix_timestamp()?;
        let rows = sqlx::query_as::<_, InviteToken>(&format!(
            "{SELECT_INVITE_TOKEN}
             WHERE {ACTIVE_INVITE_TOKEN_PREDICATE}
             ORDER BY expires_at ASC, id ASC
             LIMIT ? OFFSET ?"
        ))
        .bind(now)
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    pub async fn get_active_invite_token_by_id(
        &self,
        token_id: i64,
    ) -> Result<Option<InviteToken>, anyhow::Error> {
        let now = current_unix_timestamp()?;
        let token = sqlx::query_as::<_, InviteToken>(&format!(
            "{SELECT_INVITE_TOKEN}
             WHERE id = ?
               AND {ACTIVE_INVITE_TOKEN_PREDICATE}"
        ))
        .bind(token_id)
        .bind(now)
        .fetch_optional(&self.pool)
        .await?;
        Ok(token)
    }

    pub async fn get_active_invite_token_by_token(
        &self,
        token: &str,
    ) -> Result<Option<InviteToken>, anyhow::Error> {
        let now = current_unix_timestamp()?;
        let token = sqlx::query_as::<_, InviteToken>(&format!(
            "{SELECT_INVITE_TOKEN}
             WHERE token = ?
               AND {ACTIVE_INVITE_TOKEN_PREDICATE}
             LIMIT 1"
        ))
        .bind(token)
        .bind(now)
        .fetch_optional(&self.pool)
        .await?;
        Ok(token)
    }

    pub async fn revoke_invite_token_by_id(&self, token_id: i64) -> Result<bool, anyhow::Error> {
        let now = current_unix_timestamp()?;
        let result = sqlx::query(&format!(
            "UPDATE invite_tokens
             SET is_active = 0, revoked_at = ?
             WHERE id = ?
               AND {ACTIVE_INVITE_TOKEN_PREDICATE}"
        ))
        .bind(now)
        .bind(token_id)
        .bind(now)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn consume_invite_token(
        &self,
        token: &str,
    ) -> Result<ConsumedInviteToken, TokenConsumeError> {
        let now = current_unix_timestamp()
            .map_err(|err| map_internal_token_error("Не удалось получить текущее время", err))?;
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|err| map_internal_token_error("Не удалось начать транзакцию consume_invite_token", err))?;

        let update_result = sqlx::query(&format!(
            "UPDATE invite_tokens
             SET usage_count = usage_count + 1
             WHERE token = ?
               AND {ACTIVE_INVITE_TOKEN_PREDICATE}"
        ))
        .bind(token)
        .bind(now)
        .execute(&mut *tx)
        .await
        .map_err(|err| map_internal_token_error("Не удалось обновить usage_count invite-токена", err))?;

        let row_sql = format!("{SELECT_INVITE_TOKEN} WHERE token = ?");

        if update_result.rows_affected() == 0 {
            let token_row = sqlx::query_as::<_, InviteToken>(&row_sql)
                .bind(token)
                .fetch_optional(&mut *tx)
                .await
                .map_err(|err| map_internal_token_error("Не удалось загрузить invite-токен после неуспешного consume", err))?;

            tx.rollback()
                .await
                .map_err(|err| map_internal_token_error("Не удалось откатить транзакцию consume_invite_token", err))?;

            let Some(row) = token_row else {
                return Err(TokenConsumeError::NotFound);
            };
            if !row.is_active {
                return Err(TokenConsumeError::Revoked);
            }
            if row.expires_at <= now {
                return Err(TokenConsumeError::Expired);
            }
            if row.max_usage.is_some_and(|max| row.usage_count >= max) {
                return Err(TokenConsumeError::UsageLimitReached);
            }
            return Err(TokenConsumeError::NotFound);
        }

        let row = sqlx::query_as::<_, InviteToken>(&row_sql)
            .bind(token)
            .fetch_optional(&mut *tx)
            .await
            .map_err(|err| map_internal_token_error("Не удалось перечитать invite-токен после consume", err))?;
        let row = row.ok_or_else(|| {
            TokenConsumeError::Internal(anyhow::anyhow!(
                "Invite-токен исчез после успешного обновления usage_count"
            ))
        })?;

        tx.commit()
            .await
            .map_err(|err| map_internal_token_error("Не удалось закоммитить consume_invite_token", err))?;

        Ok(ConsumedInviteToken {
            id: row.id,
            token: row.token,
            mode: token_mode(row.auto_approve),
            expires_at: row.expires_at,
            created_by: row.created_by,
            usage_count: row.usage_count,
            max_usage: row.max_usage,
        })
    }
}
