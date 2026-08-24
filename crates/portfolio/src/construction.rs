use crate::model::{
    checked_add, checked_mul, validate_unit_interval, AlphaEstimate, ConstructionMethod,
    ConstructionRequest, PortfolioConstructionError, PortfolioPlan, TargetAllocation,
};
use rust_decimal::Decimal;
use std::collections::{BTreeMap, BTreeSet};
use trading_domain::Money;

const ALLOCATION_EPSILON: Decimal = Decimal::from_parts(1, 0, 0, false, 18);

#[derive(Debug)]
struct WorkingAllocation {
    estimate: AlphaEstimate,
    score: Decimal,
    cap: Decimal,
    weight: Decimal,
}

pub fn construct_portfolio(
    request: ConstructionRequest,
) -> Result<PortfolioPlan, PortfolioConstructionError> {
    validate_request(&request)?;

    let currency = request.total_equity.currency();
    let group_caps = request
        .constraints
        .group_constraints
        .iter()
        .map(|constraint| (constraint.group.clone(), constraint.maximum_weight))
        .collect::<BTreeMap<_, _>>();

    let mut working = request
        .estimates
        .into_iter()
        .map(|estimate| {
            let score = construction_score(&estimate, request.method).transpose()?;
            Ok(score
                .filter(|value| *value > Decimal::ZERO)
                .map(|score| WorkingAllocation {
                    cap: estimate
                        .maximum_weight
                        .min(request.constraints.maximum_position_weight),
                    estimate,
                    score,
                    weight: Decimal::ZERO,
                }))
        })
        .collect::<Result<Vec<_>, PortfolioConstructionError>>()?
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();
    working.sort_by_key(|allocation| allocation.estimate.instrument.id);

    allocate_weights(
        &mut working,
        request.constraints.target_invested_weight,
        &group_caps,
    )?;

    let mut allocated_weight = Decimal::ZERO;
    let mut allocations = Vec::new();
    for allocation in working
        .into_iter()
        .filter(|allocation| allocation.weight > ALLOCATION_EPSILON)
    {
        allocated_weight = checked_add(
            allocated_weight,
            allocation.weight,
            "allocated weight overflow",
        )?;
        let target_amount = checked_mul(
            request.total_equity.amount(),
            allocation.weight,
            "target value overflow",
        )?;
        allocations.push(TargetAllocation {
            instrument: allocation.estimate.instrument,
            reference_price: allocation.estimate.reference_price,
            target_weight: allocation.weight,
            target_value: Money::new(target_amount, currency)?,
            construction_score: allocation.score,
            source_version: allocation.estimate.source_version,
        });
    }

    let allocated_amount = checked_mul(
        request.total_equity.amount(),
        allocated_weight,
        "allocated value overflow",
    )?;
    let unallocated_cash = request
        .total_equity
        .checked_sub(Money::new(allocated_amount, currency)?)?;

    Ok(PortfolioPlan {
        as_of: request.as_of,
        method: request.method,
        total_equity: request.total_equity,
        allocated_weight,
        unallocated_cash,
        allocations,
    })
}

fn validate_request(request: &ConstructionRequest) -> Result<(), PortfolioConstructionError> {
    if request.total_equity.amount() <= Decimal::ZERO {
        return Err(PortfolioConstructionError::InvalidConstraint(
            "total equity must be positive".to_owned(),
        ));
    }
    validate_unit_interval(
        request.constraints.target_invested_weight,
        "target invested weight",
    )?;
    validate_unit_interval(
        request.constraints.maximum_position_weight,
        "maximum position weight",
    )?;
    if request.constraints.maximum_position_weight <= Decimal::ZERO {
        return Err(PortfolioConstructionError::InvalidConstraint(
            "maximum position weight must be positive".to_owned(),
        ));
    }

    let mut group_names = BTreeSet::new();
    for constraint in &request.constraints.group_constraints {
        if constraint.group.trim().is_empty() || !group_names.insert(constraint.group.clone()) {
            return Err(PortfolioConstructionError::InvalidConstraint(
                "risk groups must have unique non-empty names".to_owned(),
            ));
        }
        validate_unit_interval(constraint.maximum_weight, "group maximum weight")?;
    }

    let mut instruments = BTreeSet::new();
    let mut horizon = None;
    for estimate in &request.estimates {
        if !instruments.insert(estimate.instrument.id) {
            return Err(PortfolioConstructionError::DuplicateInstrument(
                estimate.instrument.symbol.clone(),
            ));
        }
        if estimate.instrument.currency != request.total_equity.currency() {
            return Err(PortfolioConstructionError::InvalidEstimate {
                symbol: estimate.instrument.symbol.clone(),
                reason: "instrument and portfolio currencies differ".to_owned(),
            });
        }
        if estimate.forecast_volatility <= Decimal::ZERO {
            return Err(PortfolioConstructionError::InvalidEstimate {
                symbol: estimate.instrument.symbol.clone(),
                reason: "forecast volatility must be positive".to_owned(),
            });
        }
        validate_estimate_interval(estimate, estimate.confidence, "confidence")?;
        validate_estimate_interval(estimate, estimate.liquidity_score, "liquidity score")?;
        validate_estimate_interval(estimate, estimate.maximum_weight, "maximum weight")?;
        if estimate.maximum_weight <= Decimal::ZERO {
            return Err(PortfolioConstructionError::InvalidEstimate {
                symbol: estimate.instrument.symbol.clone(),
                reason: "maximum weight must be positive".to_owned(),
            });
        }
        if estimate.observed_at > request.as_of {
            return Err(PortfolioConstructionError::InvalidEstimate {
                symbol: estimate.instrument.symbol.clone(),
                reason: "estimate was not observable at construction time".to_owned(),
            });
        }
        if estimate.forecast_horizon_seconds == 0 {
            return Err(PortfolioConstructionError::InvalidEstimate {
                symbol: estimate.instrument.symbol.clone(),
                reason: "forecast horizon must be positive".to_owned(),
            });
        }
        if let Some(expected_horizon) = horizon {
            if estimate.forecast_horizon_seconds != expected_horizon {
                return Err(PortfolioConstructionError::InvalidEstimate {
                    symbol: estimate.instrument.symbol.clone(),
                    reason: "all estimates must use the same forecast horizon".to_owned(),
                });
            }
        } else {
            horizon = Some(estimate.forecast_horizon_seconds);
        }
        if estimate.source_version.trim().is_empty() {
            return Err(PortfolioConstructionError::InvalidEstimate {
                symbol: estimate.instrument.symbol.clone(),
                reason: "source version must not be empty".to_owned(),
            });
        }
    }
    Ok(())
}

fn validate_estimate_interval(
    estimate: &AlphaEstimate,
    value: Decimal,
    name: &str,
) -> Result<(), PortfolioConstructionError> {
    if value < Decimal::ZERO || value > Decimal::ONE {
        return Err(PortfolioConstructionError::InvalidEstimate {
            symbol: estimate.instrument.symbol.clone(),
            reason: format!("{name} must be between zero and one"),
        });
    }
    Ok(())
}

fn construction_score(
    estimate: &AlphaEstimate,
    method: ConstructionMethod,
) -> Option<Result<Decimal, PortfolioConstructionError>> {
    match method {
        ConstructionMethod::EqualWeight => Some(Ok(Decimal::ONE)),
        ConstructionMethod::InverseVolatility => {
            let quality = checked_mul(
                estimate.confidence,
                estimate.liquidity_score,
                "quality overflow",
            );
            Some(quality.map(|quality| quality / estimate.forecast_volatility))
        }
        ConstructionMethod::RiskAdjustedAlpha => {
            if estimate.expected_excess_return <= Decimal::ZERO {
                return None;
            }
            let score = checked_mul(
                estimate.expected_excess_return,
                estimate.confidence,
                "alpha confidence overflow",
            )
            .and_then(|value| {
                checked_mul(value, estimate.liquidity_score, "alpha liquidity overflow")
            })
            .map(|value| value / estimate.forecast_volatility);
            Some(score)
        }
    }
}

fn allocate_weights(
    allocations: &mut [WorkingAllocation],
    target_weight: Decimal,
    group_caps: &BTreeMap<String, Decimal>,
) -> Result<(), PortfolioConstructionError> {
    let iteration_limit = allocations.len().saturating_mul(4).saturating_add(4);
    for _ in 0..iteration_limit {
        let allocated = allocations.iter().try_fold(Decimal::ZERO, |total, item| {
            checked_add(total, item.weight, "weight sum overflow")
        })?;
        let remaining = target_weight - allocated;
        if remaining <= ALLOCATION_EPSILON {
            break;
        }

        let group_weights = group_weights(allocations)?;
        let active = allocations
            .iter()
            .enumerate()
            .filter(|(_, item)| {
                item.score > Decimal::ZERO
                    && item.weight + ALLOCATION_EPSILON < item.cap
                    && group_has_capacity(item, &group_weights, group_caps)
            })
            .map(|(index, _)| index)
            .collect::<Vec<_>>();
        if active.is_empty() {
            break;
        }
        let score_total = active.iter().try_fold(Decimal::ZERO, |total, index| {
            checked_add(total, allocations[*index].score, "score sum overflow")
        })?;
        if score_total <= Decimal::ZERO {
            break;
        }

        let mut increments = BTreeMap::new();
        for index in &active {
            let item = &allocations[*index];
            let proportional =
                checked_mul(remaining, item.score, "allocation overflow")? / score_total;
            increments.insert(*index, proportional.min(item.cap - item.weight));
        }
        apply_group_caps(allocations, &mut increments, &group_weights, group_caps)?;

        let mut progress = Decimal::ZERO;
        for (index, increment) in increments {
            if increment <= Decimal::ZERO {
                continue;
            }
            allocations[index].weight = checked_add(
                allocations[index].weight,
                increment,
                "position weight overflow",
            )?;
            progress = checked_add(progress, increment, "allocation progress overflow")?;
        }
        if progress <= ALLOCATION_EPSILON {
            break;
        }
    }
    Ok(())
}

fn group_weights(
    allocations: &[WorkingAllocation],
) -> Result<BTreeMap<String, Decimal>, PortfolioConstructionError> {
    let mut result = BTreeMap::new();
    for item in allocations {
        if let Some(group) = &item.estimate.risk_group {
            let current = result.get(group).copied().unwrap_or(Decimal::ZERO);
            result.insert(
                group.clone(),
                checked_add(current, item.weight, "group weight overflow")?,
            );
        }
    }
    Ok(result)
}

fn group_has_capacity(
    item: &WorkingAllocation,
    group_weights: &BTreeMap<String, Decimal>,
    group_caps: &BTreeMap<String, Decimal>,
) -> bool {
    item.estimate.risk_group.as_ref().map_or(true, |group| {
        group_caps.get(group).is_none_or(|cap| {
            group_weights.get(group).copied().unwrap_or(Decimal::ZERO) + ALLOCATION_EPSILON < *cap
        })
    })
}

fn apply_group_caps(
    allocations: &[WorkingAllocation],
    increments: &mut BTreeMap<usize, Decimal>,
    group_weights: &BTreeMap<String, Decimal>,
    group_caps: &BTreeMap<String, Decimal>,
) -> Result<(), PortfolioConstructionError> {
    for (group, cap) in group_caps {
        let current = group_weights.get(group).copied().unwrap_or(Decimal::ZERO);
        let capacity = (*cap - current).max(Decimal::ZERO);
        let group_indices = increments
            .keys()
            .copied()
            .filter(|index| allocations[*index].estimate.risk_group.as_ref() == Some(group))
            .collect::<Vec<_>>();
        let proposed = group_indices
            .iter()
            .try_fold(Decimal::ZERO, |total, index| {
                checked_add(
                    total,
                    increments.get(index).copied().unwrap_or(Decimal::ZERO),
                    "group proposal overflow",
                )
            })?;
        if proposed <= capacity || proposed <= Decimal::ZERO {
            continue;
        }
        let scale = capacity / proposed;
        for index in group_indices {
            if let Some(increment) = increments.get_mut(&index) {
                *increment = checked_mul(*increment, scale, "group cap scaling overflow")?;
            }
        }
    }
    Ok(())
}
