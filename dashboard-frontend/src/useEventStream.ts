import { useEffect, useRef, useState } from "react";
import type { EventEnvelope } from "./types";

const WS_URL = import.meta.env.VITE_WS_URL ?? "ws://localhost:8080/dashboard/ws";
const MAX_LOG_ENTRIES = 200;
const RECONNECT_DELAY_MS = 2000;

export function useEventStream() {
  const [events, setEvents] = useState<EventEnvelope[]>([]);
  const [connected, setConnected] = useState(false);
  const socketRef = useRef<WebSocket | null>(null);

  useEffect(() => {
    let cancelled = false;

    function connect() {
      if (cancelled) return;

      const socket = new WebSocket(WS_URL);
      socketRef.current = socket;

      socket.onopen = () => setConnected(true);

      socket.onmessage = (message) => {
        try {
          const envelope: EventEnvelope = JSON.parse(message.data);
          setEvents((prev) => [envelope, ...prev].slice(0, MAX_LOG_ENTRIES));
        } catch {
          // Malformed frame — drop it rather than crash the dashboard;
          // the next well-formed event will arrive momentarily.
        }
      };

      socket.onclose = () => {
        setConnected(false);
        if (!cancelled) {
          setTimeout(connect, RECONNECT_DELAY_MS);
        }
      };

      socket.onerror = () => socket.close();
    }

    connect();

    return () => {
      cancelled = true;
      socketRef.current?.close();
    };
  }, []);

  return { events, connected };
}
