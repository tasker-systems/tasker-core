"""Conftest for client integration tests.

Auto-skips tests marked with @pytest.mark.client_integration
when FFI_CLIENT_TESTS environment variable is not set to 'true'.
"""

import os

import pytest


def pytest_collection_modifyitems(config, items):  # noqa: ARG001
    """Skip client_integration tests when FFI_CLIENT_TESTS is not set."""
    if os.environ.get("FFI_CLIENT_TESTS") != "true":
        skip = pytest.mark.skip(
            reason="Set FFI_CLIENT_TESTS=true with orchestration server running"
        )
        for item in items:
            if "client_integration" in item.keywords:
                item.add_marker(skip)
