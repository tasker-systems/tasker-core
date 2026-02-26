# ADR-012: Permission Enforcement Boundary

**Status**: Accepted
**Date**: 2026-01
**Ticket**: TAS-150

## Context

Tasker Core needed an authentication and authorization model. The key question was whether Tasker should manage the full identity lifecycle (user management, role assignment, authentication) or focus only on enforcing permissions at the API boundary.

External systems (identity providers, RBAC platforms, enterprise SSO) already handle identity management well. Duplicating this within Tasker would create maintenance burden and integration friction.

## Decision

Tasker **enforces permissions** but does **not manage identity**:

- **External identity**: Authentication tokens (JWT, API keys) are issued by external systems. Tasker validates them but doesn't create users or manage roles.
- **JWKS support**: Dynamic key rotation via JSON Web Key Sets from external providers.
- **Permission vocabulary**: Fine-grained permissions scoped to Tasker's domain:
  - `tasks:*` — Task lifecycle operations
  - `steps:*` — Step management
  - `dlq:*` — Dead letter queue access
  - `templates:*` — Template management
  - `system:config:*` — System configuration
  - `worker:*` — Worker management
- **Health endpoints always public**: `/health/*` and `/metrics` endpoints are never gated — Kubernetes probes and monitoring must always work.
- **API key support**: For service-to-service communication where JWT is unnecessary overhead.

## Consequences

### Positive

- Tasker stays focused on orchestration — no user management, no password storage, no session management
- Integrates with any identity provider (Auth0, Keycloak, Azure AD, custom) via standard JWT/JWKS
- Health and metrics endpoints are always accessible for operational tooling
- Permission vocabulary maps directly to Tasker's API surface — no abstraction mismatch

### Negative

- Requires an external identity provider for JWT-based auth (API keys work standalone)
- Permission-to-role mapping must be managed outside Tasker
- No built-in audit of "who did what" beyond what the JWT claims provide

### Neutral

- API keys provide a simpler path for development and service-to-service calls
- Permission enforcement is consistent across REST and gRPC transports

## Alternatives Considered

### Alternative 1: Built-In User Management

Tasker manages users, roles, and permissions internally. Rejected because it duplicates infrastructure that external identity providers handle better, and creates a maintenance burden for password hashing, session management, and RBAC administration.

### Alternative 2: No Authorization (Rely on Network Isolation)

Trust network boundaries and don't implement authorization. Rejected because multi-tenant deployments and shared infrastructure require fine-grained access control at the API level.

### Alternative 3: Policy Engine Integration (OPA/Cedar)

Integrate a policy engine for complex authorization rules. Rejected as premature — Tasker's permission model is simple enough that a policy engine adds complexity without proportional benefit. Can be revisited if permission rules become more complex.

## References

- [TAS-150 Spec](../ticket-specs/) (archived)
- [Auth Overview](../auth/README.md)
- [Permissions and Routes](../auth/permissions.md)
- [API Security Guide](../guides/api-security.md)
