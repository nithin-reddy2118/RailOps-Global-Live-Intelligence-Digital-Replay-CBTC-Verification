import { useEffect, useMemo, useRef, useState } from "react";
import {

  MapContainer,
  Marker,
  Polyline,
  Popup,
  TileLayer,
  Tooltip,
  useMap,
} from "react-leaflet";
import { divIcon } from "leaflet";
import type { GlobalTrainOverview, StationOverview, TrainRoute } from "./types";
import { fetchGlobalTrains, fetchStations } from "./api";

type Filter = "All" | "LIVE" | "PREDICTED" | "SCHEDULED" | "LAST KNOWN";

type MapTrain = GlobalTrainOverview & {
  status: Exclude<Filter, "All">;
};

type MapStation = StationOverview;

type SearchResult =
  | { kind: "train"; train: MapTrain }
  | { kind: "station"; station: MapStation };

const API_BASE_URL =
  import.meta.env.VITE_API_BASE_URL ?? "http://localhost:8094";

function normalizeStatus(value: string | null | undefined): MapTrain["status"] {
  const status = (value ?? "").toUpperCase().replace(/[_-]/g, " ");

  if (status.includes("REALTIME") || status.includes("LIVE")) return "LIVE";
  if (status.includes("PREDICT")) return "PREDICTED";
  if (status.includes("SCHEDULE")) return "SCHEDULED";

  return "LAST KNOWN";
}

function formatSpeed(speed: number | null): string {
  return speed == null ? "Unavailable" : `${speed.toFixed(1)} km/h`;
}

function formatBearing(bearing: number | null): string {
  if (bearing == null) return "Unavailable";

  const directions = ["N", "NE", "E", "SE", "S", "SW", "W", "NW"];

  return `${bearing.toFixed(0)}° ${
    directions[Math.round(bearing / 45) % 8]
  }`;
}

function formatAge(timestamp: string | null): string {
  if (!timestamp) return "Unavailable";

  const time = new Date(timestamp).getTime();

  if (!Number.isFinite(time)) return "Unavailable";

  const seconds = Math.max(0, Math.floor((Date.now() - time) / 1000));

  if (seconds < 60) return `${seconds}s ago`;

  const minutes = Math.floor(seconds / 60);

  if (minutes < 60) return `${minutes}m ago`;

  return `${Math.floor(minutes / 60)}h ago`;
}

function trainLabel(train: MapTrain): string {
  return train.service_name
    ? `${train.train_number} · ${train.service_name}`
    : train.train_number;
}

function MapViewport({
  train,
  station,
  onZoomChange,
}: {
  train: MapTrain | null;
  station: MapStation | null;
  onZoomChange: (zoom: number) => void;
}) {
  const map = useMap();

  useEffect(() => {
    onZoomChange(map.getZoom());

    const handleZoom = () => {
      onZoomChange(map.getZoom());
    };

    map.on("zoomend", handleZoom);

    return () => {
      map.off("zoomend", handleZoom);
    };
  }, [map, onZoomChange]);

  useEffect(() => {
    onZoomChange(map.getZoom());

    const handleZoom = () => {
      onZoomChange(map.getZoom());
    };

    map.on("zoomend", handleZoom);

    return () => {
      map.off("zoomend", handleZoom);
    };
  }, [map, onZoomChange]);

  useEffect(() => {
    if (train?.latitude != null && train?.longitude != null) {
      map.flyTo(
        [train.latitude, train.longitude],
        Math.max(map.getZoom(), 8),
        { duration: 0.9 },
      );
      return;
    }

    if (station?.latitude != null && station?.longitude != null) {
      map.flyTo(
        [station.latitude, station.longitude],
        Math.max(map.getZoom(), 9),
        { duration: 0.9 },
      );
    }
  }, [map, train, station]);

  return null;
}

function MapControls() {
  const map = useMap();

  return (
    <div className="global-map-controls">
      <button
        type="button"
        onClick={() => map.fitWorld({ padding: [20, 20] })}
      >
        World
      </button>
    </div>
  );
}

function AnimatedTrainMarker({
  train,
  selected,
  onSelect,
}: {
  train: MapTrain;
  selected: boolean;
  onSelect: () => void;
}) {
  const initialPosition: [number, number] = [
    train.latitude as number,
    train.longitude as number,
  ];

  const [position, setPosition] =
    useState<[number, number]>(initialPosition);

  const lastFeedPosition = useRef<[number, number]>(initialPosition);
  const lastFeedTime = useRef(
    train.position_timestamp
      ? new Date(train.position_timestamp).getTime()
      : Date.now(),
  );

  useEffect(() => {
    if (train.latitude == null || train.longitude == null) return;

    const target: [number, number] = [train.latitude, train.longitude];

    lastFeedPosition.current = target;
    lastFeedTime.current = train.position_timestamp
      ? new Date(train.position_timestamp).getTime()
      : Date.now();

    setPosition(target);
  }, [train.latitude, train.longitude, train.position_timestamp]);

  useEffect(() => {
    if (
      train.status !== "LIVE" ||
      train.speed_kmh == null ||
      train.bearing == null ||
      train.latitude == null ||
      train.longitude == null
    ) {
      return;
    }

    let frame = 0;
    const startedAt = performance.now();
    const animate = (now: number) => {
      const elapsedSeconds = (now - startedAt) / 1000;

      // Never extrapolate beyond one normal feed interval.
      const secondsAhead = Math.min(elapsedSeconds, 15);

      const speedMetersPerSecond = train.speed_kmh! / 3.6;
      const distanceMeters =
        speedMetersPerSecond * secondsAhead;

      const bearingRadians =
        (train.bearing! * Math.PI) / 180;

      const latitudeRadians =
        (train.latitude! * Math.PI) / 180;

      const metersPerDegreeLatitude = 111_320;
      const metersPerDegreeLongitude =
        111_320 * Math.cos(latitudeRadians);

      const projectedLatitude =
        train.latitude! +
        (distanceMeters * Math.cos(bearingRadians)) /
          metersPerDegreeLatitude;

      const projectedLongitude =
        train.longitude! +
        (distanceMeters * Math.sin(bearingRadians)) /
          metersPerDegreeLongitude;

      setPosition([projectedLatitude, projectedLongitude]);

      if (elapsedSeconds < 15) {
        frame = requestAnimationFrame(animate);
      }
    };

    frame = requestAnimationFrame(animate);

    return () => cancelAnimationFrame(frame);
  }, [
    train.latitude,
    train.longitude,
    train.speed_kmh,
    train.bearing,
    train.status,
    train.position_timestamp,
  ]);

  const statusClass =
    train.status === "LIVE"
      ? "live"
      : train.status === "PREDICTED"
        ? "predicted"
        : train.status === "SCHEDULED"
          ? "scheduled"
          : "last-known";

  const icon = divIcon({
    className: "global-train-icon-wrap",
    html: `<div class="global-train-icon ${statusClass} ${selected ? "selected" : ""}">
      <span class="global-train-icon-glow"></span>
      <span class="global-train-icon-body">
        <span class="global-train-window"></span>
        <span class="global-train-window small"></span>
        <span class="global-train-light"></span>
      </span>
      <span class="global-train-label">${train.train_number}</span>
    </div>`,
    iconSize: [52, 52],
    iconAnchor: [26, 26],
  });

  return (
    <Marker
      position={position}
      icon={icon}
      zIndexOffset={
        selected ? 1000 : train.status === "LIVE" ? 500 : 100
      }
      eventHandlers={{ click: onSelect }}
    >
      <Tooltip
        permanent={selected}
        direction="top"
        offset={[0, -20]}
      >
        {trainLabel(train)}
      </Tooltip>

      <Popup>
        <div>
          <strong>{trainLabel(train)}</strong>
          <div>Status: {train.status}</div>
          <div>Speed: {formatSpeed(train.speed_kmh)}</div>
          <div>
            Destination: {train.destination ?? "Unavailable"}
          </div>
          <div>
            Last feed update: {formatAge(train.position_timestamp)}
          </div>
          <button type="button" onClick={onSelect}>
            Open details
          </button>
        </div>
      </Popup>
    </Marker>
  );
}
function occupancyLabel(train: GlobalTrainOverview): string {
  if (train.passenger_count !== null) {
    const count = train.passenger_count.toLocaleString();
    const capacity =
      train.passenger_capacity !== null
        ? ` / ${train.passenger_capacity.toLocaleString()}`
        : "";
    const percentage =
      train.occupancy_percentage !== null
        ? ` (${train.occupancy_percentage.toFixed(0)}%)`
        : "";

    return `${count}${capacity}${percentage}`;
  }

  return "Unavailable - no real passenger feed";
}
export function GlobalMap() {
  const [trains, setTrains] = useState<MapTrain[]>([]);
  const [stations, setStations] = useState<MapStation[]>([]);
  const [filter, setFilter] = useState<Filter>("All");
  const [search, setSearch] = useState("");
  const [selectedTrain, setSelectedTrain] = useState<MapTrain | null>(null);
  const [selectedStation, setSelectedStation] =
    useState<MapStation | null>(null);
  const [route, setRoute] = useState<TrainRoute | null>(null);
  const [routeLoading, setRouteLoading] = useState(false);
  const [routeError, setRouteError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [lastRefresh, setLastRefresh] = useState<Date | null>(null);
  const [mapZoom, setMapZoom] = useState(2);
  const routeRequestId = useRef(0);

  async function loadMapData() {
    try {
      setError(null);

      const [trainRows, stationRows] = await Promise.all([
        fetchGlobalTrains(),
        fetchStations(),
      ]);

      const mappedTrains = trainRows
        .filter(
          (train) =>
            train.latitude != null &&
            train.longitude != null,
        )
        .map((train) => ({
          ...train,
          status: normalizeStatus(train.data_status),
        }));

      setTrains(mappedTrains);
      setStations(stationRows);
      setLastRefresh(new Date());

      setSelectedTrain((current) => {
        if (!current) return current;

        return (
          mappedTrains.find((train) => train.id === current.id) ??
          current
        );
      });
    } catch (err) {
      setError(
        err instanceof Error
          ? err.message
          : "Unable to load global map data.",
      );
    } finally {
      setLoading(false);
    }
  }

  useEffect(() => {
    void loadMapData();

    const interval = window.setInterval(() => {
      void loadMapData();
    }, 15000);

    return () => window.clearInterval(interval);
  }, []);

  const visibleTrains = useMemo(() => {
  if (filter === "All") return trains;

  return trains.filter((train) => train.status === filter);
}, [filter, trains]);

const renderedTrains = useMemo(() => {
  let limit = 600;

  if (mapZoom >= 3) limit = 1500;
  if (mapZoom >= 4) limit = 3000;
  if (mapZoom >= 5) limit = 6000;
  if (mapZoom >= 6) limit = visibleTrains.length;

  const limited = visibleTrains.slice(0, limit);

  if (
    selectedTrain &&
    !limited.some((train) => train.id === selectedTrain.id)
  ) {
    return [selectedTrain, ...limited];
  }

  return limited;
}, [mapZoom, selectedTrain, visibleTrains]);

  const liveCount = useMemo(
    () => trains.filter((train) => train.status === "LIVE").length,
    [trains],
  );

  const searchResults = useMemo<SearchResult[]>(() => {
    const query = search.trim().toLowerCase();

    if (!query) return [];

    const results: SearchResult[] = [];

    for (const train of trains) {
      const haystack = [
        train.train_number,
        train.operator,
        train.service_name,
        train.current_station,
        train.next_station,
        train.destination,
        train.country_code,
      ]
        .filter(Boolean)
        .join(" ")
        .toLowerCase();

      if (haystack.includes(query)) {
        results.push({ kind: "train", train });
      }
    }

    for (const station of stations) {
      const haystack = [
        station.name,
        station.station_code,
        station.operator,
        station.country_code,
      ]
        .filter(Boolean)
        .join(" ")
        .toLowerCase();

      if (haystack.includes(query)) {
        results.push({ kind: "station", station });
      }
    }

    return results.slice(0, 12);
  }, [search, stations, trains]);

  async function selectTrain(train: MapTrain) {
    setSelectedTrain(train);
    setSelectedStation(null);
    setSearch(trainLabel(train));
    setRoute(null);
    setRouteError(null);

    if (!train.train_number) return;

    const requestId = ++routeRequestId.current;

    setRouteLoading(true);

    try {
      const response = await fetch(
        `${API_BASE_URL}/dashboard/trains/${encodeURIComponent(
          train.train_number,
        )}/route`,
      );

      if (!response.ok) {
        throw new Error(`Route request failed (${response.status}).`);
      }

      const routeData = (await response.json()) as TrainRoute;

      if (requestId === routeRequestId.current) {
        setRoute(routeData);
      }
    } catch (err) {
      if (requestId === routeRequestId.current) {
        setRouteError(
          err instanceof Error
            ? err.message
            : "Route data unavailable.",
        );
      }
    } finally {
      if (requestId === routeRequestId.current) {
        setRouteLoading(false);
      }
    }
  }

  function selectStation(station: MapStation) {
    setSelectedStation(station);
    setSelectedTrain(null);
    setRoute(null);
    setRouteError(null);
    setSearch(station.name);
  }

  function selectSearchResult(result: SearchResult) {
    if (result.kind === "train") {
      void selectTrain(result.train);
    } else {
      selectStation(result.station);
    }
  }

  const routePositions =
    route?.points
      ?.filter(
        (point) =>
          Number.isFinite(point.latitude) &&
          Number.isFinite(point.longitude),
      )
      .map(
        (point) =>
          [point.latitude, point.longitude] as [number, number],
      ) ?? [];

  return (
    <section className="global-map-shell">
      <div className="global-map-header">
        <div>
          <h2>Global Rail Network</h2>
          <p>
            Live railway feeds over a worldwide railway-infrastructure layer. Confirmed positions refresh every 15 seconds.
          </p>
        </div>

        <div className="global-map-summary">
          <span>{visibleTrains.length} trains</span>
          <span>{liveCount} LIVE</span>
          <span>{stations.length} stations</span>
          <span>
            Updated {lastRefresh ? lastRefresh.toLocaleTimeString() : "—"}
          </span>
        </div>
      </div>

      <div className="global-map-toolbar">
        <div className="global-map-search">
          <input
            value={search}
            onChange={(event) => setSearch(event.target.value)}
            onKeyDown={(event) => {
              if (event.key === "Enter" && searchResults[0]) {
                selectSearchResult(searchResults[0]);
              }

              if (event.key === "Escape") {
                setSearch("");
              }
            }}
            placeholder="Search train, station, city, operator..."
            aria-label="Search trains and stations"
          />

          {searchResults.length > 0 && (
            <div className="global-map-search-results">
              {searchResults.map((result) => {
                if (result.kind === "train") {
                  return (
                    <button
                      type="button"
                      className="global-map-search-result"
                      key={`train-${result.train.id}`}
                      onClick={() => selectSearchResult(result)}
                    >
                      <strong>{trainLabel(result.train)}</strong>
                      <span>
                        {result.train.operator ?? "Unknown operator"} ·{" "}
                        {result.train.destination ??
                          "Destination unavailable"}
                      </span>
                    </button>
                  );
                }

                return (
                  <button
                    type="button"
                    className="global-map-search-result"
                    key={`station-${result.station.id}`}
                    onClick={() => selectSearchResult(result)}
                  >
                    <strong>{result.station.name}</strong>
                    <span>
                      {result.station.operator ?? "Unknown operator"} ·{" "}
                      {result.station.country_code ??
                        "Country unavailable"}
                    </span>
                  </button>
                );
              })}
            </div>
          )}
        </div>

        <div className="global-map-filters">
          {(
            [
              "All",
              "LIVE",
              "PREDICTED",
              "SCHEDULED",
              "LAST KNOWN",
            ] as Filter[]
          ).map((item) => (
            <button
              key={item}
              type="button"
              className={filter === item ? "active" : ""}
              onClick={() => setFilter(item)}
            >
              {item}
            </button>
          ))}
        </div>

        <button
          type="button"
          className="global-map-refresh"
          onClick={() => void loadMapData()}
        >
          Refresh
        </button>
      </div>

      {error && <div className="global-map-error">{error}</div>}

      <div className="global-map-content">
        <div className="global-map-canvas">
          <MapContainer
            center={[20, 0]}
            zoom={2}
            minZoom={2}
            maxZoom={18}
            scrollWheelZoom
            doubleClickZoom
            dragging
            touchZoom
            boxZoom
            keyboard
            zoomControl
            style={{ height: "100%", width: "100%" }}
          >
            <TileLayer attribution="&copy; OpenStreetMap contributors" url="https://{s}.tile.openstreetmap.org/{z}/{x}/{y}.png" />
            
            <MapViewport
              train={selectedTrain}
              station={selectedStation}
              onZoomChange={setMapZoom}
            />
  

            <MapControls />

            {routePositions.length >= 2 && (
              <Polyline
                positions={routePositions}
                pathOptions={{
                  color: "#2563eb",
                  weight: 5,
                  opacity: 0.85,
                }}
              />
            )}

            {stations
              .filter((station) => station.latitude != null && station.longitude != null)
              .map((station) => {
                const stationIcon = divIcon({
                  className: "global-station-icon-wrap",
                  html: `<div class="global-station-icon"><span class="global-station-pulse"></span><span class="global-station-glyph">??</span></div>`,
                  iconSize: [28, 28], iconAnchor: [14, 14],
                });
                return <Marker key={station.id} position={[station.latitude as number, station.longitude as number]} icon={stationIcon} zIndexOffset={selectedStation?.id === station.id ? 900 : 50} eventHandlers={{ click: () => selectStation(station) }}>
                  <Tooltip direction="top" offset={[0, -12]}>{station.name}</Tooltip>
                  <Popup><div><strong>{station.name}</strong><div>{station.operator ?? "Operator unavailable"}</div><div>{station.station_code ?? "No station code"}</div></div></Popup>
                </Marker>;
              })}

            {renderedTrains.map((train) => {
              if (
                train.latitude == null ||
                train.longitude == null
              ) {
                return null;
              }

              return (
                <AnimatedTrainMarker
                  key={train.id}
                  train={train}
                  selected={selectedTrain?.id === train.id}
                  onSelect={() => void selectTrain(train)}
                />
              );
            })}
          </MapContainer>

          <div className="global-map-interaction-hint">
            🌍 World railway view · tracks + stations · live train markers · 15s refresh
          </div>
        </div>

        <aside className="global-map-details">
          {loading && !selectedTrain && !selectedStation && (
            <div className="map-detail-empty">
              Loading network data...
            </div>
          )}

          {!selectedTrain && !selectedStation && !loading && (
            <div className="map-detail-empty">
              <h3>Explore the global network</h3>
              <p>
                {liveCount} live trains are currently available from the
                connected railway feeds.
              </p>
              <p>
                Train positions are never invented. If a real feed does not
                provide coordinates, that train is not placed on the map.
              </p>
            </div>
          )}

          {selectedTrain && (
            <div className="map-detail-card">
              <div className="map-detail-heading">
                <div>
                  <span className="map-detail-kicker">
                    {selectedTrain.status}
                  </span>
                  <h3>{trainLabel(selectedTrain)}</h3>
                </div>

                <button
                  type="button"
                  onClick={() => setSelectedTrain(null)}
                >
                  Close
                </button>
              </div>

              <div className="map-detail-grid">
                <div>
                  <span>Operator</span>
                  <strong>
                    {selectedTrain.operator ?? "Unavailable"}
                  </strong>
                </div>

                <div>
                  <span>Country</span>
                  <strong>
                    {selectedTrain.country_code ?? "Unavailable"}
                  </strong>
                </div>

                <div>
                  <span>Speed</span>
                  <strong>
                    {formatSpeed(selectedTrain.speed_kmh)}
                  </strong>
                </div>

                <div>
                  <span>Direction</span>
                  <strong>
                    {formatBearing(selectedTrain.bearing)}
                  </strong>
                </div>

                <div>
                  <span>Current station</span>
                  <strong>
                    {selectedTrain.current_station ?? "Unavailable"}
                  </strong>
                </div>

                <div>
                  <span>Next station</span>
                  <strong>
                    {selectedTrain.next_station ?? "Unavailable"}
                  </strong>
                </div>

                <div>
                  <span>Destination</span>
                  <strong>
                    {selectedTrain.destination ?? "Unavailable"}
                  </strong>
                </div>

                <div>
                  <span>Data source</span>
                  <strong>
                    {selectedTrain.source || "Unavailable"}
                  </strong>
                </div>

                <div>
                  <span>Last update</span>
                  <strong>
                    {formatAge(selectedTrain.position_timestamp)}
                  </strong>
                </div>
              </div>

              <div className="map-detail-route">
                <h4>Route</h4>

                {routeLoading && <p>Loading route...</p>}

                {routeError && (
                  <p className="map-detail-warning">{routeError}</p>
                )}

                {!routeLoading &&
                  !routeError &&
                  routePositions.length < 2 && (
                    <p>
                      No route geometry is currently available for this
                      train.
                    </p>
                  )}

                {!routeLoading &&
                  !routeError &&
                  routePositions.length >= 2 && (
                    <p>
                      Route loaded with {routePositions.length} mapped
                      points.
                    </p>
                  )}
              </div>
            </div>
          )}

          {selectedStation && (
            <div className="map-detail-card">
              <div className="map-detail-heading">
                <div>
                  <span className="map-detail-kicker">STATION</span>
                  <h3>{selectedStation.name}</h3>
                </div>

                <button
                  type="button"
                  onClick={() => setSelectedStation(null)}
                >
                  Close
                </button>
              </div>

              <div className="map-detail-grid">
                <div>
                  <span>Operator</span>
                  <strong>
                    {selectedStation.operator ?? "Unavailable"}
                  </strong>
                </div>

                <div>
                  <span>Country</span>
                  <strong>
                    {selectedStation.country_code ?? "Unavailable"}
                  </strong>
                </div>

                <div>
                  <span>Station code</span>
                  <strong>
                    {selectedStation.station_code ?? "Unavailable"}
                  </strong>
                </div>

                <div>
                  <div>
  <strong>Passenger load:</strong> {selectedTrain ? occupancyLabel(selectedTrain) : "Unavailable"}
  {selectedTrain?.passenger_count_source
    ? ` - ${selectedTrain?.passenger_count_source}`
    : ""}
</div>
<span>Source</span>
                  <strong>
                    {selectedStation.source || "Unavailable"}
                  </strong>
                </div>
              </div>

              <div className="map-detail-route">
                <h4>Upcoming trains</h4>
                <p>
                  The current dashboard backend does not expose a station
                  arrival-board endpoint yet, so no ETA is fabricated here.
                </p>
              </div>
            </div>
          )}
        </aside>
      </div>
    </section>
  );
}







