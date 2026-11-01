import { useEffect, useMemo, useState } from "react";
import { fetchReplayEvents, fetchReplayRuns, fetchSnapshot, login, verifyReplayRun } from "./api";
import { applyEvent } from "./eventReducer";
import { AlertsPanel, AuthorityPanel, EventLogPanel, ServicePanel, SignalsPanel, SwitchesPanel, TrainsPanel } from "./Panels";
import { TrackMap } from "./TrackMap";
import { GlobalMap } from "./GlobalMap";
import type { NetworkOverview, ReplayEvent, ReplayRun, VerificationResult } from "./types";
import { useEventStream } from "./useEventStream";

const SNAPSHOT_REFRESH_MS = 15000;
type View = "dashboard" | "world-map" | "track-map" | "trains" | "signals" | "switches" | "authorities" | "events" | "alerts" | "replay";

export default function App() {
  const [view, setView] = useState<View>("dashboard");
  const [overview, setOverview] = useState<NetworkOverview | null>(null);
  const [error, setError] = useState<string | null>(null);
  const { events, connected } = useEventStream();

  useEffect(() => {
    let cancelled = false;
    async function load() { try { const snapshot = await fetchSnapshot(); if (!cancelled) { setOverview(snapshot); setError(null); } } catch (e) { if (!cancelled) setError(e instanceof Error ? e.message : "failed to load snapshot"); } }
    load(); const interval = setInterval(load, SNAPSHOT_REFRESH_MS); return () => { cancelled = true; clearInterval(interval); };
  }, []);

  useEffect(() => { if (events.length) setOverview(prev => prev ? applyEvent(prev, events[0]) : prev); }, [events]);

  const activeTrains = useMemo(() => overview?.trains.filter(t => t.operational_state !== "communication_lost").length ?? 0, [overview]);
  const activeAlerts = overview?.unacknowledged_alerts.length ?? 0;

  return <div className="shell">
    <aside className="sidebar">
      <div className="brand"><div className="brand-mark">â£</div><div><strong>CBTC Platform</strong><span>Secure Â· Reliable Â· Intelligent</span></div></div>
      <nav>
        <NavItem icon="â¦" label="Dashboard" active={view === "dashboard"} onClick={() => setView("dashboard")} />
        <NavItem icon="M" label="World Map" active={view === "world-map"} onClick={() => setView("world-map")} />
        <NavItem icon="T" label="Track Map" active={view === "track-map"} onClick={() => setView("track-map")} /><NavItem icon="R" label="Trains" active={view === "trains"} onClick={() => setView("trains")} /><NavItem icon="S" label="Signals" active={view === "signals"} onClick={() => setView("signals")} /><NavItem icon="W" label="Switches" active={view === "switches"} onClick={() => setView("switches")} /><NavItem icon="A" label="Authorities" active={view === "authorities"} onClick={() => setView("authorities")} /><NavItem icon="E" label="Events" active={view === "events"} onClick={() => setView("events")} />
        <NavItem icon="!" label="Alerts" badge={activeAlerts} active={view === "alerts"} onClick={() => setView("alerts")} />
        <NavItem icon="â»" label="Replay & Trace" active={view === "replay"} onClick={() => setView("replay")} />
        <NavItem icon="â¤" label="Reports" /><NavItem icon="â" label="Settings" />
      </nav>
      <div className="sidebar-footer"><span className="online-dot" /> System operational</div>
    </aside>

    <main className="main">
      <header className="topbar"><div><span className="eyebrow">CBTC GLOBAL CONTROL CENTER</span><h1>{({dashboard:"Network Overview","world-map":"Global Rail Network","track-map":"Track Operations",trains:"Train Fleet",signals:"Signal Network",switches:"Switches & Routes",authorities:"Movement Authorities",events:"Event Stream",alerts:"Alerts",replay:"Replay & Forensics"} as Record<View,string>)[view]}</h1></div><div className="topbar-right"><span className="system-state"><i className="online-dot" /> System Online</span><span className="bell">â§<b>{activeAlerts}</b></span><div className="user"><div className="avatar">A</div><div><strong>admin</strong><span>Control Center</span></div></div></div></header>
      {error && <div className="global-error">{error}</div>}
      {view === "dashboard" ? <DashboardView overview={overview} events={events} connected={connected} activeTrains={activeTrains} /> : view === "world-map" ? <WorldMapView /> : view === "replay" ? <ReplayView /> : <OperationsView view={view} overview={overview} events={events} connected={connected} />}
    </main>
  </div>;
}

function OperationsView({ view, overview, events, connected }: { view: View; overview: NetworkOverview | null; events: ReturnType<typeof useEventStream>["events"]; connected: boolean }) { if (!overview) return <div className="loading-card">Loading network state</div>; return <div className="dashboard-content">{view === "track-map" ? <section className="panel map-panel"><div className="panel-title"><h3>Track Map</h3><span className="timestamp">{new Date(overview.generated_at).toLocaleString()}</span></div><TrackMap blocks={overview.blocks} trains={overview.trains} /></section> : view === "trains" ? <TrainsPanel trains={overview.trains} /> : view === "signals" ? <SignalsPanel signals={overview.signals} /> : view === "switches" ? <SwitchesPanel switches={overview.switches} /> : view === "authorities" ? <AuthorityPanel authorities={overview.active_authorities} /> : view === "events" ? <EventLogPanel events={events} connected={connected} /> : <AlertsPanel alerts={overview.unacknowledged_alerts} />}</div>; }

function DashboardView({ overview, events, connected, activeTrains }: { overview: NetworkOverview | null; events: ReturnType<typeof useEventStream>["events"]; connected: boolean; activeTrains: number }) {
  if (!overview) return <div className="loading-card">Loading network stateâ¦</div>;
  return <div className="dashboard-content">
    <section className="overview-strip"><Metric icon="â£" label="Trains" value={`${activeTrains} / ${overview.trains.length}`} note="Active / Total" blue /><Metric icon="â" label="Signals" value={`${overview.signals.length} / ${overview.signals.length}`} note="On / Total" green /><Metric icon="â" label="Switches" value={`${overview.switches.filter(s => !s.locked).length} / ${overview.switches.length}`} note="Normal / Total" purple /><Metric icon="â³" label="Active Alerts" value={String(overview.unacknowledged_alerts.length)} note="Requires Attention" red /></section>
    <section className="panel map-panel"><div className="panel-title"><h3>Track Map</h3><span className="timestamp">{new Date(overview.generated_at).toLocaleString()}</span></div><TrackMap blocks={overview.blocks} trains={overview.trains} /></section>
    <div className="three-col"><TrainsPanel trains={overview.trains} /><AlertsPanel alerts={overview.unacknowledged_alerts} /></div>
    <div className="three-col equal"><SignalsPanel signals={overview.signals} /><SwitchesPanel switches={overview.switches} /><AuthorityPanel authorities={overview.active_authorities} /></div>
    <div className="three-col lower"><EventLogPanel events={events} connected={connected} /><ServicePanel connected={connected} /><div className="panel mini-map"><div className="panel-title"><h3>Live Track View (Mini)</h3></div><div className="mini-track"><span className="mini-train blue">T-001</span><span className="mini-train yellow">T-003</span><i /><i /><i /><i /></div></div></div>
  </div>;
}

function WorldMapView() {
  return <div className="dashboard-content"><GlobalMap /></div>;
}

function ReplayView() {
  const [token, setToken] = useState(() => localStorage.getItem("cbtc_admin_token") ?? "");
  const [runs, setRuns] = useState<ReplayRun[]>([]);
  const [selected, setSelected] = useState<ReplayRun | null>(null);
  const [events, setEvents] = useState<ReplayEvent[]>([]);
  const [verification, setVerification] = useState<VerificationResult | null>(null);
  const [playing, setPlaying] = useState(false);
  const [cursor, setCursor] = useState(0);
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState("");
  const [loginOpen, setLoginOpen] = useState(false);

  async function refreshRuns() {
    if (!token) return;
    try { setRuns(await fetchReplayRuns(token)); setMessage(""); }
    catch (e) { if ((e as Error & { status?: number }).status === 401) { localStorage.removeItem("cbtc_admin_token"); setToken(""); setLoginOpen(true); } else setMessage((e as Error).message); }
  }
  useEffect(() => { if (token) void refreshRuns(); }, [token]);
  useEffect(() => { if (!playing || !events.length) return; const timer = setInterval(() => setCursor(c => { if (c >= events.length - 1) { setPlaying(false); return c; } return c + 1; }), 700); return () => clearInterval(timer); }, [playing, events.length]);

  async function chooseRun(run: ReplayRun) {
    if (!token) { setLoginOpen(true); return; }
    setSelected(run); setVerification(null); setCursor(0); setBusy(true);
    try { setEvents(await fetchReplayEvents(run.id, token)); } catch (e) { setMessage((e as Error).message); } finally { setBusy(false); }
  }
  async function verify() {
    if (!selected) return;
    if (!token) { setLoginOpen(true); return; }
    setBusy(true); try { setVerification(await verifyReplayRun(selected.id, token)); } catch (e) { setMessage((e as Error).message); } finally { setBusy(false); }
  }

  return <div className="replay-page">
    <div className="replay-toolbar"><div><span className="eyebrow">PTDF FORENSICS</span><h2>Replay & Trace</h2><p>The platform records continuously in the background. Sign in only when you want to inspect, replay, or verify stored traces.</p></div><div className="recording-live-card"><span className="recording-live-dot"/><div><strong>CONTINUOUS RECORDING</strong><small>PTDF trace capture is running automatically</small></div></div></div>
    {message && <div className="replay-message">{message}</div>}
    {!token && <div className="auth-card replay-access-card"><div><span className="eyebrow">PROTECTED ACCESS</span><h3>Replay inspection is locked</h3><p>Live recording does not require sign-in. Authentication is only needed to read or verify the recorded trace data.</p></div><button className="primary" onClick={() => setLoginOpen(true)}>Sign in to inspect</button></div>}
    <div className="replay-layout">
      <section className="panel session-panel"><div className="panel-title"><h3>Recorded Sessions</h3><span>{token ? `${runs.length} sessions` : "Sign in to browse"}</span></div>{token ? runs.map(run => <button className={`session-row ${selected?.id === run.id ? "selected" : ""}`} key={run.id} onClick={() => void chooseRun(run)}><span className="session-icon">R</span><span><strong>{run.label}</strong><small>{new Date(run.started_at).toLocaleString()}</small></span><span className="session-meta"><b>{run.event_count}</b><small>{run.status}</small></span></button>) : <div className="empty">Your traces are still being recorded. Sign in when you want to inspect them.</div>}</section>
      <section className="panel replay-stage"><div className="panel-title"><h3>{selected ? selected.label : "Replay workspace"}</h3>{selected && <span className={`status-badge ${selected.status}`}>{selected.status}</span>}</div>{selected ? <><div className="replay-controls"><button onClick={() => setCursor(Math.max(0, cursor - 1))}>Prev</button><button className="play" onClick={() => setPlaying(p => !p)}>{playing ? "Pause" : "Play"}</button><button onClick={() => setCursor(Math.min(Math.max(0, events.length - 1), cursor + 1))}>Next</button><span className="replay-position">Event {events.length ? cursor + 1 : 0} / {events.length}</span><button className="verify" onClick={() => void verify()} disabled={busy}>Verify Trace</button></div><input className="timeline" type="range" min="0" max={Math.max(0, events.length - 1)} value={Math.min(cursor, Math.max(0, events.length - 1))} onChange={e => setCursor(Number(e.target.value))} /><div className="replay-summary"><MetricSmall label="Events" value={String(events.length)} /><MetricSmall label="Duration" value={selected.ended_at ? formatDuration(new Date(selected.started_at), new Date(selected.ended_at)) : "Live"} /><MetricSmall label="Verification" value={verification?.status ?? "Not run"} /></div><div className="replay-grid"><div><h4>Event Timeline</h4><div className="timeline-list">{events.map((event, index) => <button className={`timeline-row ${index === cursor ? "current" : ""}`} key={`${event.sequence}-${event.event_id}`} onClick={() => setCursor(index)}><span>{event.sequence}</span><span>{new Date(event.recorded_at).toLocaleTimeString()}</span><span>{event.origin_service}</span><strong>{event.event_type}</strong></button>)}{!events.length && <div className="empty">No events captured for this session.</div>}</div></div><div className="event-inspector"><h4>Event Inspector</h4>{events[cursor] ? <><div className="inspector-meta"><span>Sequence <b>{events[cursor].sequence}</b></span><span>Service <b>{events[cursor].origin_service}</b></span><span>Event ID <b>{events[cursor].event_id.slice(0, 12)}...</b></span></div><pre>{JSON.stringify(events[cursor].payload, null, 2)}</pre></> : <div className="empty">Select an event to inspect its payload.</div>}</div></div>{verification && <VerificationCard result={verification} />}</> : <div className="empty large"><strong>Replay workspace ready</strong><span>{token ? "Select a recorded session to inspect its event timeline." : "Live capture is already running. Sign in when you need historical replay or verification."}</span></div>}</section>
    </div>
    {loginOpen && <LoginModal onSuccess={t => { localStorage.setItem("cbtc_admin_token", t); setToken(t); setLoginOpen(false); }} onClose={() => setLoginOpen(false)} />}
  </div>;
}

function LoginModal({ onSuccess, onClose }: { onSuccess: (token: string) => void; onClose: () => void }) { const [username, setUsername] = useState("admin"); const [password, setPassword] = useState(""); const [error, setError] = useState(""); const [busy, setBusy] = useState(false); return <div className="modal-backdrop"><form className="login-modal" onSubmit={async e => { e.preventDefault(); setBusy(true); setError(""); try { onSuccess(await login(username, password)); } catch (err) { setError((err as Error).message); } finally { setBusy(false); } }}><div className="brand-mark">â£</div><h2>Control Center Sign In</h2><p>Administrator credentials are required for Recording / Replay.</p><label>Username<input value={username} onChange={e => setUsername(e.target.value)} /></label><label>Password<input type="password" value={password} onChange={e => setPassword(e.target.value)} autoFocus /></label>{error && <div className="login-error">{error}</div>}<div className="modal-actions"><button type="button" onClick={onClose}>Cancel</button><button className="primary" disabled={busy}>{busy ? "Signing inâ¦" : "Sign in"}</button></div></form></div>; }
function VerificationCard({ result }: { result: VerificationResult }) { return <div className={`verification-card ${result.status === "PASS" ? "pass" : "fail"}`}><div><span className="eyebrow">DETERMINISTIC TRACE VERIFICATION</span><h3>{result.status === "PASS" ? "Trace verified" : "Divergence detected"}</h3><p>Mode: {result.mode} Â· {result.event_count} events inspected</p></div><strong>{result.status}</strong>{result.issues.length > 0 && <ul>{result.issues.slice(0, 8).map((issue, i) => <li key={i}><b>{issue.code}</b> â {issue.message}</li>)}</ul>}</div>; }
function Metric({ icon, label, value, note, blue, green, purple, red }: { icon: string; label: string; value: string; note: string; blue?: boolean; green?: boolean; purple?: boolean; red?: boolean }) { return <div className={`metric ${blue ? "blue" : green ? "green" : purple ? "purple" : red ? "red" : ""}`}><div className="metric-icon">{icon}</div><div><span>{label}</span><strong>{value}</strong><small>{note}</small></div></div>; }
function MetricSmall({ label, value }: { label: string; value: string }) { return <div><span>{label}</span><strong>{value}</strong></div>; }
function NavItem({ icon, label, active, badge, onClick }: { icon: string; label: string; active?: boolean; badge?: number; onClick?: () => void }) { return <button className={`nav-item ${active ? "active" : ""}`} onClick={onClick}><span className="nav-icon">{icon}</span>{label}{badge ? <b>{badge}</b> : null}</button>; }
function formatDuration(start: Date, end: Date) { const seconds = Math.max(0, Math.floor((end.getTime() - start.getTime()) / 1000)); const h = Math.floor(seconds / 3600); const m = Math.floor((seconds % 3600) / 60); const s = seconds % 60; return `${String(h).padStart(2, "0")}:${String(m).padStart(2, "0")}:${String(s).padStart(2, "0")}`; }





