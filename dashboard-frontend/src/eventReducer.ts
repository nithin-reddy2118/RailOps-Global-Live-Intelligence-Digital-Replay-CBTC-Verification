import type { EventEnvelope, NetworkOverview } from "./types";

/** Applies one incoming event onto the current overview snapshot,
 * returning a new object (never mutating in place, so React's
 * reference-equality re-render check keeps working). Events whose
 * payload doesn't map onto anything shown in the overview (e.g.
 * telemetry position reports, which are too high-frequency to reflect
 * in this coarse view) are ignored here.
 */
export function applyEvent(overview: NetworkOverview, envelope: EventEnvelope): NetworkOverview {
  const { payload } = envelope;

  switch (payload.event_type) {
    case "BlockOccupancyChanged": {
      const blockId = payload.block_id as string;
      const newState = (payload.new_state as string).toLowerCase();
      const trainId = (payload.train_id as string | null) ?? null;
      return {
        ...overview,
        blocks: overview.blocks.map((b) =>
          b.id === blockId ? { ...b, occupancy: newState, occupied_by: trainId } : b,
        ),
      };
    }

    case "SignalAspectChanged": {
      const signalId = payload.signal_id as string;
      const newAspect = (payload.new_aspect as string).toLowerCase();
      return {
        ...overview,
        signals: overview.signals.map((s) => (s.id === signalId ? { ...s, current_aspect: newAspect } : s)),
      };
    }

    case "SwitchPositionChanged": {
      const switchId = payload.switch_id as string;
      const newPosition = (payload.new_position as string).toLowerCase();
      return {
        ...overview,
        switches: overview.switches.map((sw) => (sw.id === switchId ? { ...sw, position: newPosition } : sw)),
      };
    }

    case "TrainStateChanged": {
      const trainId = payload.train_id as string;
      const newState = payload.new_state as string;
      return {
        ...overview,
        trains: overview.trains.map((t) => (t.id === trainId ? { ...t, operational_state: newState } : t)),
      };
    }

    case "MovementAuthorityExpired":
    case "MovementAuthorityRevoked": {
      const authorityId = payload.authority_id as string;
      return {
        ...overview,
        active_authorities: overview.active_authorities.filter((a) => a.id !== authorityId),
      };
    }

    case "MovementAuthorityGranted": {
      const authorityId = payload.authority_id as string;
      const trainId = payload.train_id as string;
      // The overview doesn't get the full authority record from this
      // event alone (block_sequence isn't included), so this inserts a
      // minimal placeholder entry; a full snapshot refresh (App.tsx
      // polls periodically) reconciles the complete record shortly
      // after.
      if (overview.active_authorities.some((a) => a.id === authorityId)) {
        return overview;
      }
      return {
        ...overview,
        active_authorities: [
          ...overview.active_authorities,
          { id: authorityId, train_id: trainId, status: "granted", expires_at: envelope.occurred_at },
        ],
      };
    }

    default:
      return overview;
  }
}
