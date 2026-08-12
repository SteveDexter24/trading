use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use trading_domain::{DomainError, Fill, Money};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PerformanceReport {
    pub cagr: Decimal,
    pub volatility: Decimal,
    pub sharpe: Decimal,
    pub sortino: Decimal,
    pub max_drawdown: Decimal,
    pub turnover: Decimal,
    pub hit_rate: Decimal,
    pub average_exposure: Decimal,
    pub total_costs: Money,
    pub benchmark_return: Decimal,
    pub strategy_return: Decimal,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EquityPoint {
    pub equity: Money,
    pub benchmark: Money,
}

pub fn summarize_equity_curve(
    points: &[EquityPoint],
    fills: &[Fill],
    years: Decimal,
) -> Result<PerformanceReport, DomainError> {
    if points.len() < 2 {
        return Err(DomainError::Adapter(
            "equity curve requires at least two points".to_owned(),
        ));
    }
    let start = points.first().expect("checked length").equity;
    let end = points.last().expect("checked length").equity;
    let currency = start.currency();
    let strategy_return = relative_return(start, end)?;
    let benchmark_return = relative_return(
        points.first().expect("checked").benchmark,
        points.last().expect("checked").benchmark,
    )?;

    let period_returns = period_returns(points)?;
    let volatility = sample_std_dev(&period_returns)?;
    let downside = sample_std_dev(
        &period_returns
            .iter()
            .copied()
            .filter(|value| *value < Decimal::ZERO)
            .collect::<Vec<_>>(),
    )?;
    let average_return = mean(&period_returns);
    let sharpe = safe_div(average_return, volatility);
    let sortino = safe_div(average_return, downside);
    let max_drawdown = max_drawdown(points)?;
    let total_costs = fills
        .iter()
        .try_fold(Money::zero(currency), |acc, fill| acc.checked_add(fill.fee))?;
    let traded_notional = fills.iter().try_fold(Money::zero(currency), |acc, fill| {
        acc.checked_add(fill.price.times(fill.quantity, fill.currency)?)
    })?;
    let turnover = safe_div(traded_notional.amount(), start.amount().max(Decimal::ONE));
    let wins = fills
        .iter()
        .filter(|fill| fill.price.value() > Decimal::ZERO)
        .count();
    let hit_rate = if fills.is_empty() {
        Decimal::ZERO
    } else {
        Decimal::from(wins) / Decimal::from(fills.len())
    };
    let average_exposure = mean(
        &points
            .iter()
            .map(|point| safe_div(point.equity.amount(), start.amount().max(Decimal::ONE)))
            .collect::<Vec<_>>(),
    );
    let cagr = if years > Decimal::ZERO {
        // Approximate CAGR from total return / years for decimal-only research reports.
        strategy_return / years
    } else {
        Decimal::ZERO
    };

    Ok(PerformanceReport {
        cagr,
        volatility,
        sharpe,
        sortino,
        max_drawdown,
        turnover,
        hit_rate,
        average_exposure,
        total_costs,
        benchmark_return,
        strategy_return,
    })
}

fn relative_return(start: Money, end: Money) -> Result<Decimal, DomainError> {
    let start_amount = start.amount().max(Decimal::ONE);
    Ok((end.amount() - start.amount()) / start_amount)
}

fn period_returns(points: &[EquityPoint]) -> Result<Vec<Decimal>, DomainError> {
    points
        .windows(2)
        .map(|window| relative_return(window[0].equity, window[1].equity))
        .collect()
}

fn max_drawdown(points: &[EquityPoint]) -> Result<Decimal, DomainError> {
    let mut peak = points[0].equity.amount();
    let mut max_dd = Decimal::ZERO;
    for point in points {
        peak = peak.max(point.equity.amount());
        if peak > Decimal::ZERO {
            let dd = (peak - point.equity.amount()) / peak;
            max_dd = max_dd.max(dd);
        }
    }
    Ok(max_dd)
}

fn mean(values: &[Decimal]) -> Decimal {
    if values.is_empty() {
        Decimal::ZERO
    } else {
        values.iter().copied().sum::<Decimal>() / Decimal::from(values.len())
    }
}

fn sample_std_dev(values: &[Decimal]) -> Result<Decimal, DomainError> {
    if values.len() < 2 {
        return Ok(Decimal::ZERO);
    }
    let avg = mean(values);
    let variance = values
        .iter()
        .map(|value| {
            let delta = *value - avg;
            delta * delta
        })
        .sum::<Decimal>()
        / Decimal::from(values.len() - 1);
    sqrt_decimal(variance)
}

fn safe_div(numerator: Decimal, denominator: Decimal) -> Decimal {
    if denominator.is_zero() {
        Decimal::ZERO
    } else {
        numerator / denominator
    }
}

fn sqrt_decimal(value: Decimal) -> Result<Decimal, DomainError> {
    if value.is_sign_negative() {
        return Err(DomainError::Adapter("negative variance".to_owned()));
    }
    if value.is_zero() {
        return Ok(Decimal::ZERO);
    }
    // Newton iteration keeps research metrics on Decimal instead of f64.
    let mut guess = value;
    for _ in 0..16 {
        guess = (guess + value / guess) / Decimal::TWO;
    }
    Ok(guess)
}

#[cfg(test)]
mod tests {
    use super::*;
    use trading_domain::Currency;

    #[test]
    fn reports_drawdown_and_costs() {
        let currency = Currency::Usd;
        let points = vec![
            EquityPoint {
                equity: Money::new(Decimal::new(100, 0), currency).expect("cash"),
                benchmark: Money::new(Decimal::new(100, 0), currency).expect("cash"),
            },
            EquityPoint {
                equity: Money::new(Decimal::new(110, 0), currency).expect("cash"),
                benchmark: Money::new(Decimal::new(105, 0), currency).expect("cash"),
            },
            EquityPoint {
                equity: Money::new(Decimal::new(90, 0), currency).expect("cash"),
                benchmark: Money::new(Decimal::new(106, 0), currency).expect("cash"),
            },
        ];
        let report = summarize_equity_curve(&points, &[], Decimal::ONE).expect("report");
        assert!(report.max_drawdown > Decimal::ZERO);
        assert_eq!(report.total_costs, Money::zero(currency));
    }
}
