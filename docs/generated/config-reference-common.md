# Configuration Reference: common

> 65/65 parameters documented

---

## backoff

**Path:** `common.backoff`

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `backoff_multiplier` | `f64` | `2.0` | Multiplier applied to the previous delay for exponential backoff calculations |
| `default_backoff_seconds` | `Vec<u32>` | `[1, 5, 15, 30, 60]` | Sequence of backoff delays in seconds for successive retry attempts |
| `jitter_enabled` | `bool` | `true` | Add random jitter to backoff delays to prevent thundering herd on retry |
| `jitter_max_percentage` | `f64` | `0.15` | Maximum jitter as a fraction of the computed backoff delay |
| `max_backoff_seconds` | `u32` | `3600` | Hard upper limit on any single backoff delay |

#### `common.backoff.backoff_multiplier`

Multiplier applied to the previous delay for exponential backoff calculations

- **Type:** `f64`
- **Default:** `2.0`
- **Valid Range:** 1.0-10.0
- **System Impact:** Controls how aggressively delays grow; 2.0 means each delay is double the previous

#### `common.backoff.default_backoff_seconds`

Sequence of backoff delays in seconds for successive retry attempts

- **Type:** `Vec<u32>`
- **Default:** `[1, 5, 15, 30, 60]`
- **Valid Range:** non-empty array of positive integers
- **System Impact:** Defines the retry cadence; after exhausting the array, the last value is reused up to max_backoff_seconds

#### `common.backoff.jitter_enabled`

Add random jitter to backoff delays to prevent thundering herd on retry

- **Type:** `bool`
- **Default:** `true`
- **Valid Range:** true/false
- **System Impact:** When true, backoff delays are randomized within jitter_max_percentage to spread retries across time

#### `common.backoff.jitter_max_percentage`

Maximum jitter as a fraction of the computed backoff delay

- **Type:** `f64`
- **Default:** `0.15`
- **Valid Range:** 0.0-1.0
- **System Impact:** A value of 0.15 means delays vary by up to +/-15% of the base delay

#### `common.backoff.max_backoff_seconds`

Hard upper limit on any single backoff delay

- **Type:** `u32`
- **Default:** `3600`
- **Valid Range:** 1-3600
- **System Impact:** Caps exponential backoff growth to prevent excessively long delays between retries

## cache

**Path:** `common.cache`

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `analytics_ttl_seconds` | `u32` | `60` | Time-to-live in seconds for cached analytics and metrics data |
| `backend` | `String` | `"redis"` | Cache backend implementation: 'redis' (distributed) or 'moka' (in-process) |
| `default_ttl_seconds` | `u32` | `3600` | Default time-to-live in seconds for cached entries |
| `enabled` | `bool` | `false` | Enable the distributed cache layer for template and analytics data |
| `template_ttl_seconds` | `u32` | `3600` | Time-to-live in seconds for cached task template definitions |

#### `common.cache.analytics_ttl_seconds`

Time-to-live in seconds for cached analytics and metrics data

- **Type:** `u32`
- **Default:** `60`
- **Valid Range:** 1-3600
- **System Impact:** Analytics data is write-heavy and changes frequently; short TTL (60s) keeps metrics current

#### `common.cache.backend`

Cache backend implementation: 'redis' (distributed) or 'moka' (in-process)

- **Type:** `String`
- **Default:** `"redis"`
- **Valid Range:** redis | moka
- **System Impact:** Redis is required for multi-instance deployments to avoid stale data; moka is suitable for single-instance or DoS protection

#### `common.cache.default_ttl_seconds`

Default time-to-live in seconds for cached entries

- **Type:** `u32`
- **Default:** `3600`
- **Valid Range:** 1-86400
- **System Impact:** Controls how long cached data remains valid before being re-fetched from the database

#### `common.cache.enabled`

Enable the distributed cache layer for template and analytics data

- **Type:** `bool`
- **Default:** `false`
- **Valid Range:** true/false
- **System Impact:** When false, all cache reads fall through to direct database queries; no cache dependency required

#### `common.cache.template_ttl_seconds`

Time-to-live in seconds for cached task template definitions

- **Type:** `u32`
- **Default:** `3600`
- **Valid Range:** 1-86400
- **System Impact:** Template changes take up to this long to propagate; shorter values increase DB load, longer values improve performance

### moka

**Path:** `common.cache.moka`

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `max_capacity` | `u64` | `10000` | Maximum number of entries the in-process Moka cache can hold |

#### `common.cache.moka.max_capacity`

Maximum number of entries the in-process Moka cache can hold

- **Type:** `u64`
- **Default:** `10000`
- **Valid Range:** 1-1000000
- **System Impact:** Bounds memory usage; least-recently-used entries are evicted when capacity is reached

### redis

**Path:** `common.cache.redis`

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `connection_timeout_seconds` | `u32` | `5` | Maximum time to wait when establishing a new Redis connection |
| `database` | `u32` | `0` | Redis database number (0-15) |
| `max_connections` | `u32` | `10` | Maximum number of connections in the Redis connection pool |
| `url` | `String` | `"${REDIS_URL:-redis://localhost:6379}"` | Redis connection URL |

#### `common.cache.redis.connection_timeout_seconds`

Maximum time to wait when establishing a new Redis connection

- **Type:** `u32`
- **Default:** `5`
- **Valid Range:** 1-60
- **System Impact:** Connections that cannot be established within this timeout fail; cache falls back to database

#### `common.cache.redis.database`

Redis database number (0-15)

- **Type:** `u32`
- **Default:** `0`
- **Valid Range:** 0-15
- **System Impact:** Isolates Tasker cache keys from other applications sharing the same Redis instance

#### `common.cache.redis.max_connections`

Maximum number of connections in the Redis connection pool

- **Type:** `u32`
- **Default:** `10`
- **Valid Range:** 1-500
- **System Impact:** Bounds concurrent Redis operations; increase for high cache throughput workloads

#### `common.cache.redis.url`

Redis connection URL

- **Type:** `String`
- **Default:** `"${REDIS_URL:-redis://localhost:6379}"`
- **Valid Range:** valid Redis URI
- **System Impact:** Must be reachable when cache is enabled with redis backend

## circuit_breakers

**Path:** `common.circuit_breakers`

### component_configs

**Path:** `common.circuit_breakers.component_configs`

#### cache

**Path:** `common.circuit_breakers.component_configs.cache`

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `failure_threshold` | `u32` | `5` | Failures before the cache circuit breaker trips to Open |
| `success_threshold` | `u32` | `2` | Successes in Half-Open required to close the cache breaker |

#### `common.circuit_breakers.component_configs.cache.failure_threshold`

Failures before the cache circuit breaker trips to Open

- **Type:** `u32`
- **Default:** `5`
- **Valid Range:** 1-100
- **System Impact:** Protects Redis/Dragonfly operations; when tripped, cache reads fall through to database

#### `common.circuit_breakers.component_configs.cache.success_threshold`

Successes in Half-Open required to close the cache breaker

- **Type:** `u32`
- **Default:** `2`
- **Valid Range:** 1-100
- **System Impact:** Low threshold (2) for fast recovery since cache failures gracefully degrade to database

#### messaging

**Path:** `common.circuit_breakers.component_configs.messaging`

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `failure_threshold` | `u32` | `5` | Failures before the messaging circuit breaker trips to Open |
| `success_threshold` | `u32` | `2` | Successes in Half-Open required to close the messaging breaker |

#### `common.circuit_breakers.component_configs.messaging.failure_threshold`

Failures before the messaging circuit breaker trips to Open

- **Type:** `u32`
- **Default:** `5`
- **Valid Range:** 1-100
- **System Impact:** Protects the messaging layer (PGMQ or RabbitMQ); when tripped, queue send/receive operations are short-circuited

#### `common.circuit_breakers.component_configs.messaging.success_threshold`

Successes in Half-Open required to close the messaging breaker

- **Type:** `u32`
- **Default:** `2`
- **Valid Range:** 1-100
- **System Impact:** Lower threshold (2) allows faster recovery since messaging failures are typically transient

#### task_readiness

**Path:** `common.circuit_breakers.component_configs.task_readiness`

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `failure_threshold` | `u32` | `10` | Failures before the task readiness circuit breaker trips to Open |
| `success_threshold` | `u32` | `3` | Successes in Half-Open required to close the task readiness breaker |

#### `common.circuit_breakers.component_configs.task_readiness.failure_threshold`

Failures before the task readiness circuit breaker trips to Open

- **Type:** `u32`
- **Default:** `10`
- **Valid Range:** 1-100
- **System Impact:** Higher than default (10 vs 5) because task readiness queries are frequent and transient failures are expected

#### `common.circuit_breakers.component_configs.task_readiness.success_threshold`

Successes in Half-Open required to close the task readiness breaker

- **Type:** `u32`
- **Default:** `3`
- **Valid Range:** 1-100
- **System Impact:** Slightly higher than default (3) for extra confidence before resuming readiness queries

#### web

**Path:** `common.circuit_breakers.component_configs.web`

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `failure_threshold` | `u32` | `5` | Failures before the web/API database circuit breaker trips to Open |
| `success_threshold` | `u32` | `2` | Successes in Half-Open required to close the web database breaker |

#### `common.circuit_breakers.component_configs.web.failure_threshold`

Failures before the web/API database circuit breaker trips to Open

- **Type:** `u32`
- **Default:** `5`
- **Valid Range:** 1-100
- **System Impact:** Protects API database operations; when tripped, API requests receive fast 503 errors instead of waiting for timeouts

#### `common.circuit_breakers.component_configs.web.success_threshold`

Successes in Half-Open required to close the web database breaker

- **Type:** `u32`
- **Default:** `2`
- **Valid Range:** 1-100
- **System Impact:** Standard threshold (2) provides confidence in recovery before restoring full API traffic

### default_config

**Path:** `common.circuit_breakers.default_config`

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `failure_threshold` | `u32` | `5` | Number of consecutive failures before a circuit breaker trips to the Open state |
| `success_threshold` | `u32` | `2` | Number of consecutive successes in Half-Open state required to close the circuit breaker |
| `timeout_seconds` | `u32` | `30` | Duration in seconds a circuit breaker stays Open before transitioning to Half-Open for probe requests |

#### `common.circuit_breakers.default_config.failure_threshold`

Number of consecutive failures before a circuit breaker trips to the Open state

- **Type:** `u32`
- **Default:** `5`
- **Valid Range:** 1-100
- **System Impact:** Lower values make the breaker more sensitive; higher values tolerate more transient failures before tripping

#### `common.circuit_breakers.default_config.success_threshold`

Number of consecutive successes in Half-Open state required to close the circuit breaker

- **Type:** `u32`
- **Default:** `2`
- **Valid Range:** 1-100
- **System Impact:** Higher values require more proof of recovery before restoring full traffic

#### `common.circuit_breakers.default_config.timeout_seconds`

Duration in seconds a circuit breaker stays Open before transitioning to Half-Open for probe requests

- **Type:** `u32`
- **Default:** `30`
- **Valid Range:** 1-300
- **System Impact:** Controls recovery speed; shorter timeouts attempt recovery sooner but risk repeated failures

### global_settings

**Path:** `common.circuit_breakers.global_settings`

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `metrics_collection_interval_seconds` | `u32` | `30` | Interval in seconds between circuit breaker metrics collection sweeps |
| `min_state_transition_interval_seconds` | `f64` | `5.0` | Minimum time in seconds between circuit breaker state transitions |

#### `common.circuit_breakers.global_settings.metrics_collection_interval_seconds`

Interval in seconds between circuit breaker metrics collection sweeps

- **Type:** `u32`
- **Default:** `30`
- **Valid Range:** 1-3600
- **System Impact:** Controls how frequently circuit breaker state, failure counts, and transition counts are collected for observability

#### `common.circuit_breakers.global_settings.min_state_transition_interval_seconds`

Minimum time in seconds between circuit breaker state transitions

- **Type:** `f64`
- **Default:** `5.0`
- **Valid Range:** 0.0-60.0
- **System Impact:** Prevents rapid oscillation between Open and Closed states during intermittent failures

## database

**Path:** `common.database`

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `url` | `String` | `"${DATABASE_URL:-postgresql://localhost/tasker}"` | PostgreSQL connection URL for the primary database |

#### `common.database.url`

PostgreSQL connection URL for the primary database

- **Type:** `String`
- **Default:** `"${DATABASE_URL:-postgresql://localhost/tasker}"`
- **Valid Range:** valid PostgreSQL connection URI
- **System Impact:** All task, step, and workflow state is stored here; must be reachable at startup

**Environment Recommendations:**

| Environment | Value | Rationale |
|-------------|-------|-----------|
| development | postgresql://localhost/tasker | Local default, no auth |
| production | ${DATABASE_URL} | Always use env var injection for secrets rotation |
| test | postgresql://tasker:tasker@localhost:5432/tasker_rust_test | Isolated test database with known credentials |

**Related:** `common.database.pool.max_connections`, `common.pgmq_database.url`

### pool

**Path:** `common.database.pool`

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `acquire_timeout_seconds` | `u32` | `10` | Maximum time to wait when acquiring a connection from the pool |
| `idle_timeout_seconds` | `u32` | `300` | Time before an idle connection is closed and removed from the pool |
| `max_connections` | `u32` | `25` | Maximum number of concurrent database connections in the pool |
| `max_lifetime_seconds` | `u32` | `1800` | Maximum total lifetime of a connection before it is closed and replaced |
| `min_connections` | `u32` | `5` | Minimum number of idle connections maintained in the pool |
| `slow_acquire_threshold_ms` | `u32` | `100` | Threshold in milliseconds above which connection acquisition is logged as slow |

#### `common.database.pool.acquire_timeout_seconds`

Maximum time to wait when acquiring a connection from the pool

- **Type:** `u32`
- **Default:** `10`
- **Valid Range:** 1-300
- **System Impact:** Queries fail with a timeout error if no connection is available within this window

#### `common.database.pool.idle_timeout_seconds`

Time before an idle connection is closed and removed from the pool

- **Type:** `u32`
- **Default:** `300`
- **Valid Range:** 1-3600
- **System Impact:** Controls how quickly the pool shrinks back to min_connections after load drops

#### `common.database.pool.max_connections`

Maximum number of concurrent database connections in the pool

- **Type:** `u32`
- **Default:** `25`
- **Valid Range:** 1-1000
- **System Impact:** Controls database connection concurrency; too few causes query queuing under load, too many risks DB resource exhaustion

**Environment Recommendations:**

| Environment | Value | Rationale |
|-------------|-------|-----------|
| development | 10-25 | Small pool for local development |
| production | 30-50 | Scale based on worker count and concurrent task volume |
| test | 10-30 | Moderate pool; cluster tests may run 10 services sharing the same DB |

**Related:** `common.database.pool.min_connections`, `common.database.pool.acquire_timeout_seconds`

#### `common.database.pool.max_lifetime_seconds`

Maximum total lifetime of a connection before it is closed and replaced

- **Type:** `u32`
- **Default:** `1800`
- **Valid Range:** 60-86400
- **System Impact:** Prevents connection drift from server-side config changes or memory leaks in long-lived connections

#### `common.database.pool.min_connections`

Minimum number of idle connections maintained in the pool

- **Type:** `u32`
- **Default:** `5`
- **Valid Range:** 0-100
- **System Impact:** Keeps connections warm to avoid cold-start latency on first queries after idle periods

#### `common.database.pool.slow_acquire_threshold_ms`

Threshold in milliseconds above which connection acquisition is logged as slow

- **Type:** `u32`
- **Default:** `100`
- **Valid Range:** 10-60000
- **System Impact:** Observability: slow acquire warnings indicate pool pressure or network issues

## execution

**Path:** `common.execution`

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `environment` | `String` | `"development"` | Runtime environment identifier used for configuration context selection and logging |
| `step_enqueue_batch_size` | `u32` | `50` | Number of steps to enqueue in a single batch during task initialization |

#### `common.execution.environment`

Runtime environment identifier used for configuration context selection and logging

- **Type:** `String`
- **Default:** `"development"`
- **Valid Range:** test | development | production
- **System Impact:** Affects log levels, default tuning, and environment-specific behavior throughout the system

#### `common.execution.step_enqueue_batch_size`

Number of steps to enqueue in a single batch during task initialization

- **Type:** `u32`
- **Default:** `50`
- **Valid Range:** 1-1000
- **System Impact:** Controls step enqueueing throughput; larger batches reduce round trips but increase per-batch latency

## mpsc_channels

**Path:** `common.mpsc_channels`

### event_publisher

**Path:** `common.mpsc_channels.event_publisher`

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `event_queue_buffer_size` | `usize` | `5000` | Bounded channel capacity for the event publisher MPSC channel |

#### `common.mpsc_channels.event_publisher.event_queue_buffer_size`

Bounded channel capacity for the event publisher MPSC channel

- **Type:** `usize`
- **Default:** `5000`
- **Valid Range:** 100-100000
- **System Impact:** Controls backpressure for domain event publishing; smaller buffers apply backpressure sooner

### ffi

**Path:** `common.mpsc_channels.ffi`

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `ruby_event_buffer_size` | `usize` | `1000` | Bounded channel capacity for Ruby FFI event delivery |

#### `common.mpsc_channels.ffi.ruby_event_buffer_size`

Bounded channel capacity for Ruby FFI event delivery

- **Type:** `usize`
- **Default:** `1000`
- **Valid Range:** 100-50000
- **System Impact:** Buffers events between the Rust runtime and Ruby FFI layer; overflow triggers backpressure on the dispatch side

### overflow_policy

**Path:** `common.mpsc_channels.overflow_policy`

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `log_warning_threshold` | `f64` | `0.8` | Channel saturation fraction at which warning logs are emitted |

#### `common.mpsc_channels.overflow_policy.log_warning_threshold`

Channel saturation fraction at which warning logs are emitted

- **Type:** `f64`
- **Default:** `0.8`
- **Valid Range:** 0.0-1.0
- **System Impact:** A value of 0.8 means warnings fire when any channel reaches 80% capacity

#### metrics

**Path:** `common.mpsc_channels.overflow_policy.metrics`

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `saturation_check_interval_seconds` | `u32` | `30` | Interval in seconds between channel saturation metric samples |

#### `common.mpsc_channels.overflow_policy.metrics.saturation_check_interval_seconds`

Interval in seconds between channel saturation metric samples

- **Type:** `u32`
- **Default:** `30`
- **Valid Range:** 1-3600
- **System Impact:** Lower intervals give finer-grained capacity visibility but add sampling overhead

## pgmq_database

**Path:** `common.pgmq_database`

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `enabled` | `bool` | `true` | Enable PGMQ messaging subsystem |
| `url` | `String` | `"${PGMQ_DATABASE_URL:-}"` | PostgreSQL connection URL for a dedicated PGMQ database; when empty, PGMQ shares the primary database |

#### `common.pgmq_database.enabled`

Enable PGMQ messaging subsystem

- **Type:** `bool`
- **Default:** `true`
- **Valid Range:** true/false
- **System Impact:** When false, PGMQ queue operations are disabled; only useful if using RabbitMQ as the sole messaging backend

#### `common.pgmq_database.url`

PostgreSQL connection URL for a dedicated PGMQ database; when empty, PGMQ shares the primary database

- **Type:** `String`
- **Default:** `"${PGMQ_DATABASE_URL:-}"`
- **Valid Range:** valid PostgreSQL connection URI or empty string
- **System Impact:** Separating PGMQ to its own database isolates messaging I/O from task state queries, reducing contention under heavy load

**Related:** `common.database.url`, `common.pgmq_database.enabled`

### pool

**Path:** `common.pgmq_database.pool`

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `acquire_timeout_seconds` | `u32` | `5` | Maximum time to wait when acquiring a connection from the PGMQ pool |
| `idle_timeout_seconds` | `u32` | `300` | Time before an idle PGMQ connection is closed and removed from the pool |
| `max_connections` | `u32` | `15` | Maximum number of concurrent connections in the PGMQ database pool |
| `max_lifetime_seconds` | `u32` | `1800` | Maximum total lifetime of a PGMQ database connection before replacement |
| `min_connections` | `u32` | `3` | Minimum idle connections maintained in the PGMQ database pool |
| `slow_acquire_threshold_ms` | `u32` | `100` | Threshold in milliseconds above which PGMQ pool acquisition is logged as slow |

#### `common.pgmq_database.pool.acquire_timeout_seconds`

Maximum time to wait when acquiring a connection from the PGMQ pool

- **Type:** `u32`
- **Default:** `5`
- **Valid Range:** 1-300
- **System Impact:** Queue operations fail with timeout if no PGMQ connection is available within this window

#### `common.pgmq_database.pool.idle_timeout_seconds`

Time before an idle PGMQ connection is closed and removed from the pool

- **Type:** `u32`
- **Default:** `300`
- **Valid Range:** 1-3600
- **System Impact:** Controls how quickly the PGMQ pool shrinks after messaging load drops

#### `common.pgmq_database.pool.max_connections`

Maximum number of concurrent connections in the PGMQ database pool

- **Type:** `u32`
- **Default:** `15`
- **Valid Range:** 1-500
- **System Impact:** Separate from the main database pool; size according to messaging throughput requirements

#### `common.pgmq_database.pool.max_lifetime_seconds`

Maximum total lifetime of a PGMQ database connection before replacement

- **Type:** `u32`
- **Default:** `1800`
- **Valid Range:** 60-86400
- **System Impact:** Prevents connection drift in long-running PGMQ connections

#### `common.pgmq_database.pool.min_connections`

Minimum idle connections maintained in the PGMQ database pool

- **Type:** `u32`
- **Default:** `3`
- **Valid Range:** 0-100
- **System Impact:** Keeps PGMQ connections warm to avoid cold-start latency on queue operations

#### `common.pgmq_database.pool.slow_acquire_threshold_ms`

Threshold in milliseconds above which PGMQ pool acquisition is logged as slow

- **Type:** `u32`
- **Default:** `100`
- **Valid Range:** 10-60000
- **System Impact:** Observability: slow PGMQ acquire warnings indicate messaging pool pressure

## queues

**Path:** `common.queues`

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `backend` | `String` | `"${TASKER_MESSAGING_BACKEND:-pgmq}"` | Messaging backend: 'pgmq' (PostgreSQL-based, LISTEN/NOTIFY) or 'rabbitmq' (AMQP broker) |
| `default_visibility_timeout_seconds` | `u32` | `30` | Default time a dequeued message remains invisible to other consumers |
| `naming_pattern` | `String` | `"{namespace}_{name}_queue"` | Template pattern for constructing queue names from namespace and name |
| `orchestration_namespace` | `String` | `"orchestration"` | Namespace prefix for orchestration queue names |
| `worker_namespace` | `String` | `"worker"` | Namespace prefix for worker queue names |

#### `common.queues.backend`

Messaging backend: 'pgmq' (PostgreSQL-based, LISTEN/NOTIFY) or 'rabbitmq' (AMQP broker)

- **Type:** `String`
- **Default:** `"${TASKER_MESSAGING_BACKEND:-pgmq}"`
- **Valid Range:** pgmq | rabbitmq
- **System Impact:** Determines the entire message transport layer; pgmq requires only PostgreSQL, rabbitmq requires a separate AMQP broker

**Environment Recommendations:**

| Environment | Value | Rationale |
|-------------|-------|-----------|
| production | pgmq or rabbitmq | pgmq for simplicity, rabbitmq for high-throughput push semantics |
| test | pgmq | Single-dependency setup, simpler CI |

**Related:** `common.queues.pgmq`, `common.queues.rabbitmq`

#### `common.queues.default_visibility_timeout_seconds`

Default time a dequeued message remains invisible to other consumers

- **Type:** `u32`
- **Default:** `30`
- **Valid Range:** 1-3600
- **System Impact:** If a consumer fails to process a message within this window, the message becomes visible again for retry

#### `common.queues.naming_pattern`

Template pattern for constructing queue names from namespace and name

- **Type:** `String`
- **Default:** `"{namespace}_{name}_queue"`
- **Valid Range:** string containing {namespace} and {name} placeholders
- **System Impact:** Determines the actual PGMQ/RabbitMQ queue names; changing this after deployment requires manual queue migration

#### `common.queues.orchestration_namespace`

Namespace prefix for orchestration queue names

- **Type:** `String`
- **Default:** `"orchestration"`
- **Valid Range:** non-empty string
- **System Impact:** Used in queue naming pattern to isolate orchestration queues from worker queues

#### `common.queues.worker_namespace`

Namespace prefix for worker queue names

- **Type:** `String`
- **Default:** `"worker"`
- **Valid Range:** non-empty string
- **System Impact:** Used in queue naming pattern to isolate worker queues from orchestration queues

### orchestration_queues

**Path:** `common.queues.orchestration_queues`

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `step_results` | `String` | `"orchestration_step_results"` | Queue name for step execution results returned by workers |
| `task_finalizations` | `String` | `"orchestration_task_finalizations"` | Queue name for task finalization messages |
| `task_requests` | `String` | `"orchestration_task_requests"` | Queue name for incoming task execution requests |

#### `common.queues.orchestration_queues.step_results`

Queue name for step execution results returned by workers

- **Type:** `String`
- **Default:** `"orchestration_step_results"`
- **Valid Range:** valid queue name
- **System Impact:** Workers publish step completion results here for the orchestration result processor

#### `common.queues.orchestration_queues.task_finalizations`

Queue name for task finalization messages

- **Type:** `String`
- **Default:** `"orchestration_task_finalizations"`
- **Valid Range:** valid queue name
- **System Impact:** Tasks ready for completion evaluation are enqueued here

#### `common.queues.orchestration_queues.task_requests`

Queue name for incoming task execution requests

- **Type:** `String`
- **Default:** `"orchestration_task_requests"`
- **Valid Range:** valid queue name
- **System Impact:** The orchestration system reads new task requests from this queue

### pgmq

**Path:** `common.queues.pgmq`

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `poll_interval_ms` | `u32` | `500` | Interval in milliseconds between PGMQ polling cycles when no LISTEN/NOTIFY events arrive |

#### `common.queues.pgmq.poll_interval_ms`

Interval in milliseconds between PGMQ polling cycles when no LISTEN/NOTIFY events arrive

- **Type:** `u32`
- **Default:** `500`
- **Valid Range:** 10-10000
- **System Impact:** Lower values reduce message latency in polling mode but increase database load; in Hybrid mode this is the fallback interval

#### queue_depth_thresholds

**Path:** `common.queues.pgmq.queue_depth_thresholds`

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `critical_threshold` | `i64` | `5000` | Queue depth at which the API returns HTTP 503 Service Unavailable for new task submissions |
| `overflow_threshold` | `i64` | `10000` | Queue depth indicating an emergency condition requiring manual intervention |

#### `common.queues.pgmq.queue_depth_thresholds.critical_threshold`

Queue depth at which the API returns HTTP 503 Service Unavailable for new task submissions

- **Type:** `i64`
- **Default:** `5000`
- **Valid Range:** 1+
- **System Impact:** Backpressure mechanism: rejects new work to allow the system to drain existing messages

#### `common.queues.pgmq.queue_depth_thresholds.overflow_threshold`

Queue depth indicating an emergency condition requiring manual intervention

- **Type:** `i64`
- **Default:** `10000`
- **Valid Range:** 1+
- **System Impact:** Highest severity threshold; triggers error-level logging and metrics for operational alerting

### rabbitmq

**Path:** `common.queues.rabbitmq`

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `heartbeat_seconds` | `u16` | `30` | AMQP heartbeat interval for connection liveness detection |
| `prefetch_count` | `u16` | `100` | Number of unacknowledged messages RabbitMQ will deliver before waiting for acks |
| `url` | `String` | `"${RABBITMQ_URL:-amqp://guest:guest@localhost:5672/%2F}"` | AMQP connection URL for RabbitMQ; %2F is the URL-encoded default vhost '/' |

#### `common.queues.rabbitmq.heartbeat_seconds`

AMQP heartbeat interval for connection liveness detection

- **Type:** `u16`
- **Default:** `30`
- **Valid Range:** 0-3600
- **System Impact:** Detects dead connections; 0 disables heartbeats (not recommended in production)

#### `common.queues.rabbitmq.prefetch_count`

Number of unacknowledged messages RabbitMQ will deliver before waiting for acks

- **Type:** `u16`
- **Default:** `100`
- **Valid Range:** 1-65535
- **System Impact:** Controls consumer throughput vs. memory usage; higher values increase throughput but buffer more messages in-process

#### `common.queues.rabbitmq.url`

AMQP connection URL for RabbitMQ; %2F is the URL-encoded default vhost '/'

- **Type:** `String`
- **Default:** `"${RABBITMQ_URL:-amqp://guest:guest@localhost:5672/%2F}"`
- **Valid Range:** valid AMQP URI
- **System Impact:** Only used when queues.backend = 'rabbitmq'; must be reachable at startup

## system

**Path:** `common.system`

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `default_dependent_system` | `String` | `"default"` | Default system name assigned to tasks that do not specify a dependent system |

#### `common.system.default_dependent_system`

Default system name assigned to tasks that do not specify a dependent system

- **Type:** `String`
- **Default:** `"default"`
- **Valid Range:** non-empty string
- **System Impact:** Groups tasks for routing and reporting; most single-system deployments can leave this as default

## task_templates

**Path:** `common.task_templates`

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| `search_paths` | `Vec<String>` | `["config/tasks/**/*.{yml,yaml}"]` | Glob patterns for discovering task template YAML files |

#### `common.task_templates.search_paths`

Glob patterns for discovering task template YAML files

- **Type:** `Vec<String>`
- **Default:** `["config/tasks/**/*.{yml,yaml}"]`
- **Valid Range:** valid glob patterns
- **System Impact:** Templates matching these patterns are loaded at startup for task definition discovery

---

*Generated by `tasker-ctl docs` — [Tasker Configuration System](https://github.com/tasker-systems/tasker-core)*
