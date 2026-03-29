interface MascotSvgProps {
  size: number;
}

export function ClaudeMascot({ size }: MascotSvgProps) {
  return (
    <svg
      viewBox="0 0 240 140"
      overflow="hidden"
      xmlns="http://www.w3.org/2000/svg"
      height={size * 0.75}
      width={size * 0.75 * (210 / 140)}
      style={{ shapeRendering: "crispEdges" }}
    >
      <g className="body">
        <rect className="pixel" x="40" y="0" width="160" height="115" />
      </g>
      <g className="arm-left">
        <rect className="pixel" x="10" y="60" width="42" height="30" />
      </g>
      <g className="arm-right">
        <rect className="pixel" x="188" y="60" width="42" height="30" />
      </g>
      <g className="legs">
        <rect className="pixel" x="55" y="115" width="15" height="30" />
        <rect className="pixel" x="85" y="115" width="15" height="30" />
        <rect className="pixel" x="140" y="115" width="15" height="30" />
        <rect className="pixel" x="170" y="115" width="15" height="30" />
      </g>
      <rect className="eye" x="65" y="25" width="15" height="35" style={{ transformOrigin: "72.5px 42.5px" }} />
      <rect className="eye" x="160" y="25" width="15" height="35" style={{ transformOrigin: "167.5px 42.5px" }} />
    </svg>
  );
}
