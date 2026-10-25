# HTTP API JSON

The shape of JSON on the wire for every tasker-systems HTTP API — temper's API and tasker-core's
orchestration and worker APIs — and the OpenAPI specs generated from them. Several SDKs per
repository are generated from those specs, so the spec is a contract in its own right. Enforcement
status per repo is in [`rules.md`](rules.md); the Google JSON style guide ledger is
[`../sources/google/docs-and-json.md`](../sources/google/docs-and-json.md).

Google's JSON guide predates OpenAPI and serves a different API culture (camelCase, a `data`
envelope, `apiVersion`/`kind` fields). temper and tasker-core already agree on a different,
consistent style; these rules write that style down.

**API-JSON-1 — snake_case names, snake_case enum values, SCREAMING_SNAKE error codes.** Property
names and enum values on the wire are snake_case (`#[serde(rename_all = "snake_case")]`); the
`code` of an error is SCREAMING_SNAKE. *Why:* every SDK is generated from these names, so a casing
inconsistency becomes an inconsistency in four languages. Both APIs are already snake_case for
properties; tasker-core mixes PascalCase enum values (`ManualDlq`, `MaxRetriesExceeded`) with
snake_case ones.

**API-JSON-2 — one error shape, and the spec describes it.** Every error response is
`{"error": {"code": "<SCREAMING_SNAKE>", "message": "...", "details": {...}?}}`. The HTTP status
carries the number; `code` is the stable string a client branches on. The OpenAPI spec's error
schema is the shape actually sent, proven by a test that serialises each error variant and validates
it against the spec. *Why:* a generated client's error path is built from the spec. tasker-core
sends the right shape but its spec derives `ToSchema` on the Rust error enum, so 72 operations
document an error as `{"NotFound": {"message": …}}` or the bare string `"Forbidden"` — a shape that
never appears on the wire.

**API-JSON-3 — timestamps are RFC 3339 in UTC, typed `date-time`.** A field ending `_at` is a
timestamp and nothing else, and the spec declares it `format: date-time`. *Why:* a timestamp typed
as a plain string reaches each generated SDK as a string, and every caller parses it by hand.
tasker-core types 9 of its 19 `*_at` fields as `date-time`; temper 24 of 25.

**API-JSON-4 — a duration is an integer with its unit in the name.** `retry_after_seconds`,
`timeout_ms` — not ISO 8601 duration strings. *Why:* the unit is visible at every use, and no
client needs a duration parser. Both APIs already do this.

**API-JSON-5 — no success envelope; a list says whether it is complete.** A success body is the
resource or the list itself, not `{"data": …}`. A list response carries enough to tell a page from
the whole set: a total, and a truncation or next-page signal. *Why:* the envelope adds nothing the
HTTP status does not already say. An unmarked page is the bug that temper's own guidance warns
agents about: a short page read as the whole set.

## Not specified

- **The pagination shape.** temper returns `limit, offset, total, returned, truncated`; tasker-core
  returns `page, per_page, total_count, total_pages, has_next, has_previous`. Both satisfy
  API-JSON-5. Unifying them is an API-breaking change and a separate ruling.
- **Null versus absent.** Whether an empty optional is omitted or sent as `null` is per field, and
  the spec must say which; no repo-wide rule yet.
- **Versioning, idempotency keys, `Retry-After`** — Google's guide is silent, and so is this file
  for now. [CONC-DB-2](concurrency.md) covers idempotency on the server side.
