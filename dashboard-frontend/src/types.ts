export interface StationOverview {
  id: string;
  name: string;
  country_code: string | null;
  latitude: number | null;
  longitude: number | null;
  station_code: string | null;
  operator: string | null;
  source: string;
}
export interface TrainOverview {
  id: string;
  identifier: string;
  operational_state: string;
  current_block: string | null;
  last_heartbeat: string;
}



export interface BlockOverview {
  id: string;
  name: string;
  occupancy: string;
  occupied_by: string | null;
}

export interface SignalOverview {
  id: string;
  name: string;
  current_aspect: string;
  protects_block: string;
}

export interface SwitchOverview {
  id: string;
  name: string;
  position: string;
  locked: boolean;
}

export interface AuthorityOverview {
  id: string;
  train_id: string;
  status: string;
  expires_at: string;
}

export interface AlertOverview {
  id: string;
  severity: string;
  title: string;
  created_at: string;
}

export interface NetworkOverview {
  trains: TrainOverview[];
  blocks: BlockOverview[];
  signals: SignalOverview[];
  switches: SwitchOverview[];
  active_authorities: AuthorityOverview[];
  unacknowledged_alerts: AlertOverview[];
  generated_at: string;
}

export interface EventEnvelope {
  event_id: string;
  causality_id: string;
  origin_service: string;
  occurred_at: string;
  payload: Record<string, unknown> & { event_type: string };
}

export interface ReplayRun {
  id: string;
  label: string;
  started_at: string;
  ended_at: string | null;
  status: string;
  event_count: number;
  active?: boolean;
}

export interface ReplayEvent {
  sequence: number;
  event_id: string;
  causality_id: string;
  origin_service: string;
  event_type: string;
  recorded_at: string;
  payload: Record<string, unknown>;
}

export interface VerificationIssue {
  sequence: number | null;
  code: string;
  message: string;
}

export interface VerificationResult {
  run_id: string;
  mode: "trace_integrity";
  status: "PASS" | "DIVERGED";
  event_count: number;
  issues: VerificationIssue[];
}




export interface RoutePoint {
  station_id: string;
  station_name: string;
  latitude: number;
  longitude: number;
  sequence: number;
}

export interface TrainRoute {
  train_id: string;
  train_number: string;
  operator: string | null;
  origin: string | null;
  destination: string | null;
  points: RoutePoint[];
}






export interface GlobalTrainOverview {
  id: string;
  train_number: string;
  operator: string | null;
  country_code: string | null;
  service_name: string | null;
  latitude: number | null;
  longitude: number | null;
  speed_kmh: number | null;
  bearing: number | null;
  current_station: string | null;
  next_station: string | null;
  destination: string | null;
  position_timestamp: string | null;
  source: string;
  data_status: string;
  passenger_count: number | null;
  passenger_capacity: number | null;
  occupancy_percentage: number | null;
  occupancy_status: string | null;
  passenger_count_source: string | null;
}
