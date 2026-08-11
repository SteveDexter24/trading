# ADR 0003: Point-in-time data and Webull boundary

Status: Accepted

## Context

Historical decisions are invalid if they see later revisions, future news, or
unversioned corporate-action adjustments. Webull transport contracts and
availability also vary by product and region.

## Decision

Market and research records store `observed_at`, `effective_at`, `source`, and
`ingested_at`. Exchange trading date remains separate from UTC timestamps.
Fundamentals retain revision numbers; corporate actions retain explicit
versions. Backtest queries must filter `observed_at <= decision_time` and pin a
corporate-action version.

The Webull anti-corruption layer separates HTTP, MQTT, gRPC, credentials, and
signing from domain types. As checked on 2026-08-11, the current official
documentation describes:

- HTTPS request authentication and HMAC-SHA1 signing headers;
- HTTP trading and market-data APIs;
- MQTT market streams;
- gRPC server-streaming trade events;
- sandbox and production host families.

The official overview currently describes the public trading API as a US-market
offering and says additional global markets are planned. Therefore this release
does not guess HK request signing details, endpoints, order payloads, fractional
capability, board-lot metadata, or fees. The HK adapter constructor fails closed
until an official HK contract is available and reviewed.

Sources:

- <https://developer.webull.com/apis/docs/>
- <https://developer.webull.com/apis/docs/authentication/signature>
- <https://developer.webull.com/apis/docs/reference/custom/subscribe-trade-events>
- <https://developer.webull.com/apis/docs/sdk>

## Consequences

The schema can support bias-controlled research and explicit data revisions.
The engine can paper-test HK rules with synthetic/reference data, but cannot
send Webull HK orders.
