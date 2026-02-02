# Skill: Python Development

## When to Use

Use this skill when writing, reviewing, or modifying Python code in `workers/python/`, including step handlers, PyO3/maturin FFI bindings, pytest tests, or type annotations.

## Tooling

| Tool | Purpose | Command |
|------|---------|---------|
| ruff | Linting & formatting | `cargo make check-python` |
| mypy | Type checking | Part of `check-python` |
| pytest | Testing | `cargo make test-python` |
| maturin | Rust-Python FFI builds (PyO3) | `uv run maturin develop` |
| uv | Package management | `uv sync` |

### Setup

```bash
cd workers/python
cargo make setup          # Creates .venv, syncs dependencies
uv run maturin develop    # Build Rust extension in dev mode
```

### Quality Checks

```bash
cargo make check-python     # format-check + lint + typecheck + test
cargo make fix-python       # format-fix + lint-fix
cargo make test-python      # pytest
cargo make test-coverage    # pytest with coverage
```

## Code Style

### Formatting & Naming

- PEP 8 with project-specific ruff settings in `pyproject.toml`
- Classes: `PascalCase`; functions/methods: `snake_case`; constants: `SCREAMING_SNAKE_CASE`
- Private: leading underscore `_internal_helper()`
- Always use `from __future__ import annotations` for modern type syntax

### Module Organization

```python
"""Order processing step handler."""

from __future__ import annotations

# 1. Standard library
from typing import Any

# 2. Third-party
import httpx

# 3. Local
from tasker_core.step_handler import StepHandler
from tasker_core.step_handler.mixins import APIMixin
from tasker_core.types import StepContext, StepHandlerResult

# 4. Constants
DEFAULT_API_TIMEOUT = 30

# 5. Classes
class OrderHandler(StepHandler, APIMixin):
    """Handles order processing operations."""
    def call(self, context: StepContext) -> StepHandlerResult:
        pass
```

## Type Hints

### Always Use Type Hints

```python
from typing import Any, Optional

def call(self, context: StepContext) -> StepHandlerResult:
    order_id: str = context.input_data.get("order_id", "")
    config: dict[str, Any] = context.step_config
    result: Optional[dict[str, Any]] = self._process_order(order_id)
```

### Type Aliases for Complex Types

```python
from typing import TypeAlias

OrderData: TypeAlias = dict[str, Any]
BatchResult: TypeAlias = list[dict[str, Any]]
```

## Handler Pattern

### The `call(context)` Contract

Every handler inherits from `StepHandler` and implements `call`:

```python
class OrderHandler(StepHandler, APIMixin):
    """Processes order operations via external API."""

    def call(self, context: StepContext) -> StepHandlerResult:
        try:
            order_id = context.input_data["order_id"]
            response = self.get(f"/api/orders/{order_id}")

            if response.status_code == 200:
                return self.success(
                    result=response.json(),
                    metadata={"fetched_at": datetime.now().isoformat()},
                )
            return self.failure(
                message=f"API error: {response.status_code}",
                error_type="APIError",
                retryable=response.status_code >= 500,
            )
        except KeyError as e:
            return self.failure(
                message=f"Missing required field: {e}",
                error_type="ValidationError",
                retryable=False,
            )
```

### Result Factory Methods

```python
# Success
self.success(result={"order_id": "123"}, metadata={"duration_ms": 150})

# Failure
self.failure(message="Validation failed", error_type="ValidationError",
             error_code="INVALID_QUANTITY", retryable=False)

# Decision
self.decision_success(steps=["ship_order", "send_confirmation"],
                      routing_context={"decision": "standard_flow"})

# Skip branches
self.skip_branches(reason="No items require processing",
                   routing_context={"skip_reason": "empty_cart"})
```

## Composition Over Inheritance (TAS-112)

Python uses multiple inheritance (mixins) for handler capabilities:

```python
# WRONG: Single specialized base
class MyHandler(APIHandler):  # Don't do this

# RIGHT: Composition via mixins
class MyHandler(StepHandler, APIMixin, DecisionMixin):
    pass
```

| Mixin | Provides |
|-------|----------|
| `APIMixin` | `get`, `post`, `put`, `delete` |
| `DecisionMixin` | `decision_success`, `skip_branches`, `decision_failure` |
| `BatchableMixin` | `get_batch_context`, `batch_worker_complete`, `handle_no_op_worker` |

## Error Handling

### Specific Exceptions First

```python
try:
    self._validate_input(context.input_data)
    result = self._process(context)
    return self.success(result=result)
except ValidationError as e:
    return self.failure(str(e), error_type="ValidationError", retryable=False)
except httpx.TimeoutException as e:
    return self.failure(str(e), error_type="TimeoutError", retryable=True)
except Exception as e:
    return self.failure(str(e), error_type="UnexpectedError", retryable=True)
```

## FFI Considerations

```python
# Native extension loaded automatically
from tasker_core._native import some_ffi_function

# Types converted automatically:
# Python dict <-> Rust HashMap
# Python list <-> Rust Vec
# Python str  <-> Rust String

# Minimize FFI calls -- extract data, process in Python
def process(context: StepContext) -> StepHandlerResult:
    order_id = context.input_data["order_id"]
    result = self._python_processing(order_id)
    return self.success(result=result)
```

- Python has GIL -- same concurrency constraints as Ruby
- Pull-based dispatch model via `FfiDispatchChannel`

## Testing (pytest)

```python
class TestOrderHandler:
    @pytest.fixture
    def handler(self) -> OrderHandler:
        return OrderHandler()

    @pytest.fixture
    def context(self) -> StepContext:
        return StepContext(
            task_uuid="test-task-uuid",
            step_uuid="test-step-uuid",
            input_data={"order_id": "12345"},
            step_config={},
        )

    def test_call_with_valid_order_returns_success(self, handler, context):
        with patch.object(handler, "get") as mock_get:
            mock_get.return_value = Mock(status_code=200, json=lambda: {"id": "12345"})
            result = handler.call(context)
            assert result.success
            assert result.result["id"] == "12345"

    @pytest.mark.parametrize("status_code,expected_retryable", [
        (400, False), (404, False), (500, True), (502, True),
    ])
    def test_call_handles_api_errors(self, handler, context, status_code, expected_retryable):
        with patch.object(handler, "get") as mock_get:
            mock_get.return_value = Mock(status_code=status_code)
            result = handler.call(context)
            assert not result.success
            assert result.retryable == expected_retryable
```

## Documentation (Google Style Docstrings)

```python
class OrderHandler(StepHandler):
    """Handles order processing operations.

    Attributes:
        api_base_url: Base URL for the order API.
        timeout: Request timeout in seconds.

    Example:
        >>> handler = OrderHandler()
        >>> result = handler.call(context)
    """

    def call(self, context: StepContext) -> StepHandlerResult:
        """Process an order step.

        Args:
            context: Execution context containing order data and config.

        Returns:
            StepHandlerResult with order processing outcome.

        Raises:
            ValidationError: If order_id is missing or invalid.
        """
```

## References

- Best practices: `docs/development/best-practices-python.md`
- Composition: `docs/principles/composition-over-inheritance.md`
- Cross-language: `docs/principles/cross-language-consistency.md`
- FFI safety: `docs/development/ffi-callback-safety.md`
