use crate::db::{
    Db, RegisterResult, RegistrationRequest, RequestStatus, SELECT_REQUEST, STATUS_APPROVED,
    STATUS_DELETED, STATUS_PENDING, current_unix_timestamp,
};

impl Db {
    /// Создаёт или возвращает существующую pending-заявку.
    pub async fn register_or_get(
        &self,
        tg_user_id: i64,
        tg_username: Option<&str>,
        tg_display_name: Option<&str>,
    ) -> Result<RegisterResult, anyhow::Error> {
        let now = current_unix_timestamp()?;
        let mut tx = self.pool.begin().await?;

        let existing_sql = format!("{SELECT_REQUEST} WHERE tg_user_id = ?");
        let existing = sqlx::query_as::<_, RegistrationRequest>(&existing_sql)
            .bind(tg_user_id)
            .fetch_optional(&mut *tx)
            .await?;

        if let Some(existing) = existing {
            let result = match existing.status {
                RequestStatus::Approved => existing
                    .secret
                    .map(RegisterResult::Approved)
                    .unwrap_or(RegisterResult::AlreadyPending),
                RequestStatus::Rejected => RegisterResult::Rejected,
                RequestStatus::Pending => {
                    sqlx::query(
                        "UPDATE registration_requests
                         SET tg_username = ?, tg_display_name = ?, created_at = ?
                         WHERE tg_user_id = ? AND status = 'pending'",
                    )
                    .bind(tg_username)
                    .bind(tg_display_name)
                    .bind(now)
                    .bind(tg_user_id)
                    .execute(&mut *tx)
                    .await?;
                    RegisterResult::AlreadyPending
                }
                RequestStatus::Deleted => {
                    let revived = sqlx::query(
                        "UPDATE registration_requests
                         SET status = 'pending',
                             tg_username = ?,
                             tg_display_name = ?,
                             telemt_username = NULL,
                             secret = NULL,
                             created_at = ?,
                             resolved_at = NULL
                         WHERE tg_user_id = ? AND status = 'deleted'",
                    )
                    .bind(tg_username)
                    .bind(tg_display_name)
                    .bind(now)
                    .bind(tg_user_id)
                    .execute(&mut *tx)
                    .await?;

                    if revived.rows_affected() == 0 {
                        RegisterResult::AlreadyPending
                    } else {
                        let revived_sql =
                            format!("{SELECT_REQUEST} WHERE tg_user_id = ? AND status = 'pending'");
                        let req = sqlx::query_as::<_, RegistrationRequest>(&revived_sql)
                            .bind(tg_user_id)
                            .fetch_optional(&mut *tx)
                            .await?
                            .ok_or_else(|| anyhow::anyhow!("заявка была возвращена в pending"))?;
                        RegisterResult::NewPending(req)
                    }
                }
            };

            tx.commit().await?;
            return Ok(result);
        }

        let inserted = sqlx::query(
            "INSERT INTO registration_requests (tg_user_id, tg_username, tg_display_name, status, created_at)
             VALUES (?, ?, ?, 'pending', ?)
             ON CONFLICT(tg_user_id) DO NOTHING",
        )
        .bind(tg_user_id)
        .bind(tg_username)
        .bind(tg_display_name)
        .bind(now)
        .execute(&mut *tx)
        .await?;

        let result = if inserted.rows_affected() == 0 {
            let existing = sqlx::query_as::<_, RegistrationRequest>(&existing_sql)
                .bind(tg_user_id)
                .fetch_optional(&mut *tx)
                .await?;

            match existing {
                Some(existing) => match existing.status {
                    RequestStatus::Approved => existing
                        .secret
                        .map(RegisterResult::Approved)
                        .unwrap_or(RegisterResult::AlreadyPending),
                    RequestStatus::Rejected => RegisterResult::Rejected,
                    RequestStatus::Pending | RequestStatus::Deleted => {
                        RegisterResult::AlreadyPending
                    }
                },
                None => {
                    return Err(anyhow::anyhow!(
                        "Не удалось определить состояние заявки после upsert"
                    ));
                }
            }
        } else {
            let pending_sql =
                format!("{SELECT_REQUEST} WHERE tg_user_id = ? AND status = 'pending'");
            let req = sqlx::query_as::<_, RegistrationRequest>(&pending_sql)
                .bind(tg_user_id)
                .fetch_optional(&mut *tx)
                .await?
                .ok_or_else(|| anyhow::anyhow!("только что создали заявку"))?;
            RegisterResult::NewPending(req)
        };

        tx.commit().await?;
        Ok(result)
    }

    /// Получает pending-заявку по id.
    pub async fn get_pending_by_id(
        &self,
        id: i64,
    ) -> Result<Option<RegistrationRequest>, anyhow::Error> {
        let sql = format!("{SELECT_REQUEST} WHERE id = ? AND status = '{STATUS_PENDING}'");
        let r = sqlx::query_as::<_, RegistrationRequest>(&sql)
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(r)
    }

    /// Помечает заявку как approved и сохраняет telemt_username и secret.
    pub async fn approve(
        &self,
        id: i64,
        telemt_username: &str,
        secret: &str,
    ) -> Result<Option<RegistrationRequest>, anyhow::Error> {
        let now = current_unix_timestamp()?;
        let mut tx = self.pool.begin().await?;

        let sql = format!("{SELECT_REQUEST} WHERE id = ? AND status = '{STATUS_PENDING}'");
        let request = sqlx::query_as::<_, RegistrationRequest>(&sql)
            .bind(id)
            .fetch_optional(&mut *tx)
            .await?;

        let Some(request) = request else {
            tx.rollback().await?;
            return Ok(None);
        };

        let updated = sqlx::query(
            "UPDATE registration_requests
             SET status = 'approved', telemt_username = ?, secret = ?, resolved_at = ?
             WHERE id = ? AND status = 'pending'",
        )
        .bind(telemt_username)
        .bind(secret)
        .bind(now)
        .bind(id)
        .execute(&mut *tx)
        .await?;

        if updated.rows_affected() == 0 {
            tx.rollback().await?;
            return Ok(None);
        }

        tx.commit().await?;
        Ok(Some(request))
    }

    /// Помечает заявку как rejected.
    pub async fn reject(&self, id: i64) -> Result<Option<RegistrationRequest>, anyhow::Error> {
        let now = current_unix_timestamp()?;
        let mut tx = self.pool.begin().await?;

        let sql = format!("{SELECT_REQUEST} WHERE id = ? AND status = '{STATUS_PENDING}'");
        let request = sqlx::query_as::<_, RegistrationRequest>(&sql)
            .bind(id)
            .fetch_optional(&mut *tx)
            .await?;

        let Some(request) = request else {
            tx.rollback().await?;
            return Ok(None);
        };

        let updated = sqlx::query(
            "UPDATE registration_requests
             SET status = 'rejected', resolved_at = ?
             WHERE id = ? AND status = 'pending'",
        )
        .bind(now)
        .bind(id)
        .execute(&mut *tx)
        .await?;

        if updated.rows_affected() == 0 {
            tx.rollback().await?;
            return Ok(None);
        }

        tx.commit().await?;
        Ok(Some(request))
    }

    /// Деактивирует пользователя (помечает как удалённого для истории; сама запись остаётся).
    pub async fn deactivate_user(&self, tg_user_id: i64) -> Result<bool, anyhow::Error> {
        let r = sqlx::query(
            "UPDATE registration_requests
             SET status = ?, last_synced_at = ?
             WHERE tg_user_id = ? AND status = ?",
        )
        .bind(STATUS_DELETED)
        .bind(current_unix_timestamp()?)
        .bind(tg_user_id)
        .bind(STATUS_APPROVED)
        .execute(&self.pool)
        .await?;
        Ok(r.rows_affected() > 0)
    }

    /// Устанавливает пользователя как approved (для /create без предварительной заявки).
    pub async fn set_approved(
        &self,
        tg_user_id: i64,
        tg_username: Option<&str>,
        tg_display_name: Option<&str>,
        telemt_username: &str,
        secret: &str,
    ) -> Result<(), anyhow::Error> {
        let now = current_unix_timestamp()?;
        sqlx::query(
            "INSERT INTO registration_requests
             (tg_user_id, tg_username, tg_display_name, status, telemt_username, secret, created_at, resolved_at)
             VALUES (?, ?, ?, 'approved', ?, ?, ?, ?)
             ON CONFLICT(tg_user_id) DO UPDATE SET
                 status = 'approved',
                 tg_username = COALESCE(excluded.tg_username, registration_requests.tg_username),
                 tg_display_name = COALESCE(excluded.tg_display_name, registration_requests.tg_display_name),
                 telemt_username = excluded.telemt_username,
                 secret = excluded.secret,
                 resolved_at = excluded.resolved_at",
        )
        .bind(tg_user_id)
        .bind(tg_username)
        .bind(tg_display_name)
        .bind(telemt_username)
        .bind(secret)
        .bind(now)
        .bind(now)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Получает approved-пользователя по tg_user_id.
    pub async fn get_approved(
        &self,
        tg_user_id: i64,
    ) -> Result<Option<(String, String)>, anyhow::Error> {
        let sql = format!("{SELECT_REQUEST} WHERE tg_user_id = ? AND status = '{STATUS_APPROVED}'");
        let r = sqlx::query_as::<_, RegistrationRequest>(&sql)
            .bind(tg_user_id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(r.and_then(|x| x.telemt_username.zip(x.secret)))
    }

    pub async fn get_request_by_tg_user(
        &self,
        tg_user_id: i64,
    ) -> Result<Option<RegistrationRequest>, anyhow::Error> {
        let sql = format!("{SELECT_REQUEST} WHERE tg_user_id = ?");
        let r = sqlx::query_as::<_, RegistrationRequest>(&sql)
            .bind(tg_user_id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(r)
    }

    /// Ищет tg_user_id по tg_username (без учёта регистра, без @).
    pub async fn find_tg_user_id_by_username(
        &self,
        username: &str,
    ) -> Result<Option<i64>, anyhow::Error> {
        let normalized = username.trim_start_matches('@');
        if normalized.is_empty() {
            return Ok(None);
        }

        let user_id = sqlx::query_scalar::<_, i64>(
            "SELECT tg_user_id FROM registration_requests
             WHERE lower(tg_username) = lower(?)
             ORDER BY created_at DESC
             LIMIT 1",
        )
        .bind(normalized)
        .fetch_optional(&self.pool)
        .await?;
        Ok(user_id)
    }

    pub async fn count_pending_requests(&self) -> Result<i64, anyhow::Error> {
        let total = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM registration_requests WHERE status = ?",
        )
        .bind(STATUS_PENDING)
        .fetch_one(&self.pool)
        .await?;
        Ok(total)
    }

    pub async fn list_pending_requests_page(
        &self,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<RegistrationRequest>, anyhow::Error> {
        let rows = sqlx::query_as::<_, RegistrationRequest>(
            "SELECT id, tg_user_id, tg_username, tg_display_name, status, telemt_username, secret, created_at,
                    backend_mode, last_sync_error, last_seen_revision, last_synced_at
             FROM registration_requests
             WHERE status = ?
             ORDER BY created_at DESC, id DESC
             LIMIT ? OFFSET ?",
        )
        .bind(STATUS_PENDING)
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    pub async fn count_active_users(&self) -> Result<i64, anyhow::Error> {
        let total = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM registration_requests WHERE status = ?",
        )
        .bind(STATUS_APPROVED)
        .fetch_one(&self.pool)
        .await?;
        Ok(total)
    }

    pub async fn list_active_users_page(
        &self,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<RegistrationRequest>, anyhow::Error> {
        let rows = sqlx::query_as::<_, RegistrationRequest>(
            "SELECT id, tg_user_id, tg_username, tg_display_name, status, telemt_username, secret, created_at,
                    backend_mode, last_sync_error, last_seen_revision, last_synced_at
             FROM registration_requests
             WHERE status = ?
             ORDER BY created_at DESC
             LIMIT ? OFFSET ?",
        )
        .bind(STATUS_APPROVED)
        .bind(limit)
        .bind(offset)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    pub async fn get_active_user_by_tg_user(
        &self,
        tg_user_id: i64,
    ) -> Result<Option<RegistrationRequest>, anyhow::Error> {
        let row = sqlx::query_as::<_, RegistrationRequest>(
            "SELECT id, tg_user_id, tg_username, tg_display_name, status, telemt_username, secret, created_at,
                    backend_mode, last_sync_error, last_seen_revision, last_synced_at
             FROM registration_requests
             WHERE status = ? AND tg_user_id = ?
             LIMIT 1",
        )
        .bind(STATUS_APPROVED)
        .bind(tg_user_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row)
    }

    pub async fn mark_sync_state(
        &self,
        tg_user_id: i64,
        backend_mode: &str,
        last_seen_revision: Option<&str>,
        last_sync_error: Option<&str>,
    ) -> Result<(), anyhow::Error> {
        let now = current_unix_timestamp()?;
        sqlx::query(
            "UPDATE registration_requests
             SET backend_mode = ?,
                 last_seen_revision = ?,
                 last_sync_error = ?,
                 last_synced_at = ?
             WHERE tg_user_id = ?",
        )
        .bind(backend_mode)
        .bind(last_seen_revision)
        .bind(last_sync_error)
        .bind(now)
        .bind(tg_user_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}
