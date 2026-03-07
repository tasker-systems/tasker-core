# Problem Statement: Security and Secrets in the Action Grammar Context

*Research spike — March 2026*

---

## Why This Comes Up Now

The action grammar research established a clear three-layer execution model: `tasker-grammar` (pure data transformation), `tasker-worker` (worker lifecycle bridge), and domain handlers (traditional callables). For the pure data capabilities — `transform`, `validate`, `assert` — the grammar crate can be implemented, tested, and validated independently, because they are stateless functions over JSON. No infrastructure. No credentials. No side effects.

The three remaining capabilities — `acquire`, `persist`, `emit` — are fundamentally different. They reach outside the process. They read from external data sources, write to external systems, and fire events. They are the reasons workflows exist. And they are exactly the capabilities Tasker avoided taking responsibility for until now, because domain handlers owned their own infrastructure.

With grammar compositions, **Tasker becomes the code** for these operations. The composition executor in `tasker-worker` is the thing that calls the database, calls the API, emits the event. The platform now handles data that it previously never saw in cleartext. The choices made here — about how credentials are injected, how PII is protected, how data at rest is handled — are architectural choices, not implementation details. They cannot be deferred to post-implementation cleanup.

The trigger: we cannot write good stub implementations for the `acquire`, `persist`, and `emit` executors in Phase 1C without knowing the `ResourceHandle` interface. The interface depends on the design decisions in this spike.

---

## What Changed When Domain Handlers Were the Implementation

With traditional domain handlers, a boundary existed:

```
Tasker: orchestration framework
Handler: everything else
```

The handler owned its database pool. The handler owned its API client. The handler managed its credentials (typically via environment variables or a secrets manager the handler's runtime accessed). The handler decided what to encrypt and what to expose. Tasker's responsibility was ensuring the handler was invoked with the right inputs at the right time and that the result was recorded.

This was an intentional boundary. It served two purposes: (1) it kept Tasker's scope manageable, and (2) it let organizations bring their existing infrastructure patterns without Tasker imposing opinions on them.

That boundary still exists for domain handlers. Nothing in the grammar work changes it. A Ruby handler that talks to Stripe still manages its own Stripe credentials. A Python handler that writes to BigQuery still manages its own service account keys. The boundary holds everywhere traditional handlers are used.

---

## What Changes With Grammar Compositions

When a `persist` capability step writes to a database, the composition executor — code in `tasker-worker` — is the code doing the write. It needs a database connection. That connection needs credentials. Where do those credentials come from?

The naive answer is "pass them in the composition config." This is wrong for reasons that are immediately apparent: the composition config lives in the TaskTemplate YAML, gets stored in `named_tasks.configuration`, flows through the MCP server, and appears in `tasker-ctl composition validate` output. Credentials in config are credentials in plaintext in the database.

The next-naive answer is "use environment variables." This is how `DATABASE_URL` works today for Tasker's own database. Env vars are usable and Tasker currently relies on them. But env vars have known problems:
- They are process-wide; any code running in the process can read them
- An injudicious log or trace span can spill them
- They don't support rotation without process restart
- They don't support scoping (all database connections use the same URL)
- A jaq filter can't accidentally emit an env var value, but errors and panics can surface them in logs

The correct answer requires designing a `ResourceRegistry` — a configured set of named resources where secret values are resolved at startup from a `SecretsProvider` and never placed in the data paths (jaq contexts, step results, trace spans, composition configs).

This is not a new idea. It is the standard pattern in cloud infrastructure: connection strings are configuration, credentials are secrets, and the two are resolved separately at runtime. The design work here is Tasker's instantiation of that pattern.

---

## The Three Distinct Concerns

The problem decomposes into three separable concerns with different threat models and different right answers. Conflating them produces a blob with unclear boundaries. Keeping them separate produces a coherent design.

### Concern 1: Credential Injection for Action Capabilities

**Question**: How do `acquire`, `persist`, and `emit` capability executors access external systems without credentials being in composition configs, jaq contexts, or step results?

**Threat model**: Credentials in data paths (accidentally logged, stored in step results, exposed through MCP tools, visible in `tasker-ctl` output, or accessible to jaq filter expressions).

**The non-negotiable constraint**: The composition context envelope (`.context`, `.deps`, `.prev`, `.step`) is data. It flows through jaq filters. It is logged on errors. It is stored as step results. Credentials must never be in it.

**The right answer**: A `ResourceRegistry` initialized at worker startup. Action capability executors receive `Arc<dyn ResourceHandle>` — already-initialized connection pools or authenticated clients — not credential values. The `ResourceRegistry` resolves secrets through a `SecretsProvider` strategy. The capability executor never sees the secret.

**See**: `01-secrets-and-credential-injection.md`, `02-resource-registry.md`

### Concern 2: Trace and Log Safety for Sensitive Data

**Question**: How do we prevent PII and sensitive data values from leaking into traces, structured logs, and error messages, given that step contexts legitimately contain sensitive data?

**Threat model**: PII in observability infrastructure (traces exported to Jaeger/Honeycomb/Datadog, structured logs shipped to Elasticsearch, error messages with sensitive field values).

**The constraint that distinguishes this from Concern 1**: This is about data the system legitimately processes, not credentials. A payment workflow's `.context` legitimately contains a credit card number. We can't exclude it — it's the payload being processed. We need to handle it safely.

**The right answer**: Field-level classification in the TaskTemplate (`data_classification:` block), enforced by a `DataClassifier` that operates at the observability emission layer (trace spans, log records). Data flows normally through the composition; redaction happens only when data would leave the process boundary into observability systems.

**See**: `03-trace-and-log-safety.md`

### Concern 3: Encryption at Rest for Task Data

**Question**: Should step results, task context, and checkpoint data stored in PostgreSQL be encrypted, and if so, at what granularity?

**Threat model**: Database backup exfiltration, compromised read access to PostgreSQL, long-lived sensitive data in step results.

**The constraint**: This must be transparent to the composition engine. A jaq filter reads `.context.credit_card_number` expecting a string. If that field is stored encrypted, it must be decrypted before the composition context is assembled. The encrypt/decrypt cycle lives at the sqlx access layer, not inside `tasker-grammar`.

**The right answer**: An `EncryptionProvider` strategy with field-level encryption specified in the TaskTemplate, applied at storage write/read time. Envelope encryption (separate DEK per record, KEK managed by a KMS) is the standard approach that enables key rotation without re-encrypting all data.

**See**: `04-encryption-at-rest.md`

---

## What Tasker Is Not

The strategy-pattern design is explicit about what Tasker is not:

**Not a vault.** Tasker does not store secrets. It resolves secret references through external providers. A `SecretsProvider` implementation talks to Vault, AWS SSM, or a SOPS-encrypted file. Tasker does not become the secret store.

**Not a KMS.** Tasker does not manage encryption keys. An `EncryptionProvider` implementation delegates key management to AWS KMS, GCP KMS, or Vault Transit. Tasker does not generate or store KEKs.

**Not a DLP system.** The `DataClassifier` is not a data loss prevention scanner. It enforces field-level classification specified by the template author. It does not discover PII by scanning data content. Template authors are responsible for correctly classifying their data.

**Not prescriptive for domain handler authors.** These mechanisms apply to grammar-composed virtual handlers. Domain handler authors continue to manage their own credentials, encryption, and data protection as they always have.

---

## The Dog-Fooding Argument

Tasker currently uses environment variables for its own infrastructure credentials: `DATABASE_URL`, `PGMQ_DATABASE_URL`, `REDIS_URL`, `RABBITMQ_URL`, `TASKER_JWT_PUBLIC_KEY`. These are visible in the annotated config reference:

```toml
url = "${DATABASE_URL:-postgresql://localhost/tasker}"
url = "${PGMQ_DATABASE_URL:-}"
url = "${REDIS_URL:-redis://localhost:6379}"
url = "${RABBITMQ_URL:-amqp://guest:guest@localhost:5672/%2F}"
jwt_public_key = "${TASKER_JWT_PUBLIC_KEY:-}"
```

This is fine for development. It is not sufficient for production deployments at organizations that have real secrets management requirements. The same spike that designs `tasker-secure` for grammar compositions should also design how Tasker's own configuration layer adopts the `SecretsProvider` strategy.

The specific outcome: the config values currently supporting `${VAR:-default}` shell substitution should also support `{secret_ref: "path/to/secret"}` resolution through a configured `SecretsProvider`. Env vars remain a valid backend (the `EnvSecretsProvider` is a first-class implementation), but they are no longer the only strategy.

This has a concrete benefit: teams using SOPS-encrypted config files, Vault, or AWS SSM for their organization's secrets management can deploy Tasker into that environment without wrapper scripts that materialize env vars.

The dog-fooding constraint is intentional design pressure: if the `SecretsProvider` strategy is good enough for Tasker's own `DATABASE_URL`, it is good enough for `acquire`/`persist`/`emit` resource credentials.

---

## Relationship to the Action Grammar Implementation Phases

In `docs/action-grammar/implementation-phases.md`, the "Open Design: Infrastructure Injection for Action Capabilities" section in `transform-revised-grammar.md` deferred this work to "a substantial body of work that should be scoped as its own research spike after the core grammar primitives (Phase 1) are validated."

That deferral needs to be qualified. The Phase 1A (expression engine) and Phase 1B (core type definitions) work can proceed without this spike. The Phase 1C capability executor implementations (`acquire`, `persist`, `emit`) cannot be written — even as stubs — without knowing the `ResourceHandle` interface. The interface must be stable before Phase 1C begins.

The practical implication: this spike (specifically Spikes S1 and S2 described in `06-research-spikes.md`) must be complete before Phase 1C begins. Spikes S3 (encryption) and S4 (trace safety) can be developed in parallel with Phase 1C, because those concerns affect the worker integration layer (Phase 3) rather than the grammar executor stubs.

---

*This document is the framing entry point for the security and secrets research directory. Read each numbered document for the detailed treatment of each concern.*
