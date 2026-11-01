import type { BlockOverview, TrainOverview } from "./types";

interface Props { blocks: BlockOverview[]; trains: TrainOverview[]; }

const COLORS: Record<string, string> = {
  clear: "#24c47a",
  occupied: "#ff4d67",
  ambiguous: "#f0b429",
  out_of_service: "#667085",
};

export function TrackMap({ blocks, trains }: Props) {
  const sorted = [...blocks].sort((a, b) => a.name.localeCompare(b.name));
  const trainById = new Map(trains.map((t) => [t.id, t]));
  const width = Math.max(900, sorted.length * 150 + 80);

  return (
    <div className="track-map-shell">
      <svg viewBox={`0 0 ${width} 300`} className="track-svg" role="img" aria-label="CBTC network track map">
        <defs>
          <linearGradient id="trackGlow" x1="0" x2="1">
            <stop offset="0" stopColor="#1a5a8a" />
            <stop offset="0.5" stopColor="#25c7a1" />
            <stop offset="1" stopColor="#1a5a8a" />
          </linearGradient>
        </defs>
        <text x="36" y="34" className="map-label">NORTH YARD</text>
        <text x={width - 150} y="34" className="map-label">EAST JUNCTION</text>
        <path d={`M 42 112 H ${width - 42}`} className="rail-base" />
        <path d={`M 42 112 H ${width - 42}`} className="rail-active" />
        <path d={`M ${width * 0.38} 112 C ${width * 0.43} 112, ${width * 0.43} 195, ${width * 0.50} 195 H ${width * 0.68}`} className="branch-rail" />
        <path d={`M ${width * 0.52} 195 C ${width * 0.56} 195, ${width * 0.57} 112, ${width * 0.64} 112`} className="branch-rail" />

        {sorted.map((block, i) => {
          const x = 55 + i * ((width - 110) / Math.max(1, sorted.length - 1));
          const color = COLORS[block.occupancy] ?? "#667085";
          const train = block.occupied_by ? trainById.get(block.occupied_by) : undefined;
          return (
            <g key={block.id}>
              <circle cx={x} cy="112" r="9" fill={color} stroke="#06111e" strokeWidth="4" />
              <text x={x} y="150" textAnchor="middle" className="block-label">{block.name}</text>
              {train && (
                <g>
                  <rect x={x - 30} y="72" width="60" height="25" rx="7" className="train-chip" />
                  <text x={x} y="89" textAnchor="middle" className="train-chip-text">{train.identifier}</text>
                </g>
              )}
            </g>
          );
        })}

        <text x={width * 0.47} y="225" textAnchor="middle" className="station-label">CENTRAL STATION</text>
        <rect x={width * 0.44} y="180" width="16" height="16" className="junction" />
        <rect x={width * 0.57} y="180" width="16" height="16" className="junction" />
      </svg>
      <div className="map-legend">
        <span><i className="legend-dot train" /> Train</span>
        <span><i className="legend-dot occupied" /> Occupied block</span>
        <span><i className="legend-dot free" /> Free block</span>
        <span><i className="legend-ring stop" /> Signal stop</span>
        <span><i className="legend-diamond" /> Switch</span>
      </div>
    </div>
  );
}
