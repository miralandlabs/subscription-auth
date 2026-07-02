//! Postgres access — solrisk hardened transaction pattern (SET LOCAL, DEALLOCATE, timeouts).

use {
    chrono::{DateTime, Utc},
    deadpool_postgres::{
        Client, Config, ManagerConfig, Pool, PoolConfig, RecyclingMethod, Runtime,
    },
    openssl::ssl::{SslConnector, SslMethod},
    postgres_openssl::MakeTlsConnector,
    serde_json::Value,
    std::time::Duration,
    tokio::time::timeout,
    tokio_postgres::types::ToSql,
    tracing::{error, warn},
    uuid::Uuid,
};

use crate::error::Error;

#[derive(Clone, Debug)]
pub struct ServiceRow {
    pub service_id: String,
    pub merchant_wallet: String,
    pub service_url: String,
    pub resources_allowlist: Value,
    pub tier_bundles: Option<Value>,
    pub status: String,
}

#[derive(Clone, Debug)]
pub struct TokenRow {
    pub jti: Uuid,
    pub service_id: String,
    pub payer: String,
    pub tier: String,
    pub resources: Value,
    pub issued_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub revoked_at: Option<DateTime<Utc>>,
}

#[derive(Clone)]
pub struct AuthDb {
    pool: Pool,
}

impl AuthDb {
    const WAIT: Duration = Duration::from_secs(15);
    const CREATE: Duration = Duration::from_secs(10);
    const RECYCLE: Duration = Duration::from_secs(30);
    const POOL_GET_TIMEOUT: Duration = Duration::from_secs(20);
    const TX_BEGIN_TIMEOUT: Duration = Duration::from_secs(20);
    const SET_LOCAL_CMD_TIMEOUT: Duration = Duration::from_secs(5);
    const QUERY_TIMEOUT: Duration = Duration::from_secs(60);
    const DEALLOCATE_TIMEOUT: Duration = Duration::from_secs(5);
    const PG_STATEMENT_TIMEOUT: &'static str = "25s";

    pub fn connect(database_url: impl Into<String>) -> Result<Self, Error> {
        let url_string = database_url.into();
        tracing::info!("Initializing database connection pool");

        let mut cfg = Config::new();
        cfg.url = Some(url_string.clone());
        cfg.manager = Some(ManagerConfig {
            recycling_method: RecyclingMethod::Clean,
        });
        cfg.pool = Some(PoolConfig {
            max_size: 5,
            timeouts: deadpool_postgres::Timeouts {
                wait: Some(Self::WAIT),
                create: Some(Self::CREATE),
                recycle: Some(Self::RECYCLE),
            },
            ..Default::default()
        });

        tracing::info!(
            "Pool config: max_size=5, wait={:?}, create={:?}, recycle={:?}",
            Self::WAIT,
            Self::CREATE,
            Self::RECYCLE
        );

        let mut builder =
            SslConnector::builder(SslMethod::tls()).map_err(|e| Error::Internal(e.to_string()))?;
        // Default: verify server certificate against system CA bundle (protects against MITM).
        // Most managed Postgres hosts (Neon, Supabase, RDS) use certs from well-known CAs.
        // Set SUBSCRIPTION_AUTH_SSL_NO_VERIFY=true ONLY for local dev with self-signed certs.
        let no_verify = std::env::var("SUBSCRIPTION_AUTH_SSL_NO_VERIFY")
            .map(|v| v == "true" || v == "1")
            .unwrap_or(false);
        if no_verify {
            tracing::warn!("SUBSCRIPTION_AUTH_SSL_NO_VERIFY=true: TLS certificate verification disabled. Do NOT use in production.");
            builder.set_verify(openssl::ssl::SslVerifyMode::NONE);
        } else {
            builder.set_verify(openssl::ssl::SslVerifyMode::PEER);
            tracing::info!("TLS certificate verification enabled (production mode)");
        }
        let tls = MakeTlsConnector::new(builder.build());
        let pool = cfg
            .create_pool(Some(Runtime::Tokio1), tls)
            .map_err(|e| Error::Internal(format!("db pool: {e}")))?;

        tracing::info!("Database connection pool created successfully");
        Ok(Self { pool })
    }

    pub fn from_env_var(var_name: &str) -> Option<Result<Self, Error>> {
        let Ok(url) = std::env::var(var_name) else {
            return None;
        };
        if url.is_empty() {
            return None;
        }
        Some(Self::connect(url))
    }

    async fn conn(&self) -> Result<Client, Error> {
        let start = std::time::Instant::now();
        tracing::info!(
            "db pool get attempt - pool_status: available={} size={} max_size={}",
            self.pool.status().available,
            self.pool.status().size,
            self.pool.status().max_size
        );

        let result = timeout(Self::POOL_GET_TIMEOUT, self.pool.get()).await;

        match result {
            Ok(Ok(client)) => {
                tracing::info!("db pool get succeeded in {:?}", start.elapsed());
                Ok(client)
            }
            Ok(Err(e)) => {
                error!(
                    "db pool get failed after {:?} - error: {} - pool_status: available={} size={} max_size={}",
                    start.elapsed(),
                    e,
                    self.pool.status().available,
                    self.pool.status().size,
                    self.pool.status().max_size
                );
                Err(Error::Internal(format!("db pool error: {e}")))
            }
            Err(_) => {
                error!(
                    "db pool get TIMED OUT after {:?} - pool_status: available={} size={} max_size={}",
                    start.elapsed(),
                    self.pool.status().available,
                    self.pool.status().size,
                    self.pool.status().max_size
                );
                Err(Error::Internal(format!(
                    "db pool get timed out after {:?}",
                    Self::POOL_GET_TIMEOUT
                )))
            }
        }
    }

    pub async fn ping(&self) -> Result<(), Error> {
        const PING_TIMEOUT: Duration = Duration::from_secs(8);
        let client = self.conn().await?;
        match timeout(PING_TIMEOUT, client.simple_query("SELECT 1")).await {
            Ok(Ok(_)) => Ok(()),
            Ok(Err(e)) => {
                Self::discard_client(client, "health ping", "ping failed");
                Err(Error::Internal(format!("health ping failed: {e}")))
            }
            Err(_) => {
                Self::discard_client(client, "health ping", "ping timed out");
                Err(Error::Internal("health ping timed out".into()))
            }
        }
    }

    pub async fn insert_nonce(
        &self,
        wallet: &str,
        nonce: &str,
        expires_at: DateTime<Utc>,
    ) -> Result<(), Error> {
        let client = self.conn().await?;
        self.exec_in_tx(
            client,
            r#"
            INSERT INTO subscription_auth_nonces (wallet, nonce, expires_at)
            VALUES ($1, $2, $3)
            ON CONFLICT (wallet, nonce) DO NOTHING
            "#,
            &[&wallet, &nonce, &expires_at],
            "insert nonce",
        )
        .await?;
        Ok(())
    }

    pub async fn nonce_exists(&self, wallet: &str, nonce: &str) -> Result<bool, Error> {
        let client = self.conn().await?;
        let row = self
            .query_opt_in_tx(
                client,
                r#"
                SELECT 1 FROM subscription_auth_nonces
                WHERE wallet = $1 AND nonce = $2 AND expires_at > NOW()
                "#,
                &[&wallet, &nonce],
                "nonce exists",
            )
            .await?;
        Ok(row.is_some())
    }

    /// Count non-expired outstanding nonces for a wallet.
    /// Used to enforce a per-wallet rate limit on /challenge.
    pub async fn count_active_nonces(&self, wallet: &str) -> Result<i64, Error> {
        let client = self.conn().await?;
        let row = self
            .query_opt_in_tx(
                client,
                r#"
                SELECT COUNT(*) AS n FROM subscription_auth_nonces
                WHERE wallet = $1 AND expires_at > NOW()
                "#,
                &[&wallet],
                "count active nonces",
            )
            .await?;
        Ok(row.map(|r| r.get::<_, i64>("n")).unwrap_or(0))
    }

    pub async fn consume_nonce(&self, wallet: &str, nonce: &str) -> Result<bool, Error> {
        let client = self.conn().await?;
        let rows = self
            .exec_in_tx(
                client,
                r#"
                DELETE FROM subscription_auth_nonces
                WHERE wallet = $1 AND nonce = $2 AND expires_at > NOW()
                "#,
                &[&wallet, &nonce],
                "consume nonce",
            )
            .await?;
        Ok(rows > 0)
    }

    /// Cleanup expired nonces to prevent unbounded table growth.
    /// Should be called periodically (e.g., every 30 minutes).
    /// Returns the number of nonces deleted.
    pub async fn cleanup_expired_nonces(&self) -> Result<u64, Error> {
        let client = self.conn().await?;
        self.exec_in_tx(
            client,
            r#"
            DELETE FROM subscription_auth_nonces
            WHERE expires_at < NOW() - INTERVAL '1 hour'
            "#,
            &[],
            "cleanup expired nonces",
        )
        .await
    }

    pub async fn get_service(&self, service_id: &str) -> Result<Option<ServiceRow>, Error> {
        let client = self.conn().await?;
        let row = self
            .query_opt_in_tx(
                client,
                r#"
                SELECT service_id, merchant_wallet, service_url, resources_allowlist,
                       tier_bundles, status
                FROM subscription_auth_services
                WHERE service_id = $1
                "#,
                &[&service_id],
                "get service",
            )
            .await?;
        Ok(row.map(|r| ServiceRow {
            service_id: r.get("service_id"),
            merchant_wallet: r.get("merchant_wallet"),
            service_url: r.get("service_url"),
            resources_allowlist: r.get("resources_allowlist"),
            tier_bundles: r.get("tier_bundles"),
            status: r.get("status"),
        }))
    }

    pub async fn insert_service(
        &self,
        service_id: &str,
        merchant_wallet: &str,
        service_url: &str,
        resources_allowlist: &Value,
        tier_bundles: Option<&Value>,
    ) -> Result<(), Error> {
        let client = self.conn().await?;
        self.exec_in_tx(
            client,
            r#"
            INSERT INTO subscription_auth_services
                (service_id, merchant_wallet, service_url, resources_allowlist, tier_bundles)
            VALUES ($1, $2, $3, $4, $5)
            "#,
            &[
                &service_id,
                &merchant_wallet,
                &service_url,
                &resources_allowlist,
                &tier_bundles,
            ],
            "insert service",
        )
        .await?;
        Ok(())
    }

    pub async fn update_service(
        &self,
        service_id: &str,
        merchant_wallet: &str,
        resources_allowlist: &Value,
        tier_bundles: Option<&Value>,
    ) -> Result<bool, Error> {
        let client = self.conn().await?;
        let rows = self
            .exec_in_tx(
                client,
                r#"
                UPDATE subscription_auth_services
                SET resources_allowlist = $3,
                    tier_bundles = $4,
                    updated_at = NOW()
                WHERE service_id = $1 AND merchant_wallet = $2 AND status = 'active'
                "#,
                &[
                    &service_id,
                    &merchant_wallet,
                    &resources_allowlist,
                    &tier_bundles,
                ],
                "update service",
            )
            .await?;
        Ok(rows > 0)
    }

    pub async fn retire_service(
        &self,
        service_id: &str,
        merchant_wallet: &str,
    ) -> Result<bool, Error> {
        let client = self.conn().await?;
        let rows = self
            .exec_in_tx(
                client,
                r#"
                UPDATE subscription_auth_services
                SET status = 'retired', updated_at = NOW()
                WHERE service_id = $1 AND merchant_wallet = $2 AND status = 'active'
                "#,
                &[&service_id, &merchant_wallet],
                "retire service",
            )
            .await?;
        Ok(rows > 0)
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn insert_token(
        &self,
        jti: Uuid,
        service_id: &str,
        payer: &str,
        tier: &str,
        resources: &Value,
        issued_at: DateTime<Utc>,
        expires_at: DateTime<Utc>,
    ) -> Result<(), Error> {
        let client = self.conn().await?;
        self.exec_in_tx(
            client,
            r#"
            INSERT INTO subscription_auth_tokens
                (jti, service_id, payer, tier, resources, issued_at, expires_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            "#,
            &[
                &jti,
                &service_id,
                &payer,
                &tier,
                &resources,
                &issued_at,
                &expires_at,
            ],
            "insert token",
        )
        .await?;
        Ok(())
    }

    pub async fn revoke_token(
        &self,
        jti: Uuid,
        service_id: &str,
        merchant_wallet: &str,
    ) -> Result<bool, Error> {
        let client = self.conn().await?;
        let rows = self
            .exec_in_tx(
                client,
                r#"
                UPDATE subscription_auth_tokens t
                SET revoked_at = NOW()
                FROM subscription_auth_services s
                WHERE t.jti = $1
                  AND t.service_id = $2
                  AND s.service_id = t.service_id
                  AND s.merchant_wallet = $3
                  AND t.revoked_at IS NULL
                "#,
                &[&jti, &service_id, &merchant_wallet],
                "revoke token",
            )
            .await?;
        Ok(rows > 0)
    }

    pub async fn get_token(&self, jti: Uuid) -> Result<Option<TokenRow>, Error> {
        let client = self.conn().await?;
        let row = self
            .query_opt_in_tx(
                client,
                r#"
                SELECT jti, service_id, payer, tier, resources, issued_at, expires_at, revoked_at
                FROM subscription_auth_tokens
                WHERE jti = $1
                "#,
                &[&jti],
                "get token",
            )
            .await?;
        Ok(row.map(|r| TokenRow {
            jti: r.get("jti"),
            service_id: r.get("service_id"),
            payer: r.get("payer"),
            tier: r.get("tier"),
            resources: r.get("resources"),
            issued_at: r.get("issued_at"),
            expires_at: r.get("expires_at"),
            revoked_at: r.get("revoked_at"),
        }))
    }

    pub async fn list_revocations(
        &self,
        service_id: &str,
        since: Option<DateTime<Utc>>,
        window_start: DateTime<Utc>,
        limit: i64,
    ) -> Result<(Vec<String>, DateTime<Utc>, bool), Error> {
        let client = self.conn().await?;
        let since_ts = since.unwrap_or(window_start);
        let rows = self
            .query_in_tx(
                client,
                r#"
                SELECT jti::text AS jti, revoked_at
                FROM subscription_auth_tokens
                WHERE service_id = $1
                  AND revoked_at IS NOT NULL
                  AND revoked_at > $2
                ORDER BY revoked_at ASC
                LIMIT $3
                "#,
                &[&service_id, &since_ts, &(limit + 1)],
                "list revocations",
            )
            .await?;

        let complete = rows.len() as i64 <= limit;
        let take = rows.len().min(limit as usize);
        let mut jtis = Vec::with_capacity(take);
        // When there are rows, cursor = timestamp of the last row (for incremental polling).
        // When there are NO rows, advance to now() so the next poll doesn't re-scan the
        // same empty window — without this, `cursor == since_ts` and the caller loops forever.
        let mut cursor = if rows.is_empty() {
            Utc::now()
        } else {
            since_ts
        };
        for row in rows.iter().take(take) {
            jtis.push(row.get::<_, String>("jti"));
            cursor = row.get("revoked_at");
        }
        Ok((jtis, cursor, complete))
    }

    pub async fn list_signing_keys(&self) -> Result<Vec<(String, Value)>, Error> {
        let client = self.conn().await?;
        let rows = self
            .query_in_tx(
                client,
                r#"
                SELECT kid, public_jwk
                FROM subscription_auth_signing_keys
                WHERE retired_at IS NULL OR retired_at > NOW() - INTERVAL '24 hours'
                ORDER BY active_from DESC
                "#,
                &[],
                "list signing keys",
            )
            .await?;
        Ok(rows
            .iter()
            .map(|r| (r.get("kid"), r.get("public_jwk")))
            .collect())
    }

    /// List tokens for a merchant wallet, most-recent first.
    /// `before` is an optional keyset cursor (issued_at of the last row from the previous page).
    /// Returns (rows, has_more). Query limit+1 rows internally; if more than `limit` come back,
    /// there is a next page and the caller should pass the last row's `issued_at` as `before`.
    pub async fn list_subscriptions(
        &self,
        merchant_wallet: &str,
        service_id: Option<&str>,
        limit: i64,
        before: Option<DateTime<Utc>>,
    ) -> Result<(Vec<TokenRow>, bool), Error> {
        let client = self.conn().await?;
        let fetch_limit = limit + 1; // fetch one extra to detect has_more
        let rows = match (service_id, before) {
            (Some(sid), Some(cursor)) => {
                self.query_in_tx(
                    client,
                    r#"
                    SELECT t.jti, t.service_id, t.payer, t.tier, t.resources,
                           t.issued_at, t.expires_at, t.revoked_at
                    FROM subscription_auth_tokens t
                    JOIN subscription_auth_services s ON s.service_id = t.service_id
                    WHERE s.merchant_wallet = $1 AND t.service_id = $2 AND t.issued_at < $3
                    ORDER BY t.issued_at DESC
                    LIMIT $4
                    "#,
                    &[&merchant_wallet, &sid, &cursor, &fetch_limit],
                    "list subscriptions",
                )
                .await?
            }
            (Some(sid), None) => {
                self.query_in_tx(
                    client,
                    r#"
                    SELECT t.jti, t.service_id, t.payer, t.tier, t.resources,
                           t.issued_at, t.expires_at, t.revoked_at
                    FROM subscription_auth_tokens t
                    JOIN subscription_auth_services s ON s.service_id = t.service_id
                    WHERE s.merchant_wallet = $1 AND t.service_id = $2
                    ORDER BY t.issued_at DESC
                    LIMIT $3
                    "#,
                    &[&merchant_wallet, &sid, &fetch_limit],
                    "list subscriptions",
                )
                .await?
            }
            (None, Some(cursor)) => {
                self.query_in_tx(
                    client,
                    r#"
                    SELECT t.jti, t.service_id, t.payer, t.tier, t.resources,
                           t.issued_at, t.expires_at, t.revoked_at
                    FROM subscription_auth_tokens t
                    JOIN subscription_auth_services s ON s.service_id = t.service_id
                    WHERE s.merchant_wallet = $1 AND t.issued_at < $2
                    ORDER BY t.issued_at DESC
                    LIMIT $3
                    "#,
                    &[&merchant_wallet, &cursor, &fetch_limit],
                    "list subscriptions",
                )
                .await?
            }
            (None, None) => {
                self.query_in_tx(
                    client,
                    r#"
                    SELECT t.jti, t.service_id, t.payer, t.tier, t.resources,
                           t.issued_at, t.expires_at, t.revoked_at
                    FROM subscription_auth_tokens t
                    JOIN subscription_auth_services s ON s.service_id = t.service_id
                    WHERE s.merchant_wallet = $1
                    ORDER BY t.issued_at DESC
                    LIMIT $2
                    "#,
                    &[&merchant_wallet, &fetch_limit],
                    "list subscriptions",
                )
                .await?
            }
        };

        let has_more = rows.len() as i64 > limit;
        let take = rows.len().min(limit as usize);

        let items = rows
            .iter()
            .take(take)
            .map(|r| TokenRow {
                jti: r.get("jti"),
                service_id: r.get("service_id"),
                payer: r.get("payer"),
                tier: r.get("tier"),
                resources: r.get("resources"),
                issued_at: r.get("issued_at"),
                expires_at: r.get("expires_at"),
                revoked_at: r.get("revoked_at"),
            })
            .collect();
        Ok((items, has_more))
    }

    // --- Transaction helpers (solrisk pattern) ---

    async fn begin_transaction(client: &Client, label: &str) -> Result<(), Error> {
        timeout(Self::TX_BEGIN_TIMEOUT, client.batch_execute("BEGIN"))
            .await
            .map_err(|_| {
                Error::Internal(format!(
                    "{label} transaction start timed out after {:?}",
                    Self::TX_BEGIN_TIMEOUT
                ))
            })?
            .map_err(|e| Error::Internal(format!("{label} transaction start failed: {e}")))
    }

    async fn open_transaction(client: &Client, label: &str) -> Result<(), Error> {
        Self::begin_transaction(client, label).await?;
        Self::set_statement_timeout_local(client).await?;
        Self::deallocate_prepared(client).await?;
        Ok(())
    }

    async fn set_statement_timeout_local(client: &Client) -> Result<(), Error> {
        let sql = format!(
            "SET LOCAL statement_timeout = '{}'",
            Self::PG_STATEMENT_TIMEOUT
        );
        match timeout(
            Self::SET_LOCAL_CMD_TIMEOUT,
            client.batch_execute(sql.as_str()),
        )
        .await
        {
            Ok(Ok(_)) => Ok(()),
            Ok(Err(e)) => {
                error!(error = %e, "SET LOCAL statement_timeout failed");
                Err(Error::Internal(format!(
                    "SET LOCAL statement_timeout failed: {e}"
                )))
            }
            Err(_) => Err(Error::Internal(format!(
                "SET LOCAL statement_timeout timed out after {:?}",
                Self::SET_LOCAL_CMD_TIMEOUT
            ))),
        }
    }

    async fn deallocate_prepared(client: &Client) -> Result<(), Error> {
        match timeout(
            Self::DEALLOCATE_TIMEOUT,
            client.batch_execute("DEALLOCATE ALL"),
        )
        .await
        {
            Ok(Ok(_)) => Ok(()),
            Ok(Err(e)) => Err(Error::Internal(format!("DEALLOCATE ALL failed: {e}"))),
            Err(_) => Err(Error::Internal(format!(
                "DEALLOCATE ALL timed out after {:?}",
                Self::DEALLOCATE_TIMEOUT
            ))),
        }
    }

    fn discard_client(client: Client, label: &str, reason: &str) {
        warn!(label, reason, "discarding db pool client");
        drop(Client::take(client));
    }

    async fn commit_transaction(client: &Client, label: &str) -> Result<(), Error> {
        timeout(Self::QUERY_TIMEOUT, client.batch_execute("COMMIT"))
            .await
            .map_err(|_| {
                Error::Internal(format!(
                    "{label} commit timed out after {:?}",
                    Self::QUERY_TIMEOUT
                ))
            })?
            .map_err(|e| Error::Internal(format!("{label} commit failed: {e}")))
    }

    async fn exec_in_tx(
        &self,
        client: Client,
        sql: &str,
        params: &[&(dyn ToSql + Sync)],
        label: &str,
    ) -> Result<u64, Error> {
        if let Err(e) = Self::open_transaction(&client, label).await {
            Self::discard_client(client, label, "open transaction failed");
            return Err(e);
        }
        let result = match timeout(Self::QUERY_TIMEOUT, client.execute(sql, params)).await {
            Ok(Ok(val)) => val,
            Ok(Err(e)) => {
                Self::discard_client(client, label, "query failed");
                return Err(Error::Internal(format!("{label} query failed: {e}")));
            }
            Err(_) => {
                Self::discard_client(client, label, "query timed out");
                return Err(Error::Internal(format!(
                    "{label} query timed out after {:?}",
                    Self::QUERY_TIMEOUT
                )));
            }
        };
        if let Err(e) = Self::commit_transaction(&client, label).await {
            Self::discard_client(client, label, "commit failed");
            return Err(e);
        }
        // Explicitly drop client to return it to pool
        drop(client);
        Ok(result)
    }

    async fn query_opt_in_tx(
        &self,
        client: Client,
        sql: &str,
        params: &[&(dyn ToSql + Sync)],
        label: &str,
    ) -> Result<Option<tokio_postgres::Row>, Error> {
        if let Err(e) = Self::open_transaction(&client, label).await {
            Self::discard_client(client, label, "open transaction failed");
            return Err(e);
        }
        let result = match timeout(Self::QUERY_TIMEOUT, client.query_opt(sql, params)).await {
            Ok(Ok(val)) => val,
            Ok(Err(e)) => {
                Self::discard_client(client, label, "query failed");
                return Err(Error::Internal(format!("{label} query failed: {e}")));
            }
            Err(_) => {
                Self::discard_client(client, label, "query timed out");
                return Err(Error::Internal(format!(
                    "{label} query timed out after {:?}",
                    Self::QUERY_TIMEOUT
                )));
            }
        };
        if let Err(e) = Self::commit_transaction(&client, label).await {
            Self::discard_client(client, label, "commit failed");
            return Err(e);
        }
        // Explicitly drop client to return it to pool
        drop(client);
        Ok(result)
    }

    async fn query_in_tx(
        &self,
        client: Client,
        sql: &str,
        params: &[&(dyn ToSql + Sync)],
        label: &str,
    ) -> Result<Vec<tokio_postgres::Row>, Error> {
        if let Err(e) = Self::open_transaction(&client, label).await {
            Self::discard_client(client, label, "open transaction failed");
            return Err(e);
        }
        let result = match timeout(Self::QUERY_TIMEOUT, client.query(sql, params)).await {
            Ok(Ok(val)) => val,
            Ok(Err(e)) => {
                Self::discard_client(client, label, "query failed");
                return Err(Error::Internal(format!("{label} query failed: {e}")));
            }
            Err(_) => {
                Self::discard_client(client, label, "query timed out");
                return Err(Error::Internal(format!(
                    "{label} query timed out after {:?}",
                    Self::QUERY_TIMEOUT
                )));
            }
        };
        if let Err(e) = Self::commit_transaction(&client, label).await {
            Self::discard_client(client, label, "commit failed");
            return Err(e);
        }
        // Explicitly drop client to return it to pool
        drop(client);
        Ok(result)
    }
}
