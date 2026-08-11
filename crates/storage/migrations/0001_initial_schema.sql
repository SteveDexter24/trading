CREATE TABLE instruments (
    id UUID PRIMARY KEY,
    symbol TEXT NOT NULL,
    exchange TEXT NOT NULL CHECK (exchange IN ('HKEX', 'NASDAQ', 'NYSE')),
    currency TEXT NOT NULL CHECK (currency IN ('HKD', 'USD')),
    exchange_timezone TEXT NOT NULL,
    board_lot NUMERIC NOT NULL CHECK (board_lot > 0),
    tick_size NUMERIC NOT NULL CHECK (tick_size > 0),
    fractional_supported BOOLEAN NOT NULL DEFAULT FALSE,
    broker_metadata_observed_at TIMESTAMPTZ,
    UNIQUE (symbol, exchange)
);

CREATE TABLE bars (
    event_id TEXT PRIMARY KEY,
    instrument_id UUID NOT NULL REFERENCES instruments(id),
    interval TEXT NOT NULL,
    open NUMERIC NOT NULL CHECK (open > 0),
    high NUMERIC NOT NULL CHECK (high > 0),
    low NUMERIC NOT NULL CHECK (low > 0),
    close NUMERIC NOT NULL CHECK (close > 0),
    volume NUMERIC NOT NULL CHECK (volume >= 0),
    starts_at TIMESTAMPTZ NOT NULL,
    ends_at TIMESTAMPTZ NOT NULL,
    trading_date DATE NOT NULL,
    observed_at TIMESTAMPTZ NOT NULL,
    effective_at TIMESTAMPTZ NOT NULL,
    source TEXT NOT NULL,
    ingested_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CHECK (starts_at < ends_at),
    CHECK (observed_at >= effective_at)
);

CREATE INDEX bars_point_in_time_idx
    ON bars (instrument_id, ends_at, observed_at);

CREATE TABLE quotes (
    event_id TEXT PRIMARY KEY,
    instrument_id UUID NOT NULL REFERENCES instruments(id),
    bid NUMERIC NOT NULL CHECK (bid > 0),
    ask NUMERIC NOT NULL CHECK (ask >= bid),
    sequence BIGINT NOT NULL CHECK (sequence >= 0),
    occurred_at TIMESTAMPTZ NOT NULL,
    trading_date DATE NOT NULL,
    observed_at TIMESTAMPTZ NOT NULL,
    effective_at TIMESTAMPTZ NOT NULL,
    source TEXT NOT NULL,
    ingested_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CHECK (observed_at >= occurred_at),
    UNIQUE (instrument_id, source, sequence)
);

CREATE INDEX quotes_latest_idx
    ON quotes (instrument_id, observed_at DESC);

CREATE TABLE corporate_actions (
    id UUID PRIMARY KEY,
    instrument_id UUID NOT NULL REFERENCES instruments(id),
    action_type TEXT NOT NULL,
    version INTEGER NOT NULL CHECK (version > 0),
    factor NUMERIC,
    cash_amount NUMERIC,
    currency TEXT CHECK (currency IN ('HKD', 'USD')),
    trading_date DATE NOT NULL,
    observed_at TIMESTAMPTZ NOT NULL,
    effective_at TIMESTAMPTZ NOT NULL,
    source TEXT NOT NULL,
    ingested_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    payload JSONB NOT NULL,
    UNIQUE (instrument_id, action_type, effective_at, version)
);

CREATE TABLE fundamental_observations (
    id UUID PRIMARY KEY,
    instrument_id UUID NOT NULL REFERENCES instruments(id),
    metric TEXT NOT NULL,
    value NUMERIC NOT NULL,
    currency TEXT CHECK (currency IN ('HKD', 'USD')),
    period_end DATE NOT NULL,
    revision INTEGER NOT NULL CHECK (revision > 0),
    observed_at TIMESTAMPTZ NOT NULL,
    effective_at TIMESTAMPTZ NOT NULL,
    source TEXT NOT NULL,
    ingested_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CHECK (observed_at >= effective_at),
    UNIQUE (instrument_id, metric, period_end, revision, source)
);

CREATE TABLE news_articles (
    id UUID PRIMARY KEY,
    source_article_id TEXT NOT NULL,
    headline TEXT NOT NULL,
    body TEXT,
    published_at TIMESTAMPTZ NOT NULL,
    observed_at TIMESTAMPTZ NOT NULL,
    effective_at TIMESTAMPTZ NOT NULL,
    source TEXT NOT NULL,
    ingested_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CHECK (observed_at >= published_at),
    UNIQUE (source, source_article_id)
);

CREATE TABLE news_entities (
    article_id UUID NOT NULL REFERENCES news_articles(id),
    instrument_id UUID NOT NULL REFERENCES instruments(id),
    relevance NUMERIC NOT NULL CHECK (relevance BETWEEN 0 AND 1),
    observed_at TIMESTAMPTZ NOT NULL,
    PRIMARY KEY (article_id, instrument_id)
);

CREATE TABLE model_versions (
    id UUID PRIMARY KEY,
    name TEXT NOT NULL,
    version TEXT NOT NULL,
    feature_version TEXT NOT NULL,
    artifact_digest TEXT NOT NULL,
    training_cutoff_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    metadata JSONB NOT NULL,
    UNIQUE (name, version)
);

CREATE TABLE features (
    id UUID PRIMARY KEY,
    instrument_id UUID NOT NULL REFERENCES instruments(id),
    feature_version TEXT NOT NULL,
    values JSONB NOT NULL,
    observed_at TIMESTAMPTZ NOT NULL,
    effective_at TIMESTAMPTZ NOT NULL,
    source TEXT NOT NULL,
    ingested_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    CHECK (observed_at >= effective_at),
    UNIQUE (instrument_id, feature_version, effective_at, observed_at)
);

CREATE TABLE strategy_signals (
    id UUID PRIMARY KEY,
    strategy_id TEXT NOT NULL,
    instrument_id UUID NOT NULL REFERENCES instruments(id),
    side TEXT NOT NULL CHECK (side IN ('Buy', 'Sell')),
    generated_at TIMESTAMPTZ NOT NULL,
    feature_id UUID REFERENCES features(id),
    model_version_id UUID REFERENCES model_versions(id),
    probability NUMERIC CHECK (probability BETWEEN 0 AND 1),
    uncertainty NUMERIC CHECK (uncertainty BETWEEN 0 AND 1),
    expected_horizon_seconds BIGINT CHECK (expected_horizon_seconds > 0),
    payload JSONB NOT NULL
);

CREATE TABLE order_intents (
    id UUID PRIMARY KEY,
    client_order_id UUID NOT NULL UNIQUE,
    strategy_id TEXT NOT NULL,
    instrument_id UUID NOT NULL,
    created_at TIMESTAMPTZ NOT NULL,
    payload JSONB NOT NULL
);

CREATE TABLE risk_decisions (
    id UUID PRIMARY KEY,
    order_intent_id UUID NOT NULL UNIQUE REFERENCES order_intents(id),
    approved BOOLEAN NOT NULL,
    evaluated_at TIMESTAMPTZ NOT NULL,
    payload JSONB NOT NULL
);

CREATE TABLE orders (
    id UUID PRIMARY KEY,
    client_order_id UUID NOT NULL UNIQUE,
    order_intent_id UUID NOT NULL UNIQUE REFERENCES order_intents(id),
    risk_decision_id UUID NOT NULL REFERENCES risk_decisions(id),
    status TEXT NOT NULL,
    broker_order_id TEXT UNIQUE,
    submitted_at TIMESTAMPTZ,
    updated_at TIMESTAMPTZ NOT NULL,
    payload JSONB NOT NULL
);

CREATE TABLE fills (
    id UUID PRIMARY KEY,
    event_id TEXT NOT NULL UNIQUE,
    order_id UUID NOT NULL REFERENCES orders(id),
    filled_at TIMESTAMPTZ NOT NULL,
    payload JSONB NOT NULL
);

CREATE TABLE positions (
    strategy_id TEXT NOT NULL,
    instrument_id UUID NOT NULL REFERENCES instruments(id),
    quantity NUMERIC NOT NULL,
    average_price NUMERIC NOT NULL CHECK (average_price > 0),
    observed_at TIMESTAMPTZ NOT NULL,
    source TEXT NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL,
    PRIMARY KEY (strategy_id, instrument_id)
);

CREATE TABLE cash_balances (
    currency TEXT NOT NULL CHECK (currency IN ('HKD', 'USD')),
    settled NUMERIC NOT NULL CHECK (settled >= 0),
    unsettled NUMERIC NOT NULL CHECK (unsettled >= 0),
    observed_at TIMESTAMPTZ NOT NULL,
    source TEXT NOT NULL,
    PRIMARY KEY (currency, observed_at)
);

CREATE TABLE fx_rates (
    base_currency TEXT NOT NULL CHECK (base_currency IN ('HKD', 'USD')),
    quote_currency TEXT NOT NULL CHECK (quote_currency IN ('HKD', 'USD')),
    rate NUMERIC NOT NULL CHECK (rate > 0),
    conversion_cost NUMERIC NOT NULL CHECK (conversion_cost >= 0),
    observed_at TIMESTAMPTZ NOT NULL,
    effective_at TIMESTAMPTZ NOT NULL,
    source TEXT NOT NULL,
    ingested_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (base_currency, quote_currency, observed_at),
    CHECK (base_currency <> quote_currency)
);

CREATE TABLE portfolio_snapshots (
    id UUID PRIMARY KEY,
    net_asset_value NUMERIC NOT NULL CHECK (net_asset_value >= 0),
    reporting_currency TEXT NOT NULL CHECK (reporting_currency IN ('HKD', 'USD')),
    observed_at TIMESTAMPTZ NOT NULL,
    source TEXT NOT NULL,
    payload JSONB NOT NULL
);

CREATE TABLE system_events (
    event_id TEXT PRIMARY KEY,
    event_type TEXT NOT NULL,
    payload JSONB NOT NULL,
    occurred_at TIMESTAMPTZ NOT NULL,
    ingested_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
