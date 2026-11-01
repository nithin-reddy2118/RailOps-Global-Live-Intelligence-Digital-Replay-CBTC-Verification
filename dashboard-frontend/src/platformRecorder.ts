export type PlatformEventPayload = Record<string, unknown>;

const platformSessionId = crypto.randomUUID();

export async function recordPlatformEvent(
  eventType: string,
  payload: PlatformEventPayload = {},
): Promise<void> {
  try {
    await fetch("/dashboard/platform/event", {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
      },
      body: JSON.stringify({
        event_type: eventType,
        origin_service: "dashboard-frontend",
        payload: {
          session_id: platformSessionId,
          ...payload,
        },
      }),
      keepalive: true,
    });
  } catch {
    // Recording must never break the dashboard itself.
  }
}

export function installPlatformErrorRecorder(): void {
  window.addEventListener("error", (event) => {
    void recordPlatformEvent("platform.frontend.error", {
      message: event.message,
      source: event.filename || null,
      line: event.lineno || null,
      column: event.colno || null,
    });
  });

  window.addEventListener("unhandledrejection", (event) => {
    void recordPlatformEvent("platform.frontend.unhandled_rejection", {
      reason: String(event.reason ?? "unknown"),
    });
  });
}

export function recordPlatformAction(
  action: string,
  payload: PlatformEventPayload = {},
): void {
  void recordPlatformEvent("platform.frontend.action", {
    action,
    ...payload,
  });
}

export function installPlatformFetchRecorder(): void {
  const originalFetch = window.fetch.bind(window);

  window.fetch = async (
    input: RequestInfo | URL,
    init?: RequestInit,
  ): Promise<Response> => {
    const started = performance.now();

    const url =
      typeof input === "string"
        ? input
        : input instanceof URL
          ? input.toString()
          : input.url;

    const method =
      init?.method ??
      (input instanceof Request ? input.method : "GET");

    try {
      const response = await originalFetch(input, init);

      void recordPlatformEvent("platform.frontend.network", {
        method,
        url: new URL(url, window.location.origin).pathname,
        status: response.status,
        ok: response.ok,
        duration_ms: Math.round(performance.now() - started),
      });

      return response;
    } catch (error) {
      void recordPlatformEvent("platform.frontend.network_error", {
        method,
        url: new URL(url, window.location.origin).pathname,
        duration_ms: Math.round(performance.now() - started),
        error: String(error),
      });

      throw error;
    }
  };
}

export function installPlatformLifecycleRecorder(): void {
  void recordPlatformEvent("platform.frontend.lifecycle", {
    event: "startup",
    path: window.location.pathname,
    visibility: document.visibilityState,
    online: navigator.onLine,
  });

  document.addEventListener("visibilitychange", () => {
    void recordPlatformEvent("platform.frontend.lifecycle", {
      event: "visibilitychange",
      visibility: document.visibilityState,
    });
  });

  window.addEventListener("online", () => {
    void recordPlatformEvent("platform.frontend.lifecycle", {
      event: "online",
    });
  });

  window.addEventListener("offline", () => {
    void recordPlatformEvent("platform.frontend.lifecycle", {
      event: "offline",
    });
  });

  window.addEventListener("pagehide", () => {
    void recordPlatformEvent("platform.frontend.lifecycle", {
      event: "pagehide",
    });
  });
}

export function installPlatformNavigationRecorder(): void {
  const record = () => {
    void recordPlatformEvent("platform.frontend.navigation", {
      path: window.location.pathname,
      search: window.location.search,
      hash: window.location.hash,
    });
  };

  window.addEventListener("popstate", record);
  window.addEventListener("hashchange", record);

  record();
}

export function recordReactError(
  error: unknown,
  componentStack?: string,
): void {
  void recordPlatformEvent("platform.frontend.react_error", {
    error: String(error),
    component_stack: componentStack ?? null,
  });
}
