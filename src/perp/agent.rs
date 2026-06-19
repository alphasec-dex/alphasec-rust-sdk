//! PerpAgent sub-facade — wraps PerpApiClient + AlphaSecSigner for perp operations.
//!
//! Access via `agent.perp()`. All trading methods resolve symbol → market_id via a
//! lazily-populated cache (RwLock<HashMap>) backed by GET /fapi/v1/market.
//! Reads are lock-free once the cache is warm; updates take a write lock only on the
//! first resolution of an unknown symbol (or cold start).

use std::{collections::HashMap, sync::Arc};

use rust_decimal::Decimal;
use tokio::sync::RwLock;

use crate::{
    error::{AlphaSecError, Result},
    perp::{
        client::PerpApiClient,
        types::{
            FundingItem, PerpAccount, PerpCandle, PerpDepth, PerpFill, PerpFundingQuery,
            PerpHistoryQuery, PerpMarket, PerpOrder, PerpOrderQuery, PerpTicker, PerpTrade,
            Position, PositionHistory, PositionSetting, TimeInForce, TransferDirection,
        },
    },
    signer::AlphaSecSigner,
    types::orders::OrderSide,
};

// ---------------------------------------------------------------------------
// Mapping helpers
// ---------------------------------------------------------------------------

/// Map `OrderSide` → perp wire side byte (BUY=0, SELL=1).
fn order_side_to_u8(side: OrderSide) -> u8 {
    match side {
        OrderSide::Buy => 0,
        OrderSide::Sell => 1,
    }
}

/// Map `TimeInForce` → perp wire tif byte (GTC=0, IOC=1, POST=2, MARKET=3).
fn tif_to_u8(tif: TimeInForce) -> u8 {
    match tif {
        TimeInForce::Gtc => 0,
        TimeInForce::Ioc => 1,
        TimeInForce::Post => 2,
        TimeInForce::Market => 3,
    }
}

// ---------------------------------------------------------------------------
// Market cache
// ---------------------------------------------------------------------------

/// Lazily-populated symbol → market_id cache.
///
/// Populated on first resolution via GET /fapi/v1/market.  All subsequent reads
/// are contention-free (RwLock read path).
#[derive(Debug, Default)]
pub struct MarketCache(RwLock<HashMap<String, u64>>);

impl MarketCache {
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    /// Resolve symbol to numeric market_id.
    ///
    /// Fast path: read-lock only.
    /// Slow path (cache miss): releases read-lock, fetches markets, takes write-lock.
    async fn resolve(&self, symbol: &str, client: &PerpApiClient) -> Result<u64> {
        // Fast path: read lock only — avoids exclusive lock on hot path.
        {
            let cache = self.0.read().await;
            if let Some(&id) = cache.get(symbol) {
                return Ok(id);
            }
        }

        // Slow path: fetch markets and populate the cache.
        match client.get_markets().await {
            Ok(markets) => {
                let mut cache = self.0.write().await;
                for m in &markets {
                    if let Ok(id) = m.market_id.parse::<u64>() {
                        cache.insert(m.symbol.clone(), id);
                    }
                }
                // get_markets() succeeded: a missing symbol genuinely does not exist.
                cache.get(symbol).copied().ok_or_else(|| {
                    AlphaSecError::invalid_parameter(format!("Unknown perp symbol: {}", symbol))
                })
            }
            Err(e) => {
                // Market refresh failed (network/API). If a previous successful fetch already
                // cached this symbol, serve it (graceful degradation). Otherwise propagate the
                // real transport/API error: masking it as InvalidParameter would tell the caller
                // "symbol doesn't exist" when the truth is "couldn't reach the server" — corrupting
                // their retry decision and discarding the root cause.
                // Serve a previously-cached id if present; otherwise surface the real error.
                let cache = self.0.read().await;
                cache.get(symbol).copied().ok_or(e)
            }
        }
    }
}

// ---------------------------------------------------------------------------
// PerpAgent
// ---------------------------------------------------------------------------

/// Sub-facade for all perpetual futures operations.
///
/// Borrowed from `Agent`; constructed via `Agent::perp()`.
/// All trading methods sign locally and submit via REST to `/fapi/v1/*`.
pub struct PerpAgent<'a> {
    signer: &'a AlphaSecSigner,
    client: &'a PerpApiClient,
    address: &'a str,
    cache: Arc<MarketCache>,
}

impl<'a> PerpAgent<'a> {
    /// Create a new PerpAgent (called by `Agent::perp()`).
    pub fn new(
        signer: &'a AlphaSecSigner,
        client: &'a PerpApiClient,
        address: &'a str,
        cache: Arc<MarketCache>,
    ) -> Self {
        Self {
            signer,
            client,
            address,
            cache,
        }
    }

    // -----------------------------------------------------------------------
    // Internal: sign and submit helpers
    // -----------------------------------------------------------------------

    async fn sign_and_submit<F>(&self, build_wire: F, submit: &str) -> Result<String>
    where
        F: FnOnce() -> Result<Vec<u8>>,
    {
        let wire = build_wire()?;
        let signed_tx = self
            .signer
            .generate_alphasec_transaction(None, &wire, None)
            .await?;
        match submit {
            "order" => self.client.order(&signed_tx).await,
            "cancel" => self.client.cancel(&signed_tx).await,
            "cancel_all" => self.client.cancel_all(&signed_tx).await,
            "modify" => self.client.modify(&signed_tx).await,
            "deposit" => self.client.deposit(&signed_tx).await,
            "withdraw" => self.client.withdraw(&signed_tx).await,
            "set_leverage" => self.client.set_leverage(&signed_tx).await,
            _ => Err(AlphaSecError::invalid_parameter(format!(
                "unknown submit endpoint: {}",
                submit
            ))),
        }
    }

    // -----------------------------------------------------------------------
    // Trading methods
    // -----------------------------------------------------------------------

    /// Place a new perp limit/market order.
    ///
    /// `symbol` is resolved to `market_id` via the lazy market cache.
    pub async fn order(
        &self,
        symbol: &str,
        side: OrderSide,
        price: Decimal,
        quantity: Decimal,
        tif: TimeInForce,
        reduce_only: bool,
        client_order_id: Option<&str>,
    ) -> Result<String> {
        let market_id = self.cache.resolve(symbol, self.client).await?;
        self.sign_and_submit(
            || {
                self.signer.create_perp_order_data(
                    market_id,
                    order_side_to_u8(side),
                    price,
                    quantity,
                    reduce_only,
                    tif_to_u8(tif),
                    client_order_id,
                )
            },
            "order",
        )
        .await
    }

    /// Cancel an open perp order by order ID.
    pub async fn cancel(&self, symbol: &str, order_id: &str) -> Result<String> {
        let market_id = self.cache.resolve(symbol, self.client).await?;
        self.sign_and_submit(
            || self.signer.create_perp_cancel_data(market_id, order_id),
            "cancel",
        )
        .await
    }

    /// Cancel all open perp orders for a symbol.
    pub async fn cancel_all(&self, symbol: &str) -> Result<String> {
        let market_id = self.cache.resolve(symbol, self.client).await?;
        self.sign_and_submit(
            || self.signer.create_perp_cancel_all_data(market_id),
            "cancel_all",
        )
        .await
    }

    /// Modify (amend) an open perp order via cancel-and-replace (0x4A).
    ///
    /// `None` fields are omitted from the wire → server inherits the existing value.
    pub async fn modify(
        &self,
        symbol: &str,
        order_id: &str,
        new_price: Option<Decimal>,
        new_quantity: Option<Decimal>,
        client_order_id: Option<&str>,
    ) -> Result<String> {
        let market_id = self.cache.resolve(symbol, self.client).await?;
        self.sign_and_submit(
            || {
                self.signer.create_perp_modify_data(
                    market_id,
                    order_id,
                    new_price,
                    new_quantity,
                    client_order_id,
                )
            },
            "modify",
        )
        .await
    }

    // -----------------------------------------------------------------------
    // Funds / leverage
    // -----------------------------------------------------------------------

    /// Transfer funds between Spot and Perp wallets.
    ///
    /// `SpotToPerp` → `create_perp_deposit_data` (0x12) → POST /fapi/v1/wallet/deposit.
    /// `PerpToSpot` → `create_perp_withdraw_data` (0x44) → POST /fapi/v1/wallet/withdraw.
    pub async fn transfer(
        &self,
        direction: TransferDirection,
        token: &str,
        amount: Decimal,
    ) -> Result<String> {
        let (wire, endpoint) = self.build_transfer_wire(direction, token, amount)?;
        self.sign_and_submit(move || Ok(wire), endpoint).await
    }

    /// Select the signed wire bytes and submit endpoint for a transfer `direction`.
    ///
    /// Single source of truth shared by `transfer()` and its unit test, so a
    /// direction → command mismatch (which would route funds to the wrong wallet)
    /// is caught without a live submit:
    /// `SpotToPerp` → deposit wire (0x12) + `"deposit"`,
    /// `PerpToSpot` → withdraw wire (0x44) + `"withdraw"`.
    fn build_transfer_wire(
        &self,
        direction: TransferDirection,
        token: &str,
        amount: Decimal,
    ) -> Result<(Vec<u8>, &'static str)> {
        match direction {
            TransferDirection::SpotToPerp => Ok((
                self.signer.create_perp_deposit_data(token, amount)?,
                "deposit",
            )),
            TransferDirection::PerpToSpot => Ok((
                self.signer.create_perp_withdraw_data(token, amount)?,
                "withdraw",
            )),
        }
    }

    /// Set leverage for a symbol.
    pub async fn set_leverage(&self, symbol: &str, leverage: u32) -> Result<String> {
        let market_id = self.cache.resolve(symbol, self.client).await?;
        self.sign_and_submit(
            || {
                self.signer
                    .create_perp_set_leverage_data(market_id, leverage)
            },
            "set_leverage",
        )
        .await
    }

    // -----------------------------------------------------------------------
    // Account / position queries
    // -----------------------------------------------------------------------

    /// Get perp account balances and risk aggregates.
    pub async fn get_account(&self) -> Result<PerpAccount> {
        self.client.get_account(self.address).await
    }

    /// Get all open positions.
    pub async fn get_positions(&self) -> Result<Vec<Position>> {
        self.client.get_positions(self.address).await
    }

    /// Get position lifecycle history.
    pub async fn get_position_history(&self, q: PerpHistoryQuery) -> Result<Vec<PositionHistory>> {
        self.client.get_position_history(self.address, &q).await
    }

    /// Get per-market leverage/margin-mode settings.
    pub async fn get_position_settings(&self) -> Result<Vec<PositionSetting>> {
        self.client.get_position_settings(self.address).await
    }

    /// Get funding payment history.
    pub async fn get_funding(&self, q: PerpFundingQuery) -> Result<Vec<FundingItem>> {
        self.client.get_funding(self.address, &q).await
    }

    // -----------------------------------------------------------------------
    // Order queries
    // -----------------------------------------------------------------------

    /// Get open orders.
    pub async fn get_open_orders(&self, q: PerpOrderQuery) -> Result<Vec<PerpOrder>> {
        self.client.get_open_orders(self.address, &q).await
    }

    /// Get order history (filled / cancelled).
    pub async fn get_order_history(&self, q: PerpOrderQuery) -> Result<Vec<PerpOrder>> {
        self.client.get_order_history(self.address, &q).await
    }

    /// Get a single order by ID.
    pub async fn get_order(&self, order_id: &str) -> Result<PerpOrder> {
        self.client.get_order(order_id).await
    }

    /// Get orders submitted in a given transaction.
    pub async fn get_order_list(&self, tx_hash: &str) -> Result<Vec<PerpOrder>> {
        self.client.get_order_list(tx_hash).await
    }

    /// Get personal trade history (fills).
    pub async fn get_my_trades(&self, q: PerpOrderQuery) -> Result<Vec<PerpFill>> {
        self.client.get_my_trades(self.address, &q).await
    }

    // -----------------------------------------------------------------------
    // Market data queries
    // -----------------------------------------------------------------------

    /// Get all perp markets.
    pub async fn get_markets(&self) -> Result<Vec<PerpMarket>> {
        self.client.get_markets().await
    }

    /// Get tickers for all markets.
    pub async fn get_tickers(&self) -> Result<Vec<PerpTicker>> {
        self.client.get_tickers().await
    }

    /// Get ticker for a specific symbol.
    pub async fn get_ticker(&self, symbol: &str) -> Result<PerpTicker> {
        let market_id = self.cache.resolve(symbol, self.client).await?;
        let tickers = self.client.get_ticker(&market_id.to_string()).await?;
        tickers.into_iter().next().ok_or_else(|| {
            AlphaSecError::not_found(format!("No ticker found for symbol: {}", symbol))
        })
    }

    /// Get order book depth snapshot.
    pub async fn get_depth(&self, symbol: &str, limit: Option<u32>) -> Result<PerpDepth> {
        let market_id = self.cache.resolve(symbol, self.client).await?;
        self.client.get_depth(&market_id.to_string(), limit).await
    }

    /// Get recent public trades.
    pub async fn get_market_trades(
        &self,
        symbol: &str,
        limit: Option<u32>,
    ) -> Result<Vec<PerpTrade>> {
        let market_id = self.cache.resolve(symbol, self.client).await?;
        self.client
            .get_market_trades(&market_id.to_string(), limit)
            .await
    }

    /// Get OHLCV candles.
    pub async fn get_candles(
        &self,
        symbol: &str,
        resolution: &str,
        from: Option<i64>,
        to: Option<i64>,
    ) -> Result<Vec<PerpCandle>> {
        let market_id = self.cache.resolve(symbol, self.client).await?;
        self.client
            .get_candles(&market_id.to_string(), resolution, from, to)
            .await
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    use crate::{
        perp::client::PerpApiClient,
        signer::{AlphaSecSigner, Config},
    };

    fn make_test_signer() -> AlphaSecSigner {
        let config = Config::new(
            "https://api-testnet.alphasec.trade",
            "kairos",
            "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266",
            Some("ac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80"),
            None,
            false,
            None,
        )
        .unwrap();
        AlphaSecSigner::new(config)
    }

    fn make_test_client() -> PerpApiClient {
        let config = Config::new(
            "https://api-testnet.alphasec.trade",
            "kairos",
            "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266",
            Some("ac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80"),
            None,
            false,
            None,
        )
        .unwrap();
        PerpApiClient::new(&config).unwrap()
    }

    /// A client whose base URL refuses connections (loopback port 1) — drives the
    /// cold-fetch error path deterministically and offline (ECONNREFUSED).
    fn make_unreachable_client() -> PerpApiClient {
        let config = Config::new(
            "http://127.0.0.1:1",
            "kairos",
            "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266",
            Some("ac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80"),
            None,
            false,
            None,
        )
        .unwrap()
        .with_timeout(2);
        PerpApiClient::new(&config).unwrap()
    }

    /// Pre-populate the market cache with given entries, bypassing network.
    async fn make_cache_with(entries: &[(&str, u64)]) -> Arc<MarketCache> {
        let cache = MarketCache::new();
        let mut map = cache.0.write().await;
        for (sym, id) in entries {
            map.insert(sym.to_string(), *id);
        }
        drop(map);
        cache
    }

    #[test]
    fn order_side_maps_to_exact_wire_bytes() {
        assert_eq!(order_side_to_u8(OrderSide::Buy), 0, "BUY must encode as 0");
        assert_eq!(
            order_side_to_u8(OrderSide::Sell),
            1,
            "SELL must encode as 1"
        );
    }

    #[test]
    fn tif_maps_to_exact_wire_bytes() {
        assert_eq!(tif_to_u8(TimeInForce::Gtc), 0, "GTC must encode as 0");
        assert_eq!(tif_to_u8(TimeInForce::Ioc), 1, "IOC must encode as 1");
        assert_eq!(tif_to_u8(TimeInForce::Post), 2, "POST must encode as 2");
        assert_eq!(tif_to_u8(TimeInForce::Market), 3, "MARKET must encode as 3");
    }

    #[tokio::test]
    async fn warm_cache_resolves_correct_symbol_without_network() {
        let cache = make_cache_with(&[("BTCUSDT", 1), ("ETHUSDT", 2)]).await;
        // Unreachable: any network access during a cache hit would surface as an Err here.
        let client = make_unreachable_client();

        assert_eq!(
            cache.resolve("BTCUSDT", &client).await.unwrap(),
            1,
            "BTCUSDT must resolve to its own cached id (1), not another entry"
        );
        assert_eq!(
            cache.resolve("ETHUSDT", &client).await.unwrap(),
            2,
            "ETHUSDT must resolve to its own cached id (2), not another entry"
        );
    }

    #[tokio::test]
    async fn uncached_symbol_with_failed_fetch_errors_without_masking() {
        // "NOPE" is absent → forces the cold fetch, which fails against the unreachable client.
        let cache = make_cache_with(&[("BTCUSDT", 1)]).await;
        let client = make_unreachable_client();

        let result = cache.resolve("NOPE", &client).await;

        // (a) no fabricated id leaks through (it would be submitted against the wrong market).
        assert!(
            result.is_err(),
            "uncached symbol must not resolve to an id; got: {:?}",
            result
        );
        // (b) the failure is the real transport error, not masked as a bogus "unknown symbol".
        assert!(
            !matches!(result, Err(AlphaSecError::InvalidParameter(_))),
            "transport failure must not be masked as InvalidParameter; got: {:?}",
            result
        );
    }

    // TODO(network-failure-kinds): the test above drives ONE transport failure (connection
    // refused) via an unreachable client, proving the "don't mask, propagate" contract offline.
    // Asserting that the EXACT original error kind survives across the full surface (request
    // timeout, HTTP 5xx envelope, malformed-body decode) requires injecting arbitrary errors
    // into get_markets() — i.e. a PerpApiClient trait + mock. Deferred: per the test plan we
    // avoid a heavy client-trait refactor here. Add once a client seam exists.

    #[tokio::test]
    async fn transfer_routes_direction_to_correct_command_and_endpoint() {
        let signer = make_test_signer();
        let client = make_test_client();
        let cache = MarketCache::new();
        let address = signer.l1_address();
        let agent = PerpAgent::new(&signer, &client, address, cache);

        let amount = Decimal::from_str("100").unwrap();

        let (deposit_wire, deposit_ep) = agent
            .build_transfer_wire(TransferDirection::SpotToPerp, "2", amount)
            .unwrap();
        assert_eq!(
            deposit_ep, "deposit",
            "SpotToPerp must submit to the deposit endpoint"
        );
        assert_eq!(
            deposit_wire[0], 0x12,
            "SpotToPerp must build the 0x12 deposit wire; got 0x{:02X}",
            deposit_wire[0]
        );

        let (withdraw_wire, withdraw_ep) = agent
            .build_transfer_wire(TransferDirection::PerpToSpot, "2", amount)
            .unwrap();
        assert_eq!(
            withdraw_ep, "withdraw",
            "PerpToSpot must submit to the withdraw endpoint"
        );
        assert_eq!(
            withdraw_wire[0], 0x44,
            "PerpToSpot must build the 0x44 withdraw wire; got 0x{:02X}",
            withdraw_wire[0]
        );
    }
}
