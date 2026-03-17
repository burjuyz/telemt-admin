use crate::db::{
    ACTIVE_INVITE_TOKEN_PREDICATE, AdminActivity, AdminActivityKind, AdminStats, Db,
    current_unix_timestamp,
};

impl Db {
    pub async fn admin_stats(&self) -> Result<AdminStats, anyhow::Error> {
        let now = current_unix_timestamp()?;
        let row = sqlx::query_as::<_, (i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64, i64)>(
            &format!(
                "SELECT
                    (SELECT COUNT(*) FROM registration_requests) AS total,
                    (SELECT COUNT(*) FROM registration_requests WHERE status = 'pending') AS pending,
                    (SELECT COUNT(*) FROM registration_requests WHERE status = 'approved') AS approved,
                    (SELECT COUNT(*) FROM registration_requests WHERE status = 'rejected') AS rejected,
                    (SELECT COUNT(*) FROM registration_requests WHERE status = 'deleted') AS deleted,
                    (SELECT COUNT(*) FROM invite_tokens) AS tokens_total,
                    (SELECT COUNT(*) FROM invite_tokens WHERE {active}) AS tokens_active,
                    (SELECT COUNT(*) FROM invite_tokens
                      WHERE is_active = 1
                        AND auto_approve = 0
                        AND expires_at > ?
                        AND (max_usage IS NULL OR usage_count < max_usage)) AS tokens_manual_active,
                    (SELECT COUNT(*) FROM invite_tokens
                      WHERE is_active = 1
                        AND auto_approve = 1
                        AND expires_at > ?
                        AND (max_usage IS NULL OR usage_count < max_usage)) AS tokens_auto_active,
                    (SELECT COUNT(*) FROM invite_tokens WHERE is_active = 0) AS tokens_revoked,
                    (SELECT COUNT(*) FROM invite_tokens
                      WHERE is_active = 1
                        AND expires_at <= ?) AS tokens_expired,
                    (SELECT COUNT(*) FROM invite_tokens
                      WHERE is_active = 1
                        AND expires_at > ?
                        AND max_usage IS NOT NULL
                        AND usage_count >= max_usage) AS tokens_exhausted",
                active = ACTIVE_INVITE_TOKEN_PREDICATE
            ),
        )
        .bind(now)
        .bind(now)
        .bind(now)
        .bind(now)
        .bind(now)
        .fetch_one(&self.pool)
        .await?;

        Ok(AdminStats {
            total: row.0,
            pending: row.1,
            approved: row.2,
            rejected: row.3,
            deleted: row.4,
            tokens_total: row.5,
            tokens_active: row.6,
            tokens_manual_active: row.7,
            tokens_auto_active: row.8,
            tokens_revoked: row.9,
            tokens_expired: row.10,
            tokens_exhausted: row.11,
        })
    }

    pub async fn list_recent_admin_activities(
        &self,
        limit: i64,
    ) -> Result<Vec<AdminActivity>, anyhow::Error> {
        let rows = sqlx::query_as::<_, (i64, String, Option<i64>, Option<String>)>(
            "SELECT timestamp, kind, request_id, token FROM (
                SELECT resolved_at AS timestamp,
                       'request_approved' AS kind,
                       id AS request_id,
                       NULL AS token
                FROM registration_requests
                WHERE resolved_at IS NOT NULL
                  AND status = 'approved'

                UNION ALL

                SELECT resolved_at AS timestamp,
                       'request_rejected' AS kind,
                       id AS request_id,
                       NULL AS token
                FROM registration_requests
                WHERE resolved_at IS NOT NULL
                  AND status = 'rejected'

                UNION ALL

                SELECT created_at AS timestamp,
                       'token_created' AS kind,
                       NULL AS request_id,
                       token
                FROM invite_tokens

                UNION ALL

                SELECT revoked_at AS timestamp,
                       'token_revoked' AS kind,
                       NULL AS request_id,
                       token
                FROM invite_tokens
                WHERE revoked_at IS NOT NULL
            )
            WHERE timestamp IS NOT NULL
            ORDER BY timestamp DESC
            LIMIT ?",
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter()
            .map(|(timestamp, kind, request_id, token)| {
                let kind = match kind.as_str() {
                    "request_approved" => AdminActivityKind::RequestApproved {
                        request_id: request_id.ok_or_else(|| {
                            anyhow::anyhow!("request_id отсутствует для request_approved")
                        })?,
                    },
                    "request_rejected" => AdminActivityKind::RequestRejected {
                        request_id: request_id.ok_or_else(|| {
                            anyhow::anyhow!("request_id отсутствует для request_rejected")
                        })?,
                    },
                    "token_created" => AdminActivityKind::TokenCreated {
                        token: token.ok_or_else(|| {
                            anyhow::anyhow!("token отсутствует для token_created")
                        })?,
                    },
                    "token_revoked" => AdminActivityKind::TokenRevoked {
                        token: token.ok_or_else(|| {
                            anyhow::anyhow!("token отсутствует для token_revoked")
                        })?,
                    },
                    other => {
                        return Err(anyhow::anyhow!(
                            "Неизвестный тип admin activity из БД: {}",
                            other
                        ));
                    }
                };

                Ok(AdminActivity { timestamp, kind })
            })
            .collect()
    }
}
