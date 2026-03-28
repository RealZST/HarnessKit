interface MascotSvgProps {
  size: number;
}

export function CursorMascot({ size }: MascotSvgProps) {
  return (
    <div style={{ display: "flex", flexDirection: "column", alignItems: "center" }}>
      <div className="rocker" style={{ perspective: 800 }}>
        <svg className="icon-svg" viewBox="0 0 24 24" xmlns="http://www.w3.org/2000/svg" width={size * 0.7} height={size * 0.7}>
          <path className="icon-fill" d="M22.106 5.68L12.5.135a.998.998 0 00-.998 0L1.893 5.68a.84.84 0 00-.419.726v11.186c0 .3.16.577.42.727l9.607 5.547a.999.999 0 00.998 0l9.608-5.547a.84.84 0 00.42-.727V6.407a.84.84 0 00-.42-.726zm-.603 1.176L12.228 22.92c-.063.108-.228.064-.228-.061V12.34a.59.59 0 00-.295-.51l-9.11-5.26c-.107-.062-.063-.228.062-.228h18.55c.264 0 .428.286.296.514z" />
        </svg>
      </div>
      <svg viewBox="0 0 24 5" xmlns="http://www.w3.org/2000/svg" width={size * 0.85} style={{ marginTop: 2 }}>
        <defs>
          <filter id="mascot-cursor-blur"><feGaussianBlur in="SourceGraphic" stdDeviation="0.6" /></filter>
        </defs>
        <ellipse className="shadow-el" cx="12" cy="2.5" rx="6" ry="1.5" filter="url(#mascot-cursor-blur)" />
      </svg>
    </div>
  );
}
