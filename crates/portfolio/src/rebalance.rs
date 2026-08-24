use crate::model::{
    checked_add, checked_mul, HoldingSnapshot, PortfolioConstructionError, PortfolioPlan,
    ProposedTrade, RebalanceConstraints, RebalancePlan,
};
use rust_decimal::Decimal;
use std::collections::{BTreeMap, BTreeSet};
use trading_domain::{Instrument, Money, Price, Quantity, Side};

#[derive(Debug)]
struct DraftTrade {
    instrument: Instrument,
    side: Side,
    quantity: Decimal,
    reference_price: Price,
    target_weight: Decimal,
}

pub fn plan_rebalance(
    target: &PortfolioPlan,
    holdings: &[HoldingSnapshot],
    constraints: RebalanceConstraints,
) -> Result<RebalancePlan, PortfolioConstructionError> {
    validate_rebalance_inputs(target, holdings, constraints)?;

    let currency = target.total_equity.currency();
    let target_by_id = target
        .allocations
        .iter()
        .map(|allocation| (allocation.instrument.id, allocation))
        .collect::<BTreeMap<_, _>>();
    let holdings_by_id = holdings
        .iter()
        .map(|holding| (holding.instrument.id, holding))
        .collect::<BTreeMap<_, _>>();
    let instrument_ids = target_by_id
        .keys()
        .chain(holdings_by_id.keys())
        .copied()
        .collect::<BTreeSet<_>>();

    let mut drafts = Vec::new();
    for instrument_id in instrument_ids {
        let target_allocation = target_by_id.get(&instrument_id).copied();
        let holding = holdings_by_id.get(&instrument_id).copied();
        let instrument = target_allocation
            .map(|allocation| allocation.instrument.clone())
            .or_else(|| holding.map(|snapshot| snapshot.instrument.clone()))
            .ok_or_else(|| {
                PortfolioConstructionError::MissingReferencePrice(instrument_id.to_string())
            })?;
        let reference_price = target_allocation
            .map(|allocation| allocation.reference_price)
            .or_else(|| holding.map(|snapshot| snapshot.reference_price))
            .ok_or_else(|| {
                PortfolioConstructionError::MissingReferencePrice(instrument.symbol.clone())
            })?;
        let desired_quantity = target_allocation.map_or(Decimal::ZERO, |allocation| {
            round_to_lot(
                allocation.target_value.amount() / reference_price.value(),
                &instrument,
            )
        });
        let current_quantity = holding.map_or(Decimal::ZERO, |snapshot| snapshot.quantity.value());
        let difference = desired_quantity - current_quantity;
        if difference == Decimal::ZERO {
            continue;
        }
        drafts.push(DraftTrade {
            instrument,
            side: if difference.is_sign_positive() {
                Side::Buy
            } else {
                Side::Sell
            },
            quantity: difference.abs(),
            reference_price,
            target_weight: target_allocation
                .map_or(Decimal::ZERO, |allocation| allocation.target_weight),
        });
    }

    let gross_before_scaling = gross_notional(&drafts)?;
    let maximum_turnover = checked_mul(
        target.total_equity.amount(),
        constraints.maximum_turnover_weight,
        "turnover limit overflow",
    )?;
    let scale = if gross_before_scaling > maximum_turnover && gross_before_scaling > Decimal::ZERO {
        maximum_turnover / gross_before_scaling
    } else {
        Decimal::ONE
    };

    let mut trades = Vec::new();
    for draft in drafts {
        let scaled_quantity = round_to_lot(draft.quantity * scale, &draft.instrument);
        if scaled_quantity <= Decimal::ZERO {
            continue;
        }
        let notional = checked_mul(
            scaled_quantity,
            draft.reference_price.value(),
            "trade notional overflow",
        )?;
        if notional < constraints.minimum_trade_notional.amount() {
            continue;
        }
        trades.push(ProposedTrade {
            instrument: draft.instrument,
            side: draft.side,
            quantity: Quantity::new(scaled_quantity)?,
            reference_price: draft.reference_price,
            estimated_notional: Money::new(notional, currency)?,
            target_weight: draft.target_weight,
        });
    }
    // Propose sells first so an imperative execution shell can release cash
    // before submitting buys. Every trade still receives independent risk approval.
    trades.sort_by_key(|trade| (side_rank(trade.side), trade.instrument.id));

    let turnover_amount = trades.iter().try_fold(Decimal::ZERO, |total, trade| {
        checked_add(
            total,
            trade.estimated_notional.amount(),
            "turnover sum overflow",
        )
    })?;
    let turnover_weight = if target.total_equity.amount() > Decimal::ZERO {
        turnover_amount / target.total_equity.amount()
    } else {
        Decimal::ZERO
    };

    Ok(RebalancePlan {
        as_of: target.as_of,
        currency,
        turnover_weight,
        estimated_turnover: Money::new(turnover_amount, currency)?,
        trades,
    })
}

fn validate_rebalance_inputs(
    target: &PortfolioPlan,
    holdings: &[HoldingSnapshot],
    constraints: RebalanceConstraints,
) -> Result<(), PortfolioConstructionError> {
    if constraints.minimum_trade_notional.currency() != target.total_equity.currency() {
        return Err(PortfolioConstructionError::InvalidConstraint(
            "minimum trade notional and portfolio currencies differ".to_owned(),
        ));
    }
    if constraints.maximum_turnover_weight < Decimal::ZERO
        || constraints.maximum_turnover_weight > Decimal::new(2, 0)
    {
        return Err(PortfolioConstructionError::InvalidConstraint(
            "maximum turnover weight must be between zero and two".to_owned(),
        ));
    }

    let mut instrument_ids = BTreeSet::new();
    for holding in holdings {
        if !instrument_ids.insert(holding.instrument.id) {
            return Err(PortfolioConstructionError::DuplicateInstrument(
                holding.instrument.symbol.clone(),
            ));
        }
        if holding.instrument.currency != target.total_equity.currency() {
            return Err(PortfolioConstructionError::InvalidConstraint(
                "holding and portfolio currencies differ".to_owned(),
            ));
        }
    }
    Ok(())
}

fn gross_notional(drafts: &[DraftTrade]) -> Result<Decimal, PortfolioConstructionError> {
    drafts.iter().try_fold(Decimal::ZERO, |total, draft| {
        let notional = checked_mul(
            draft.quantity,
            draft.reference_price.value(),
            "draft notional overflow",
        )?;
        checked_add(total, notional, "gross notional overflow")
    })
}

fn round_to_lot(quantity: Decimal, instrument: &Instrument) -> Decimal {
    if instrument.fractional_supported {
        quantity
    } else {
        (quantity / instrument.board_lot.value()).floor() * instrument.board_lot.value()
    }
}

const fn side_rank(side: Side) -> u8 {
    match side {
        Side::Sell => 0,
        Side::Buy => 1,
    }
}
