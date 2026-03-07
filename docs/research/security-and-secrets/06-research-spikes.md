# Research Spikes: Phased Plan and Acceptance Criteria

*What must be built before the action grammar capability executors can be implemented*

*Research spike — March 2026*

---

## Relationship to the Grammar Implementation Phases

From `docs/action-grammar/implementation-phases.md`:

- **Phase 1A** (jaq-core integration): Can proceed immediately. No dependency on this work.
- **Phase 1B** (core type definitions): Can proceed immediately. `CapabilityDeclaration` and `CompositionSpec` have no dependency on secrets or resources.
- **Phase 1C** (capability executor implementations — `acquire`, `persist`, `emit`): **Blocked on Spikes S1 and S2**. These executors need the `ResourceHandle` interface and `ResourceRegistry` to write even their stub implementations for grammar testing.
- **Phase 1D** (composition engine): Can proceed immediately.
- **Phase 1E** (workflow modeling): Can proceed immediately.
- **Phase 1F** (integration tests): Requires Phase 1C; blocked by S1 and S2.

Spikes S3 and S4 (encryption and trace safety) can proceed in parallel with Phase 1C once S1 and S2 are complete, because they affect the worker integration layer (Phase 3) rather than the grammar executor stubs.

**The critical path**:

```
S1 (SecretsProvider + SecretValue)
  │
  ↓
S2 (ResourceRegistry + ResourceHandle stubs + dog-fooding)
  │
  ↓
Phase 1C (acquire, persist, emit executors using stub ResourceHandle)
```

S3 and S4 are parallel work that must complete before Phase 3 (worker integration) begins.

---

## Spike S1: SecretsProvider Foundation

*Goal: Establish the secrets resolution layer that everything else builds on.*

### What to Deliver

1. **`tasker-secure` crate scaffolding**
   - Workspace member at `tasker-secure/`
   - `Cargo.toml` with feature gates as specified in `05-tasker-secure-crate-proposal.md`
   - `src/lib.rs` with module structure
   - No external dependencies yet (add them feature by feature)

2. **`SecretValue` type**
   - Wraps `secrecy::SecretString`
   - `Display` and `Debug` emit `"[REDACTED]"`
   - `expose_secret()` method for intentional access
   - `From<String>` and `From<secrecy::SecretString>` conversions
   - Unit tests: verify Display/Debug redaction, verify zeroize-on-drop (check via secrecy's test utilities)

3. **`SecretsProvider` trait**
   - As specified in `01-secrets-and-credential-injection.md`
   - `SecretsError` error type with variants for `NotFound`, `AccessDenied`, `ProviderUnavailable`, `InvalidPath`
   - Trait is object-safe (no generic methods)

4. **`EnvSecretsProvider` implementation**
   - Reads environment variables
   - Normalizes path to env var name (strip prefix, uppercase, replace `/` and `-` with `_`)
   - `health_check()` always returns `Ok` (env is always available)
   - Unit tests: resolution with/without prefix, missing vars, normalization rules

5. **`ChainedSecretsProvider` implementation**
   - Tries providers in order
   - Returns first success; returns last error if all fail
   - Unit tests: chain with two providers, first succeeds; chain where first fails and second succeeds; chain where both fail

6. **Dog-fooding: Config layer `ConfigString` type**
   - `ConfigString` enum: `Literal`, `SecretRef`, `EnvRef`
   - `resolve()` method async, delegates to `SecretsProvider`
   - Backward-compatible: existing `${VAR:-default}` syntax continues to work
   - New `{secret_ref = "path"}` TOML syntax supported
   - Update annotated config documentation to show both syntaxes for the five sensitive config values (`DATABASE_URL`, `PGMQ_DATABASE_URL`, `REDIS_URL`, `RABBITMQ_URL`, `TASKER_JWT_PUBLIC_KEY`)
   - Unit tests: all three variants, chained resolution

7. **Optional: `SopsSecretsProvider` (if rops integration is straightforward)**
   - Feature-gated `sops`
   - Load and decrypt a SOPS-encrypted YAML file at startup using `rops`
   - Cache decrypted values in memory as `SecretValue`
   - Path format: dot-separated key navigation into the decrypted structure
   - Integration test: create a SOPS-encrypted YAML with age key, verify values are resolved correctly

### Acceptance Criteria

- `cargo test -p tasker-secure` passes with no real secrets backend required
- `cargo test -p tasker-secure --features sops` passes with a test SOPS file and age key in the test environment
- `EnvSecretsProvider` resolves existing `DATABASE_URL` correctly when set in the test environment
- `ConfigString::resolve()` correctly handles `Literal`, `SecretRef` (via `EnvSecretsProvider`), and `EnvRef` variants
- `SecretValue`'s Display/Debug never expose the secret value in any test output
- The five sensitive config values in the annotated config reference show both `"${VAR:-default}"` and `{secret_ref = "..."}` syntax examples

### What NOT to Do in S1

- Do not implement Vault or AWS providers yet — those require real infrastructure to test meaningfully
- Do not implement `ResourceRegistry` yet — that is S2
- Do not integrate `ConfigString` into the actual `tasker-shared` config loading path yet — validate the type design in isolation first

---

## Spike S2: ResourceRegistry and ResourceHandle

*Goal: Establish the resource layer that unblocks the grammar capability executor stubs.*

### What to Deliver

1. **`ResourceHandle` trait**
   - As specified in `02-resource-registry.md`
   - `resource_name()`, `resource_type()`, `refresh_credentials()`, `health_check()`, `as_any()`
   - `ResourceHandleExt` convenience trait with typed downcast methods
   - `ResourceError` error type

2. **`ResourceDefinition` and `ConfigValue`**
   - TOML-deserializable
   - `ConfigValue::resolve(&dyn SecretsProvider) -> Result<SecretValue, SecretsError>` for secret refs
   - `ConfigValue::resolve_literal(&dyn SecretsProvider) -> Result<String, SecretsError>` for literals and env refs

3. **`ResourceRegistry`**
   - `initialize_all(secrets, definitions) -> Result<Self, ResourceError>` (startup path)
   - `get(name) -> Option<Arc<dyn ResourceHandle>>`
   - `refresh_resource(name) -> Result<(), ResourceError>`
   - `list_resources() -> Vec<ResourceSummary>` (no credentials in summary)
   - Startup health check modes: `ping` (default) and `query`
   - Unit tests with `InMemoryResourceHandle`

4. **`InMemoryResourceHandle` (test-utils feature)**
   - Fixture data for `acquire` responses (keyed by resource name + query params)
   - Captured operations for `persist` assertions (what was written)
   - Captured operations for `emit` assertions (what was emitted)
   - `test_registry_with_fixtures(fixtures: Vec<ResourceFixture>) -> ResourceRegistry` factory

5. **`PostgresHandle` (postgres feature)**
   - Wraps `sqlx::PgPool`
   - Pool configuration from `ResourceDefinition` config
   - `pool()` method returning `&PgPool`
   - `refresh_credentials()` reinitializes the pool with freshly resolved credentials
   - Integration test: connect to a test PostgreSQL instance (feature-gated `test-services`)

6. **`HttpHandle` (http feature)**
   - Wraps `reqwest::Client` with pre-configured auth
   - `get()`, `post()`, `put()`, `delete()` methods returning authenticated `RequestBuilder`
   - `HttpAuthStrategy` trait: `apply()` and `refresh()`
   - `ApiKeyAuthStrategy` built-in implementation (header-based)
   - `BearerTokenAuthStrategy` built-in implementation (with refresh via SecretsProvider)
   - Integration test: call a local HTTP endpoint with API key auth

7. **Dog-fooding: Tasker's own config uses `ResourceRegistry` pattern**
   - The `common.database` section's `url` value supports `{secret_ref = "..."}` via `ConfigString`
   - The `common.pgmq_database.url`, `common.cache.redis.url`, `common.queues.rabbitmq.url` likewise
   - Validation: existing dev deployments using `${DATABASE_URL:-...}` continue to work unchanged
   - Update `docs/generated/annotated-production.toml` to document both forms

8. **`ExecutionContext` extension in `tasker-worker`**
   - Add `resources: Arc<ResourceRegistry>` and `classifier: Option<Arc<DataClassifier>>` fields
   - Update `CompositionExecutor` (stub) to populate these fields from worker-level singletons
   - **This is the specific integration that unblocks Phase 1C**

### Acceptance Criteria

- `cargo test -p tasker-secure --features test-utils` passes entirely in-memory
- `cargo test -p tasker-secure --features postgres,test-services` passes against a local PostgreSQL instance
- `cargo test -p tasker-secure --features http` passes against a local mock HTTP server
- Phase 1C capability executor stubs can be written with `InMemoryResourceHandle` and compile correctly
- The `acquire` executor stub: receives `ExecutionContext`, gets a resource handle by name, returns fixture data
- The `persist` executor stub: receives `ExecutionContext`, gets a resource handle by name, records the write to the capture list
- The `emit` executor stub: receives `ExecutionContext`, gets a resource handle by name, records the event to the capture list
- Existing worker tests pass with the updated `ExecutionContext` (backward-compatible because `resources` is always populated, `classifier` is `None` for tests without classification specs)

### Open Design Decision to Resolve in S2

**Credential rotation protocol**: What triggers `refresh_resource()` and how does it interact with the step retry lifecycle?

The simplest model: if a capability executor receives an auth error from the resource, it calls `context.resources.refresh_resource(name)` and returns a retriable error. The step retry picks it up, and on the next attempt the pool has fresh credentials. The `refresh_resource()` call is synchronous from the executor's perspective (the executor awaits it), but it only needs to complete fast enough to not consume the step's full execution timeout.

The question for S2: does `ResourceHandle::refresh_credentials()` need to be called from inside the capability executor (executor-driven refresh), or should it be called from the `ResourceRegistry` proactively when it detects pool exhaustion or health check failures? The executor-driven approach is simpler and sufficient for the initial implementation.

---

## Spike S3: Encryption at Rest

*Goal: Establish the field-level encryption layer for sensitive data stored in PostgreSQL.*

*This spike can proceed in parallel with Phase 1C after S1 and S2 are complete.*

### What to Deliver

1. **`EncryptionProvider` trait and `EncryptedValue` type**
   - As specified in `04-encryption-at-rest.md`
   - `EncryptionError` error type

2. **`AesGcmEncryptionProvider` (encryption feature, for dev/test)**
   - Symmetric key held in memory (loaded from config via `SecretValue`)
   - `encrypt()` and `decrypt()` for raw bytes
   - `encrypt_fields()` and `decrypt_fields()` for JSON field-level operations
   - Graceful degradation: if a field value doesn't look like an `EncryptedValue` during decrypt, return as-is (schema migration support)
   - Unit tests: round-trip encrypt/decrypt for various value types (strings, numbers, nested objects, arrays)

3. **`AwsKmsEncryptionProvider` (aws-kms feature)**
   - Envelope encryption with KMS-managed KEK
   - DEK cache: configurable size and TTL
   - Integration test: against LocalStack KMS (Docker-based)
   - KMS mock for unit tests (without LocalStack)

4. **`FieldEncryptionSpec` and integration with `DataClassificationSpec`**
   - `encrypt_at_rest: bool` field added to `ClassificationRule`
   - `FieldEncryptionSpec` derived from `ClassificationRule` where `encrypt_at_rest = true`

5. **Storage layer integration in `tasker-shared` (or `tasker-worker`)**
   - `TaskStorageService.store_task_context()` — encrypt before INSERT
   - `TaskStorageService.load_task_context()` — decrypt after SELECT
   - `StepResultStorageService.store_result()` — encrypt before UPDATE
   - `StepResultStorageService.load_result()` — decrypt after SELECT
   - `CheckpointService.save_checkpoint()` — encrypt before write if classification spec requests it
   - `CheckpointService.load_checkpoint()` — decrypt after read
   - Unit tests using `AesGcmEncryptionProvider` with test key

### Acceptance Criteria

- `cargo test -p tasker-secure --features encryption` passes entirely in-memory
- `cargo test -p tasker-secure --features aws-kms` passes against LocalStack (feature-gated `test-aws`)
- A task with `encrypt_at_rest: true` on a context field stores an `EncryptedValue` JSON structure in `tasks.context`
- The same task's composition context envelope has the decrypted value (composition executor sees plaintext)
- The graceful degradation path: an unencrypted value in a field now marked `encrypt_at_rest: true` is returned as-is (no decryption failure)
- A step result with an encrypted field, stored and retrieved via `StepResultStorageService`, round-trips correctly

### Open Design Decisions to Resolve in S3

1. **Where does the `EncryptionProvider` live?** Both `tasker-worker` and `tasker-orchestration` need it (workers encrypt step results; orchestration encrypts task context at creation). Options: (a) `tasker-shared` owns the storage layer and takes `EncryptionProvider` as a constructor arg; (b) `tasker-worker` and `tasker-orchestration` each hold their own instance. Option (a) is cleaner architecturally.

2. **DEK cache scope.** Per-step execution (conservative), per-task (moderate), or per-worker (efficient)? Start with per-step and measure KMS call rates in performance testing.

3. **Encryption version negotiation.** The `EncryptedValue.version` field is reserved for future algorithm migration. Define a version negotiation protocol now so future versions are not backward-incompatible with existing stored data.

---

## Spike S4: DataClassifier and Trace/Log Safety

*Goal: Establish the observability protection layer.*

*This spike can proceed in parallel with Phase 1C after S1 and S2 are complete.*

### What to Deliver

1. **`DataClassifier` implementation**
   - `redact(value, scope) -> Value`
   - `is_classified(path, scope) -> bool`
   - `redact_span_attributes(attrs, scope) -> attrs`
   - JsonPath expression matching (using `jsonpath-rust` crate)
   - `DataClassifier::from_spec(spec: &DataClassificationSpec) -> Self`
   - Unit tests: redaction of scalar fields, array items, nested objects, wildcards; verify original value is not modified; verify redaction tokens are correct

2. **`DataClassificationSpec` parsing in `tasker-sdk`**
   - Parse `data_classification:` section from TaskTemplate YAML
   - Validate paths are syntactically valid JsonPath
   - Validate classification values
   - Emit warnings for paths that don't resolve against known schemas
   - Integration with existing `template_validate` command

3. **Composition executor integration in `tasker-worker`**
   - All trace span emissions in the composition executor pass data through `DataClassifier::redact()` before adding to span attributes
   - All error log records with context data pass through `DataClassifier::redact()` before emission
   - Tracing span attribute helper: `classified_span_attr(name, value, scope, classifier)`

4. **TaskTemplateManager caches compiled DataClassifier**
   - One `DataClassifier` instance per loaded template
   - Populated when the template is loaded, reused for all step executions

5. **Documentation: template authoring guide**
   - How to write `data_classification:` sections
   - When to use `trace_behavior: redact` vs `omit`
   - When to use `log_behavior: redact` vs `omit`
   - The difference between Classification (observability protection) and encryption (at-rest protection)
   - Example: payment workflow with complete data_classification spec
   - Example: KYC workflow with complete data_classification spec

### Acceptance Criteria

- `cargo test -p tasker-secure` passes for DataClassifier (no infrastructure needed)
- `cargo test -p tasker-sdk` includes tests for `data_classification:` section parsing
- A composition executor that processes a step with PII in `.context` emits trace spans where the PII fields are `[REDACTED:pii]`
- A template without a `data_classification:` section has no overhead (classifier is `None`, no redaction passes)
- `tasker-ctl composition validate` reports parsing errors for invalid `data_classification:` sections
- The template authoring guide is complete and reviewed

### Wildcard Path Performance Investigation (Must Resolve in S4)

Before finalizing the DataClassifier design, measure the cost of JsonPath wildcard matching on representative payloads:

- A context JSON with 20 fields, some nested, with 3 classification rules (2 exact, 1 wildcard)
- 1000 iterations of `classifier.redact(context, DataScope::TaskContext)`
- Target: < 1ms per redaction call for this payload size
- If wildcard matching is too expensive, consider a "simple mode" that only supports exact paths

---

## Parallel Work Summary

```
Right now (unblocked):
  Phase 1A: jaq-core integration
  Phase 1B: core grammar types
  Phase 1D: composition validator + executor (standalone)
  Phase 1E: workflow modeling

Blocked until S1 complete:
  S2: ResourceRegistry
  Dog-fooding: ConfigString in config layer

Blocked until S2 complete:
  Phase 1C: acquire, persist, emit executor implementations (stubs)
  Phase 1F: integration tests (depends on Phase 1C)

Parallel with Phase 1C (can start after S1+S2 complete):
  S3: Encryption at rest
  S4: DataClassifier + trace/log safety

Blocked until Phase 1C + S3 + S4 complete:
  Phase 3A: StepDefinition extension
  Phase 3B: CompositionExecutor as StepHandler
```

---

## Estimating Effort

| Spike | Complexity | Key Risk | Blocking |
|-------|-----------|----------|---------|
| **S1** | Low-Medium | rops SOPS integration (optional); config layer dog-fooding is the important part | Phase 1C stubs indirectly; S2 directly |
| **S2** | Medium | PostgreSQL pool refresh-on-auth-failure; ExecutionContext integration into tasker-worker | Phase 1C stubs directly |
| **S3** | Medium-High | DEK cache design; KMS performance at scale; storage layer integration | Phase 3 worker integration |
| **S4** | Low-Medium | JsonPath wildcard performance; trace/log integration points | Phase 3 worker integration |

S1 and S2 together are the immediate priority. S1 is fast (mostly trait definitions and simple implementations). S2 has more moving parts (pool management, the refresh protocol, integrating `ExecutionContext` into the existing worker machinery) but is scoped clearly.

The goal of S1 + S2 is to produce a stable `ResourceHandle` interface and `InMemoryResourceHandle` test utility so that Phase 1C can write correct, testable grammar executor stubs. The full production implementations (Vault, AWS KMS, credential rotation) are important but do not block the grammar validation work.

---

*Return to `CLAUDE.md` for the full reading order, or to `docs/action-grammar/implementation-phases.md` for how these spikes integrate with the grammar implementation timeline.*
