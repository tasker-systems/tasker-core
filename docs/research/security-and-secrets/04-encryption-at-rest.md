# Concern 4: Encryption at Rest

*How sensitive data in task context, step results, and checkpoint records is protected in PostgreSQL*

*Research spike — March 2026*

---

## The Threat Model

What database-level encryption protects against: access to the PostgreSQL data files — disk exfiltration, backup theft, compromised storage layer. This is real and important, but it is typically provided by infrastructure-level controls (encrypted EBS volumes, encrypted RDS snapshots, filesystem encryption at the host level). These controls are outside Tasker's scope.

What database-level encryption does NOT protect against: a compromised database user with `SELECT` privileges, a compromised DBA account, a SQL injection vulnerability in the application, or any access that bypasses the encryption at the storage layer and reads through the normal database connection path.

Application-level encryption — encrypting specific values before they are stored in PostgreSQL — protects against all of the above, including the compromised-DBA scenario. It is the layer that adds meaningful protection beyond what infrastructure-level encryption already provides.

The trade-off: application-level encryption adds complexity (key management, query limitations on encrypted fields, performance overhead) and should only be applied where the risk justifies the cost. For general workflow data, the overhead is not justified. For fields specifically classified as sensitive (PII, payment data, healthcare data), it is.

---

## What Tasker Stores That Could Contain Sensitive Data

Surveying the database columns that hold user-provided or user-derived data:

| Table | Column | What it contains |
|-------|--------|-----------------|
| `tasks` | `context` | JSONB — the task's input data; can contain PII |
| `workflow_steps` | `inputs` | JSONB — step-specific inputs; can contain PII |
| `workflow_steps` | `results` | JSONB — step execution outputs; can contain PII |
| `workflow_steps` | `checkpoint` | JSONB — intermediate computation state; can contain PII |
| `workflow_step_transitions` | `metadata` | JSONB — transition metadata; usually not PII |
| `named_tasks` | `configuration` | JSONB — task template config; credentials must not be here |

The primary targets for field-level encryption are `tasks.context`, `workflow_steps.results`, and `workflow_steps.checkpoint`. These are the JSONB columns where sensitive workflow data is most likely to persist long-term.

---

## Envelope Encryption: The Standard Model

Envelope encryption is the pattern used by AWS S3 SSE, RDS at-rest encryption, GCP Cloud Storage, Vault Transit, and every mature cloud at-rest encryption system. It provides:

- **Key rotation without re-encrypting data**: Rotating the Key Encryption Key (KEK) means re-encrypting only the Data Encryption Keys (DEKs), not the data itself. At large scale, this difference is material.
- **Per-record uniqueness**: Each record gets a unique DEK. Compromise of one DEK exposes one record, not all records encrypted with the same key.
- **KMS-backed key management**: The KEK never leaves the KMS (AWS KMS, GCP KMS, Vault Transit). The application only ever handles the encrypted DEK, never the KEK itself.

The stored format for an encrypted field:

```json
{
  "_enc": {
    "v": 1,
    "alg": "AES-256-GCM",
    "dek_enc": "base64-encoded-encrypted-DEK",
    "iv": "base64-encoded-nonce",
    "ciphertext": "base64-encoded-encrypted-value"
  }
}
```

The `dek_enc` value is the DEK encrypted with the KEK. Decryption requires asking the KMS to decrypt the DEK, then using the plaintext DEK to decrypt the ciphertext. The plaintext DEK is ephemeral — used for this decryption and then discarded.

---

## EncryptionProvider Strategy

The same strategy pattern as `SecretsProvider`:

```rust
/// An encryption provider handles envelope-encrypted value lifecycle:
/// encrypting values before storage, and decrypting after retrieval.
///
/// Built-in implementations:
/// - AesGcmEncryptionProvider: local key for development/testing only
/// - AwsKmsEncryptionProvider: AWS KMS KEK management
/// - VaultTransitEncryptionProvider: Vault Transit engine KEK management
///
/// Organizations with other KMS backends can implement this trait in their
/// custom worker binary.
#[async_trait]
pub trait EncryptionProvider: Send + Sync + fmt::Debug {
    /// Encrypt a raw byte slice. Returns an EncryptedValue containing
    /// the encrypted DEK, the nonce, and the ciphertext.
    async fn encrypt(&self, plaintext: &[u8]) -> Result<EncryptedValue, EncryptionError>;

    /// Decrypt an EncryptedValue. Returns the original plaintext bytes.
    async fn decrypt(&self, ciphertext: &EncryptedValue) -> Result<Vec<u8>, EncryptionError>;

    /// Encrypt specific fields in a JSON value according to a field spec list.
    /// Returns a new JSON value with specified fields replaced by their
    /// EncryptedValue representations.
    async fn encrypt_fields(
        &self,
        value: &Value,
        specs: &[FieldEncryptionSpec],
    ) -> Result<Value, EncryptionError>;

    /// Decrypt specific fields in a JSON value.
    /// Returns a new JSON value with encrypted fields replaced by their
    /// original values.
    async fn decrypt_fields(
        &self,
        value: &Value,
        specs: &[FieldEncryptionSpec],
    ) -> Result<Value, EncryptionError>;

    /// Provider name for diagnostics.
    fn provider_name(&self) -> &str;

    /// Verify the provider is reachable and key access is working.
    async fn health_check(&self) -> Result<(), EncryptionError>;
}

/// Specification for a field to encrypt/decrypt.
#[derive(Debug, Clone)]
pub struct FieldEncryptionSpec {
    /// JsonPath expression identifying the field to encrypt.
    pub path: String,
    /// Data classification driving the encryption requirement.
    pub classification: DataClassification,
}

/// The stored representation of an encrypted field value.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptedValue {
    pub version: u8,
    pub algorithm: String,     // "AES-256-GCM"
    pub dek_encrypted: String, // base64-encoded DEK encrypted with KEK
    pub iv: String,            // base64-encoded nonce
    pub ciphertext: String,    // base64-encoded encrypted data
}
```

### Provider Implementations

**AesGcmEncryptionProvider** (local key — development/testing only):

Uses a locally-held AES-256 key. No KMS. Good for testing the encryption machinery without a KMS dependency. Explicitly documented as not appropriate for production because the key lives in the process (or in a config file), defeating the purpose of encryption.

```rust
pub struct AesGcmEncryptionProvider {
    /// The key material — loaded from config, held in memory.
    /// In production, this key would need to be a KEK managed by a KMS.
    /// For testing, it's a literal key value.
    key: SecretValue, // uses secrecy crate — zeroized on drop
}
```

**AwsKmsEncryptionProvider**:

Uses AWS KMS for KEK management. The plaintext DEK is generated locally, used for the encryption, then immediately encrypted with KMS and discarded. Only the encrypted DEK is stored.

```rust
pub struct AwsKmsEncryptionProvider {
    client: Arc<KmsClient>,
    /// ARN of the KMS key used as the KEK
    key_arn: String,
    region: String,
    /// DEK cache: (encrypted_dek -> plaintext_dek)
    /// Reduces KMS API calls for frequently-encrypted values.
    /// Cache TTL and size configurable.
    dek_cache: Arc<DekCache>,
}
```

The DEK cache is the primary performance optimization. KMS API calls have latency (1-5ms per call typically). For a workflow that encrypts 10 fields in a step result, without caching that's 10 KMS decrypt calls just for reading. The cache maps `encrypted_dek -> plaintext_dek` for the duration of a step execution, then is cleared. The security model: the plaintext DEK is held in memory only during the operation that uses it, then cleared. The cache is scoped to the current step execution to avoid holding plaintext keys longer than necessary.

**VaultTransitEncryptionProvider**:

Vault's Transit engine provides "encryption as a service." The plaintext never leaves the application — Vault encrypts/decrypts bytes through an API, using keys that exist only inside Vault. This is a stronger model than KMS because it limits key exposure: Vault can log every encrypt/decrypt operation, enforce access policies, and audit who decrypted what.

```rust
pub struct VaultTransitEncryptionProvider {
    client: Arc<VaultClient>,
    /// The Vault Transit key name
    key_name: String,
    mount: String, // defaults to "transit"
}
```

---

## Integration: Transparent Encryption at the sqlx Layer

The encryption must be transparent to the composition executor. A jaq filter that reads `.context.credit_card_number` expects a string. If the stored value is an `EncryptedValue` JSON object, the filter gets garbage.

The solution: field-level encryption and decryption happen at the database access layer, before composition context is assembled and after step results are produced. The composition executor never sees encrypted values.

```
Task creation:
  TaskRequest arrives with context JSON
      ↓
  TaskStorageService.store_task(context):
      if template has encryption specs for context fields:
          encrypted_context = encryption_provider
              .encrypt_fields(context, template.encryption_specs.context_fields)
      else:
          encrypted_context = context
      sqlx INSERT tasks (context = encrypted_context)

Step execution:
  Composition executor assembles context envelope:
      tasks.context from DB (may contain encrypted fields)
          ↓
      TaskStorageService.load_context(task_uuid):
          raw_context = sqlx SELECT tasks.context WHERE uuid = task_uuid
          if template has encryption specs for context fields:
              decrypted_context = encryption_provider
                  .decrypt_fields(raw_context, template.encryption_specs.context_fields)
          else:
              decrypted_context = raw_context
          return decrypted_context
      ↓
  Composition executor uses decrypted_context in .context

Step result storage:
  StepExecutionResult produced by composition executor
      ↓
  StepResultStorageService.store_result(step_uuid, result):
      if template has encryption specs for result fields:
          encrypted_result = encryption_provider
              .encrypt_fields(result, template.encryption_specs.result_fields)
      else:
          encrypted_result = result
      sqlx INSERT/UPDATE workflow_steps (results = encrypted_result)
```

This is the "encrypt on write, decrypt on read" pattern. The application layer handles it; the composition executor is unaware.

---

## TaskTemplate Encryption Spec

The template author declares which fields require at-rest encryption. This extends the `data_classification` section from Concern 3:

```yaml
name: process_payment
namespace: billing
version: "1.0.0"

data_classification:
  context_fields:
    - path: "$.credit_card_number"
      classification: payment_card
      trace_behavior: omit
      log_behavior: omit
      encrypt_at_rest: true       # ← field-level encryption enabled

    - path: "$.card_cvv"
      classification: payment_card
      trace_behavior: omit
      log_behavior: omit
      encrypt_at_rest: true

    - path: "$.billing_address.street"
      classification: pii
      trace_behavior: redact
      log_behavior: redact
      encrypt_at_rest: false      # ← trace/log protection only; not encrypted at rest

  result_fields:
    - path: "$.steps.tokenize_card.card_token"
      classification: payment_card
      trace_behavior: redact
      log_behavior: redact
      encrypt_at_rest: true
```

The `encrypt_at_rest: true` flag means this field will be encrypted before database storage and decrypted on retrieval. The encryption provider used is the one configured for the worker.

This is a deliberate authoring decision by the template author. Not every classified field needs to be encrypted at rest — the overhead is only justified when the database persistence of that value is a material risk. A field that is transient (present in `.context`, consumed in step 1, not in any step result) may not warrant at-rest encryption. A field stored permanently in a step result as a record of a transaction may warrant it.

---

## Open Questions for the Research Spike

These are questions to be resolved in Spike S3:

1. **DEK cache scope and TTL.** The cache maps encrypted DEKs to plaintext DEKs. What is the right scope? Per-step execution (cleared after step completes), per-worker (held for the worker lifetime, bounded size), or per-task (cleared when task completes)? The per-step scope is most conservative (plaintext DEKs held for the minimum time) but generates the most KMS API calls. Per-worker with an LRU cache and a short TTL is a reasonable middle ground.

2. **Key rotation impact on existing data.** When a KEK is rotated (new KMS key version), existing records have DEKs encrypted with the old key version. These records continue to be decryptable until the old key version is explicitly disabled. The standard recommendation is a rotation window during which both old and new key versions are active. Does Tasker need a migration tool to re-encrypt existing DEKs with the new key version? How long does the rotation window need to be?

3. **Schema migration considerations.** Fields that are newly marked `encrypt_at_rest: true` in an updated template version will have unencrypted existing values in the database. Reads of those records will get unencrypted values (not an `EncryptedValue` JSON structure). The decrypt function needs to detect this: if the field value doesn't look like an `EncryptedValue` (no `_enc` wrapper), return it as-is. This graceful degradation means migration can happen incrementally without a full backfill.

4. **Impact on the orchestration server.** The orchestration server writes task context at task creation time (`POST /v1/tasks`). If that context needs to be encrypted, the orchestration server also needs access to the `EncryptionProvider`. This means `tasker-secure` is a dependency of `tasker-orchestration` as well as `tasker-worker`. Is that acceptable? Alternative: encrypt at query time in the storage layer (the sqlx calls in `tasker-shared`), so the encryption provider is a `tasker-shared` concern.

5. **Performance at scale.** A high-throughput deployment that processes 1000 steps/second and encrypts 5 fields per step is generating 5000 KMS calls per second. AWS KMS has per-region quotas. The DEK cache must be large enough to keep the KMS call rate manageable. This is a real operational concern for organizations with strong encryption requirements and high throughput.

---

*Read next: `05-tasker-secure-crate-proposal.md` for the full crate design, trait surface, dependency graph, and feature gates.*
