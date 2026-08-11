use chrono::Duration;
use rust_decimal::Decimal;
use trading_domain::Money;

#[derive(Debug, Clone, Copy)]
pub struct RiskLimits {
    pub maximum_quote_age: Duration,
    pub maximum_order_value: Money,
    pub maximum_position_exposure: Money,
    pub maximum_strategy_exposure: Money,
    pub maximum_portfolio_exposure: Money,
    pub maximum_daily_loss: Money,
    pub maximum_drawdown: Money,
    pub maximum_spread_fraction: Decimal,
}
