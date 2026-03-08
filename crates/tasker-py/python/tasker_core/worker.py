"""High-level worker lifecycle manager.

Wraps bootstrap_worker() + event processing pipeline, matching
Ruby Bootstrap.start! and TypeScript WorkerServer.start().

Example:
    >>> from tasker_core import Worker
    >>>
    >>> # Start with handler discovery
    >>> worker = Worker.start(handler_packages=["app.handlers"])
    >>>
    >>> # Shutdown
    >>> worker.stop()
    >>>
    >>> # Or use as context manager
    >>> with Worker.start(handler_packages=["app.handlers"]) as worker:
    ...     pass  # worker stops on exit
    >>>
    >>> # Calling start() again returns the existing running instance
    >>> w1 = Worker.start()
    >>> w2 = Worker.start()
    >>> assert w1 is w2
"""

from __future__ import annotations

from typing import TYPE_CHECKING

from .bootstrap import bootstrap_worker, stop_worker
from .event_bridge import EventBridge, EventNames
from .event_poller import EventPoller
from .handler import HandlerRegistry
from .logging import log_info, log_warn
from .step_execution_subscriber import StepExecutionSubscriber

if TYPE_CHECKING:
    from .types import BootstrapConfig, BootstrapResult


class Worker:
    """High-level worker lifecycle manager (singleton).

    Wraps bootstrap_worker() + event processing pipeline,
    matching Ruby Bootstrap.start! and TypeScript WorkerServer.start().

    Only one Worker can be active at a time — the Rust FFI layer
    enforces a single worker process. Calling start() when a worker
    is already running returns the existing instance. Calling stop()
    clears the singleton so a fresh start() can be issued later.

    Example:
        >>> worker = Worker.start(handler_packages=["app.handlers"])
        >>> print(f"Worker running: {worker.is_running}")
        >>> worker.stop()

        >>> # Or as context manager
        >>> with Worker.start(handler_packages=["app.handlers"]) as w:
        ...     print(f"Worker {w.worker_id} running")
    """

    _instance: Worker | None = None

    def __init__(self) -> None:
        self._bootstrap_result: BootstrapResult | None = None
        self._poller: EventPoller | None = None
        self._subscriber: StepExecutionSubscriber | None = None
        self._bridge: EventBridge | None = None
        self._running = False

    @classmethod
    def start(
        cls,
        handler_packages: list[str] | None = None,
        config: BootstrapConfig | None = None,
    ) -> Worker:
        """Bootstrap the worker and start the event processing pipeline.

        If a worker is already running, returns the existing instance
        (handler_packages and config are ignored in that case).

        Handler discovery always starts with template-based bootstrap
        (matching Ruby/Python shared pattern). If handler_packages are
        provided, those packages are scanned *after* template discovery
        so explicit registrations take priority on name collisions.

        Args:
            handler_packages: Additional Python packages to scan for
                StepHandler subclasses. Scanned after template-based
                discovery; explicit registrations override templates.
            config: Optional bootstrap configuration for the Rust FFI layer.

        Returns:
            The running Worker singleton.

        Raises:
            WorkerBootstrapError: If FFI bootstrap fails.
        """
        if cls._instance is not None and cls._instance._running:
            log_warn(
                "Worker.start() called but worker is already running; returning existing instance"
            )
            return cls._instance

        worker = cls()
        worker._start(handler_packages=handler_packages, config=config)
        cls._instance = worker
        return worker

    def stop(self) -> None:
        """Stop event processing and the Rust worker (reverse order).

        Clears the singleton so a subsequent start() creates a fresh
        worker. Safe to call multiple times.
        """
        if not self._running:
            return

        self._running = False

        # Reverse order of start
        if self._poller is not None:
            self._poller.stop()

        if self._subscriber is not None:
            self._subscriber.stop()

        if self._bridge is not None:
            self._bridge.stop()

        stop_worker()
        Worker._instance = None
        log_info("Worker stopped")

    @classmethod
    def instance(cls) -> Worker | None:
        """Return the current Worker singleton, or None if not running."""
        if cls._instance is not None and cls._instance._running:
            return cls._instance
        return None

    @classmethod
    def reset_instance(cls) -> None:
        """Reset the singleton (for testing). Stops the worker if running."""
        if cls._instance is not None:
            cls._instance.stop()
        cls._instance = None

    @property
    def worker_id(self) -> str:
        """The worker ID assigned during bootstrap."""
        if self._bootstrap_result is None:
            return ""
        return self._bootstrap_result.worker_id

    @property
    def is_running(self) -> bool:
        """Whether the worker is currently running."""
        return self._running

    @property
    def bootstrap_result(self) -> BootstrapResult:
        """The bootstrap result from FFI initialization.

        Raises:
            RuntimeError: If the worker has not been started.
        """
        if self._bootstrap_result is None:
            raise RuntimeError("Worker has not been started")
        return self._bootstrap_result

    def __enter__(self) -> Worker:
        return self

    def __exit__(self, *exc: object) -> None:
        self.stop()

    # ------------------------------------------------------------------
    # Internal
    # ------------------------------------------------------------------

    def _start(
        self,
        handler_packages: list[str] | None = None,
        config: BootstrapConfig | None = None,
    ) -> None:
        # 1. Bootstrap Rust FFI layer
        self._bootstrap_result = bootstrap_worker(config)

        # 2. Handler discovery — always run template-based bootstrap first,
        #    then layer on explicit packages (which override on name collision)
        registry = HandlerRegistry.instance(skip_bootstrap=True)
        registry.bootstrap_handlers()
        if handler_packages:
            for package in handler_packages:
                registry.discover_handlers(package)

        # 3. Start event bridge
        bridge = EventBridge.instance()
        bridge.start()
        self._bridge = bridge

        # 4. Start step execution subscriber
        subscriber = StepExecutionSubscriber(
            event_bridge=bridge,
            handler_registry=registry,
            worker_id=self._bootstrap_result.worker_id,
        )
        subscriber.start()
        self._subscriber = subscriber

        # 5. Start event poller with bridge forwarding
        poller = EventPoller()
        poller.on_step_event(
            lambda event: bridge.publish(EventNames.STEP_EXECUTION_RECEIVED, event)
        )
        poller.start()
        self._poller = poller

        self._running = True
        log_info(
            f"Worker {self._bootstrap_result.worker_id} started",
            {"worker_id": self._bootstrap_result.worker_id},
        )


__all__ = ["Worker"]
