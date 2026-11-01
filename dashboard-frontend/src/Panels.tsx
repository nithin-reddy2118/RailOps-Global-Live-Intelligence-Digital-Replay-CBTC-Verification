import type { AlertOverview, AuthorityOverview, EventEnvelope, SignalOverview, SwitchOverview, TrainOverview } from "./types";

export function TrainsPanel({ trains }: { trains: TrainOverview[] }) {
  return <div className="panel table-panel"><PanelTitle title="Trains" action="View All" /><table><thead><tr><th>ID</th><th>State</th><th>Block</th><th>Heartbeat</th></tr></thead><tbody>{trains.map(t => <tr key={t.id}><td>{t.identifier}</td><td><StatusPill status={t.operational_state} /></td><td>{t.current_block ?? "—"}</td><td>{new Date(t.last_heartbeat).toLocaleTimeString()}</td></tr>)}</tbody></table></div>;
}

export function SignalsPanel({ signals }: { signals: SignalOverview[] }) {
  return <div className="panel table-panel"><PanelTitle title="Signals" action="View All" /><table><thead><tr><th>ID</th><th>Aspect</th><th>Status</th></tr></thead><tbody>{signals.map(s => <tr key={s.id}><td>{s.name}</td><td><SignalAspect aspect={s.current_aspect} /></td><td><span className="text-ok">Normal</span></td></tr>)}</tbody></table></div>;
}

export function SwitchesPanel({ switches }: { switches: SwitchOverview[] }) {
  return <div className="panel table-panel"><PanelTitle title="Switches" action="View All" /><table><thead><tr><th>ID</th><th>Position</th><th>Locked</th></tr></thead><tbody>{switches.map(s => <tr key={s.id}><td>{s.name}</td><td><span className="pill blue">{s.position}</span></td><td><StatusPill status={s.locked ? "locked" : "normal"} /></td></tr>)}</tbody></table></div>;
}

export function AuthorityPanel({ authorities }: { authorities: AuthorityOverview[] }) {
  return <div className="panel table-panel"><PanelTitle title="Movement Authorities" action="View All" /><table><thead><tr><th>Train</th><th>Authority</th><th>Status</th></tr></thead><tbody>{authorities.map(a => <tr key={a.id}><td>{a.train_id}</td><td>{a.id.slice(0, 8)}</td><td><StatusPill status={a.status} /></td></tr>)}</tbody></table></div>;
}

export function AlertsPanel({ alerts }: { alerts: AlertOverview[] }) {
  return <div className="panel alert-panel"><PanelTitle title="Recent Alerts" action="View All" /><div className="alert-table">{alerts.length === 0 ? <div className="empty">No active alerts</div> : alerts.slice(0, 5).map(a => <div className="alert-row" key={a.id}><span className={`alert-severity ${a.severity}`}>{a.severity}</span><div><strong>{a.title}</strong><small>{new Date(a.created_at).toLocaleTimeString()}</small></div></div>)}</div></div>;
}

export function EventLogPanel({ events, connected }: { events: EventEnvelope[]; connected: boolean }) {
  return <div className="panel event-panel"><PanelTitle title="Event Stream" action="View All" /><div className="event-header"><span>Time</span><span>Service</span><span>Event</span><span>Details</span></div>{events.slice(0, 7).map(e => <div className="event-grid" key={e.event_id}><span>{new Date(e.occurred_at).toLocaleTimeString()}</span><span>{e.origin_service}</span><span>{e.payload.event_type}</span><span>{formatEventDetails(e.payload)}</span></div>)}{events.length === 0 && <div className="empty">Waiting for events…</div>}<div className="stream-status"><i className={connected ? "online-dot" : "offline-dot"} />{connected ? "Live event stream connected" : "Reconnecting to event stream…"}</div></div>;
}

export function ServicePanel({ connected }: { connected: boolean }) {
  const services = [
    ["CBTC API", "Healthy"], ["Event bus (RabbitMQ)", connected ? "Healthy" : "Degraded"], ["Central control", "Healthy"], ["Dashboard backend", "Healthy"],
  ];
  return <div className="panel service-panel"><PanelTitle title="System Services" action="View All" />{services.map(([name, status]) => <div className="service-row" key={name}><span><i className={status === "Healthy" ? "online-dot" : "warning-dot"} />{name}</span><b>{status}</b></div>)}</div>;
}

function PanelTitle({ title, action }: { title: string; action?: string }) { return <div className="panel-title"><h3>{title}</h3>{action && <button>{action} →</button>}</div>; }
function StatusPill({ status }: { status: string }) { const good = ["normal", "running", "active", "proceed", "locked", "completed"].some(s => status.toLowerCase().includes(s)); return <span className={`pill ${good ? "green" : status.toLowerCase().includes("pending") ? "amber" : "red"}`}>{status}</span>; }
function SignalAspect({ aspect }: { aspect: string }) { const key = aspect.toLowerCase(); const cls = key.includes("green") || key.includes("proceed") ? "green" : key.includes("yellow") || key.includes("approach") ? "amber" : "red"; return <span className={`signal-aspect ${cls}`}><i />{aspect}</span>; }
function formatEventDetails(payload: Record<string, unknown>) { const preferred = ["train_id", "block_id", "signal_id", "authority_id", "position", "status"]; for (const key of preferred) if (payload[key] !== undefined) return `${key.split("_").join(" ")}: ${String(payload[key])}`; return "Domain event received"; }
