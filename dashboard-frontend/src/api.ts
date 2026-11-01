import type { GlobalTrainOverview, NetworkOverview, ReplayEvent, ReplayRun, StationOverview, VerificationResult } from "./types";

const API_BASE_URL = import.meta.env.VITE_API_BASE_URL ?? "http://localhost:8080";

async function request<T>(path: string, init: RequestInit = {}, token?: string): Promise<T> {
  const headers = new Headers(init.headers);
  if (init.body && !headers.has("content-type")) headers.set("content-type", "application/json");
  if (token) headers.set("authorization", `Bearer ${token}`);
  const response = await fetch(`${API_BASE_URL}${path}`, { ...init, headers });
  if (!response.ok) {
    let message = `request failed: ${response.status}`;
    try {
      const body = await response.json();
      message = body.message ?? message;
    } catch { /* keep status message */ }
    const error = new Error(message) as Error & { status?: number };
    error.status = response.status;
    throw error;
  }
  return response.json();
}

export function fetchSnapshot(): Promise<NetworkOverview> {
  return request<NetworkOverview>("/dashboard/snapshot");
}

export function fetchStations(): Promise<StationOverview[]> {
  return request<StationOverview[]>("/dashboard/stations");
}

export function fetchGlobalTrains(): Promise<GlobalTrainOverview[]> {
  return request<GlobalTrainOverview[]>("/dashboard/trains");
}

export async function login(username: string, password: string): Promise<string> {
  const result = await request<{ access_token: string }>("/auth/login", {
    method: "POST",
    body: JSON.stringify({ username, password }),
  });
  return result.access_token;
}

export function fetchReplayRuns(token: string): Promise<ReplayRun[]> {
  return request<ReplayRun[]>("/dashboard/replay/runs", {}, token);
}

export function startReplayRun(label: string, token: string): Promise<ReplayRun> {
  return request<ReplayRun>("/dashboard/replay/runs", {
    method: "POST",
    body: JSON.stringify({ label }),
  }, token);
}

export function stopReplayRun(runId: string, token: string): Promise<ReplayRun> {
  return request<ReplayRun>(`/dashboard/replay/runs/${runId}/stop`, { method: "POST" }, token);
}

export function fetchReplayEvents(runId: string, token: string): Promise<ReplayEvent[]> {
  return request<ReplayEvent[]>(`/dashboard/replay/runs/${runId}/events?limit=1000`, {}, token);
}

export function verifyReplayRun(runId: string, token: string): Promise<VerificationResult> {
  return request<VerificationResult>(`/dashboard/replay/runs/${runId}/verify`, { method: "POST" }, token);
}




