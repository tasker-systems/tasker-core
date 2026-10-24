# Security

Code-level security rules common to every tasker-systems service. Supply-chain rules are in
[`supply-chain.md`](supply-chain.md).

## Credentials

**SEC-CRED-1 — a credential is a secret type.** Tokens, keys, passwords and connection strings
that embed a password are held in a secret-wrapping type (`secrecy::SecretString`, or a wrapper
such as tasker-secure's `SecretValue`, which also zeroizes on drop), not a bare `String`. *Why:* a
bare string can be logged, formatted or serialised by any code that touches it.

**SEC-CRED-2 — a credential never prints, and that is proven.** Any type that carries a credential
either does not derive `Debug` or implements it by hand, redacting the value while showing whether
it is present (`Some("[redacted]")`). A tripwire pins the set of credential-bearing types that
derive `Debug`, so a new one must be reviewed. *Why:* `#[derive(Debug)]` on a config struct is the
most common way a secret reaches a log, and code review alone does not catch it reliably.

**SEC-CRED-3 — credentials are compared in constant time.** API keys, shared secrets, HMAC
signatures and similar values are compared with `subtle::ConstantTimeEq` or an equivalent. *Why:*
an early-exit comparison leaks how much of a guess was right.

**SEC-CRED-4 — one secret per purpose.** Two signature or authentication gates never share a
secret. *Why:* a leak in one gate should not open the other.

## Redaction

**SEC-RED-1 — redaction has one home.** Each repository has a single redaction module — URL
credentials, request paths, header values — used by logging and tracing. *Why:* copies of a
redaction helper drift, and the one that drifts is the one that leaks.

**SEC-RED-2 — redact by where the value came from, not by what it looks like.** Decide redaction
from the route, field or source, and default to redacting. *Why:* a pattern that recognises secrets
by shape misses the ones that do not look like secrets.

## SQL

**SEC-SQL-1 — values are bound, identifiers are validated.** Every value reaches the database as
a bound parameter. An identifier that cannot be bound (a `NOTIFY` channel, a queue or table name, an
`ORDER BY` column) passes through a named validator that admits a fixed pattern. *Why:* string-built
SQL is the injection surface; tasker-core's NOTIFY-channel validators came out of exactly such a
finding.

**SEC-SQL-2 — authorisation is in the query.** A read is scoped to what the caller may see inside the
query itself (or a view or function it calls), not by filtering results afterwards. *Why:* a filter
applied after the fact is a filter some code path will forget.

## Authentication posture

**SEC-AUTH-1 — a disabled-auth mode is loud and cannot ship.** If a service can run with
authentication off, it logs a prominent warning at startup in that mode, and a check prevents a
production configuration from shipping with it off. *Why:* "off by default for convenience" becomes
"off in production by accident".

**SEC-AUTH-2 — authorise before reading the body.** Permission checks run at the route, before the
request body is deserialised or any write begins.

**SEC-AUTH-3 — a refusal does not reveal existence.** Where a caller lacks access to a thing, the
response is the same as for a thing that does not exist. *Why:* a distinct "forbidden" answer is an
oracle for what exists.

## Test doors

**SEC-TEST-1 — test-only features never reach a release.** Features that open test-only behaviour
(test principals, fixture loaders, bypasses) are checked absent from every shipped binary's feature
set.

## Reporting

**SEC-RPT-1 — a SECURITY.md with private reporting.** It names the private channel (GitHub
security advisories, plus an email address), response times, and an explicit out-of-scope list.
temper's `SECURITY.md` is the template.
