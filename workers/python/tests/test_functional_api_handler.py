"""Tests for @api_handler functional decorator.

Tests:
1. Handler class composition (APIMixin + StepHandler)
2. HTTP methods via mocked httpx client (get, post, put, delete)
3. api_success / api_failure result helpers
4. Error classification (retryable vs permanent by status code)
5. Configuration passthrough (base_url, timeout, headers)
6. Async api_handler support
"""

from __future__ import annotations

import asyncio
from typing import Any, cast
from uuid import uuid4

import httpx

from tasker_core.step_handler.functional import api_handler
from tasker_core.step_handler.mixins.api import APIMixin
from tasker_core.types import FfiStepEvent, StepContext, StepHandlerResult

# ============================================================================
# Test Helpers
# ============================================================================


def _call_sync(handler: Any, ctx: StepContext) -> StepHandlerResult:
    """Call a sync handler and cast the result."""
    return cast(StepHandlerResult, handler.call(ctx))


def _make_context(
    handler_name: str = "test_api",
    input_data: dict | None = None,
) -> StepContext:
    """Create a StepContext for testing."""
    task_sequence_step = {
        "task": {"task": {"context": input_data or {}}},
        "dependency_results": {},
        "step_definition": {"handler": {"initialization": {}}},
        "workflow_step": {"attempts": 0, "max_attempts": 3, "inputs": {}},
    }

    event = FfiStepEvent(
        event_id=str(uuid4()),
        task_uuid=str(uuid4()),
        step_uuid=str(uuid4()),
        correlation_id=str(uuid4()),
        task_sequence_step=task_sequence_step,
    )

    return StepContext.from_ffi_event(event, handler_name)


def _api_handler_instance(decorated_fn: Any) -> APIMixin:
    """Instantiate the handler class from a decorated function, typed as APIMixin."""
    return cast(APIMixin, decorated_fn._handler_class())


# ============================================================================
# Tests: Handler Composition
# ============================================================================


class TestApiHandlerComposition:
    """Tests that @api_handler produces a properly composed class."""

    def test_handler_class_inherits_api_mixin(self):
        """Generated class includes APIMixin."""

        @api_handler("fetch_data", base_url="https://api.example.com")
        def fetch_data(api, context):  # noqa: ARG001
            pass

        handler_cls = fetch_data._handler_class
        assert issubclass(handler_cls, APIMixin)

    def test_handler_name_and_version(self):
        """Handler name and version are set correctly."""

        @api_handler("fetch_data", base_url="https://api.example.com", version="2.0.0")
        def fetch_data(api, context):  # noqa: ARG001
            pass

        handler = fetch_data._handler_class()
        assert handler.handler_name == "fetch_data"
        assert handler.handler_version == "2.0.0"

    def test_base_url_configured(self):
        """base_url is set on the generated class."""

        @api_handler("fetch_data", base_url="https://api.example.com")
        def fetch_data(api, context):  # noqa: ARG001
            pass

        handler_cls = fetch_data._handler_class
        assert handler_cls.base_url == "https://api.example.com"  # type: ignore[attr-defined]

    def test_timeout_configured(self):
        """Custom timeout is set on the generated class."""

        @api_handler("fetch_data", base_url="https://api.example.com", timeout=60.0)
        def fetch_data(api, context):  # noqa: ARG001
            pass

        handler_cls = fetch_data._handler_class
        assert handler_cls.default_timeout == 60.0  # type: ignore[attr-defined]

    def test_default_headers_configured(self):
        """Custom headers are set on the generated class."""

        @api_handler(
            "fetch_data",
            base_url="https://api.example.com",
            default_headers={"Authorization": "Bearer token123"},
        )
        def fetch_data(api, context):  # noqa: ARG001
            pass

        handler_cls = fetch_data._handler_class
        assert handler_cls.default_headers == {"Authorization": "Bearer token123"}  # type: ignore[attr-defined]


# ============================================================================
# Tests: HTTP Methods
# ============================================================================


class TestApiHandlerHttpMethods:
    """Tests that HTTP methods work through the api parameter."""

    @staticmethod
    def _mock_transport(status_code: int = 200, json_body: dict | None = None):
        """Create httpx.MockTransport returning a fixed response."""

        def handler(_request: httpx.Request):
            return httpx.Response(status_code=status_code, json=json_body or {})

        return httpx.MockTransport(handler)

    def test_get_success(self):
        """GET request returns success result via api.get()."""

        @api_handler("fetch_user", base_url="https://api.example.com")
        def fetch_user(api, context):  # noqa: ARG001
            response = api.get("/users/1")
            if response.ok:
                return api.api_success(response)
            return api.api_failure(response)

        handler = _api_handler_instance(fetch_user)
        handler._client = httpx.Client(
            base_url="https://api.example.com",
            transport=self._mock_transport(200, {"id": 1, "name": "Alice"}),
        )

        result = _call_sync(handler, _make_context())

        assert result.is_success is True
        assert result.result["id"] == 1  # type: ignore[index]
        assert result.result["name"] == "Alice"  # type: ignore[index]
        handler.close()

    def test_post_success(self):
        """POST request returns success result via api.post()."""

        @api_handler("create_user", base_url="https://api.example.com")
        def create_user(api, context):  # noqa: ARG001
            response = api.post("/users", json={"name": "Bob"})
            if response.ok:
                return api.api_success(response)
            return api.api_failure(response)

        handler = _api_handler_instance(create_user)
        handler._client = httpx.Client(
            base_url="https://api.example.com",
            transport=self._mock_transport(201, {"id": 42, "created": True}),
        )

        result = _call_sync(handler, _make_context())

        assert result.is_success is True
        assert result.result["id"] == 42  # type: ignore[index]
        handler.close()

    def test_delete_success(self):
        """DELETE request returns success result via api.delete()."""

        @api_handler("remove_user", base_url="https://api.example.com")
        def remove_user(api, context):  # noqa: ARG001
            response = api.delete("/users/1")
            if response.ok:
                return {"deleted": True}
            return api.api_failure(response)

        handler = _api_handler_instance(remove_user)
        handler._client = httpx.Client(
            base_url="https://api.example.com",
            transport=self._mock_transport(204),
        )

        result = _call_sync(handler, _make_context())

        assert result.is_success is True
        assert result.result["deleted"] is True  # type: ignore[index]
        handler.close()


# ============================================================================
# Tests: Error Classification
# ============================================================================


class TestApiHandlerErrorClassification:
    """Tests that api_failure classifies errors correctly."""

    @staticmethod
    def _mock_transport(
        status_code: int, json_body: dict | None = None, headers: dict[str, str] | None = None
    ):
        """Create httpx.MockTransport returning a fixed response."""
        resp_headers = headers or {}

        def handler(_request: httpx.Request):
            return httpx.Response(
                status_code=status_code,
                json=json_body or {},
                headers=resp_headers,
            )

        return httpx.MockTransport(handler)

    def test_404_is_not_retryable(self):
        """404 Not Found produces a non-retryable failure."""

        @api_handler("fetch_missing", base_url="https://api.example.com")
        def fetch_missing(api, context):  # noqa: ARG001
            response = api.get("/users/999")
            return api.api_failure(response)

        handler = _api_handler_instance(fetch_missing)
        handler._client = httpx.Client(
            base_url="https://api.example.com",
            transport=self._mock_transport(404, {"error": "not found"}),
        )

        result = _call_sync(handler, _make_context())

        assert result.is_success is False
        assert result.retryable is False
        handler.close()

    def test_503_is_retryable(self):
        """503 Service Unavailable produces a retryable failure."""

        @api_handler("fetch_unavailable", base_url="https://api.example.com")
        def fetch_unavailable(api, context):  # noqa: ARG001
            response = api.get("/health")
            return api.api_failure(response)

        handler = _api_handler_instance(fetch_unavailable)
        handler._client = httpx.Client(
            base_url="https://api.example.com",
            transport=self._mock_transport(503, {"error": "service down"}),
        )

        result = _call_sync(handler, _make_context())

        assert result.is_success is False
        assert result.retryable is True
        handler.close()

    def test_429_is_retryable_with_retry_after(self):
        """429 Too Many Requests produces a retryable failure with retry_after metadata."""

        @api_handler("fetch_throttled", base_url="https://api.example.com")
        def fetch_throttled(api, context):  # noqa: ARG001
            response = api.get("/data")
            return api.api_failure(response)

        handler = _api_handler_instance(fetch_throttled)
        handler._client = httpx.Client(
            base_url="https://api.example.com",
            transport=self._mock_transport(
                429,
                {"error": "rate limited"},
                {"retry-after": "30"},
            ),
        )

        result = _call_sync(handler, _make_context())

        assert result.is_success is False
        assert result.retryable is True
        assert result.metadata.get("retry_after_seconds") == 30
        handler.close()


# ============================================================================
# Tests: api parameter is self
# ============================================================================


class TestApiParameterIsSelf:
    """Tests that api is the handler instance itself (Python pattern)."""

    def test_api_is_handler_instance(self):
        """api parameter is the handler instance."""
        captured_api = None

        @api_handler("check_self", base_url="https://api.example.com")
        def check_self(api, context):  # noqa: ARG001
            nonlocal captured_api
            captured_api = api
            return {"ok": True}

        handler = check_self._handler_class()
        _call_sync(handler, _make_context())

        assert captured_api is handler

    def test_api_has_http_methods(self):
        """api object exposes get, post, put, patch, delete methods."""

        @api_handler("check_methods", base_url="https://api.example.com")
        def check_methods(api, context):  # noqa: ARG001
            return {"ok": True}

        handler = check_methods._handler_class()
        assert callable(getattr(handler, "get", None))
        assert callable(getattr(handler, "post", None))
        assert callable(getattr(handler, "put", None))
        assert callable(getattr(handler, "patch", None))
        assert callable(getattr(handler, "delete", None))

    def test_api_has_result_helpers(self):
        """api object exposes api_success and api_failure methods."""

        @api_handler("check_helpers", base_url="https://api.example.com")
        def check_helpers(api, context):  # noqa: ARG001
            return {"ok": True}

        handler = check_helpers._handler_class()
        assert callable(getattr(handler, "api_success", None))
        assert callable(getattr(handler, "api_failure", None))


# ============================================================================
# Tests: Async api_handler
# ============================================================================


class TestAsyncApiHandler:
    """Tests that async @api_handler works correctly."""

    def test_async_get_success(self):
        """Async api_handler with GET request."""

        def _mock_transport(_request: httpx.Request):
            return httpx.Response(status_code=200, json={"async": True})

        @api_handler("async_fetch", base_url="https://api.example.com")
        async def async_fetch(api, context):  # noqa: ARG001
            response = api.get("/data")
            if response.ok:
                return api.api_success(response)
            return api.api_failure(response)

        handler = _api_handler_instance(async_fetch)
        handler._client = httpx.Client(
            base_url="https://api.example.com",
            transport=httpx.MockTransport(_mock_transport),
        )

        coro_result = handler.call(_make_context())  # type: ignore[attr-defined]
        result = asyncio.run(coro_result)

        assert result.is_success is True
        assert result.result["async"] is True
        handler.close()
