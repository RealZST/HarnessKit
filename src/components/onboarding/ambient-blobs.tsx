import { useEffect, useRef } from "react";

interface Blob {
  x: number;
  y: number;
  radius: number;
  color: [number, number, number];
  xFreq: number;
  yFreq: number;
  xPhase: number;
  yPhase: number;
  xAmp: number;
  yAmp: number;
  radiusFreq: number;
  radiusAmp: number;
}

const BLOBS: Blob[] = [
  {
    // Blue — Tiesen primary
    x: 0.4,
    y: 0.45,
    radius: 0.35,
    color: [30, 80, 220],
    xFreq: 0.15,
    yFreq: 0.12,
    xPhase: 0,
    yPhase: 0.5,
    xAmp: 0.08,
    yAmp: 0.06,
    radiusFreq: 0.1,
    radiusAmp: 0.03,
  },
  {
    // Orange — Claude primary
    x: 0.6,
    y: 0.55,
    radius: 0.3,
    color: [210, 120, 50],
    xFreq: 0.12,
    yFreq: 0.17,
    xPhase: 1.2,
    yPhase: 0.8,
    xAmp: 0.07,
    yAmp: 0.08,
    radiusFreq: 0.08,
    radiusAmp: 0.025,
  },
  {
    // Light highlight for depth
    x: 0.5,
    y: 0.4,
    radius: 0.18,
    color: [200, 200, 255],
    xFreq: 0.1,
    yFreq: 0.14,
    xPhase: 2.5,
    yPhase: 1.5,
    xAmp: 0.1,
    yAmp: 0.07,
    radiusFreq: 0.12,
    radiusAmp: 0.02,
  },
];

export function AmbientBlobs() {
  const canvasRef = useRef<HTMLCanvasElement>(null);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;

    const prefersReducedMotion = window.matchMedia(
      "(prefers-reduced-motion: reduce)",
    ).matches;

    const dpr = window.devicePixelRatio || 1;

    const resize = () => {
      canvas.width = window.innerWidth * dpr;
      canvas.height = window.innerHeight * dpr;
      canvas.style.width = `${window.innerWidth}px`;
      canvas.style.height = `${window.innerHeight}px`;
      ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    };
    resize();
    window.addEventListener("resize", resize);

    const W = () => window.innerWidth;
    const H = () => window.innerHeight;

    let raf: number;
    const start = performance.now();

    const draw = (now: number) => {
      const t = (now - start) / 1000;
      const w = W();
      const h = H();

      ctx.clearRect(0, 0, w, h);
      ctx.globalCompositeOperation = "screen";

      for (const blob of BLOBS) {
        const bx = prefersReducedMotion
          ? blob.x * w
          : (blob.x + Math.sin(t * blob.xFreq + blob.xPhase) * blob.xAmp) * w;
        const by = prefersReducedMotion
          ? blob.y * h
          : (blob.y + Math.cos(t * blob.yFreq + blob.yPhase) * blob.yAmp) * h;
        const br = prefersReducedMotion
          ? blob.radius * Math.min(w, h)
          : (blob.radius +
              Math.sin(t * blob.radiusFreq) * blob.radiusAmp) *
            Math.min(w, h);

        const gradient = ctx.createRadialGradient(bx, by, 0, bx, by, br);
        const [r, g, b] = blob.color;
        gradient.addColorStop(0, `rgba(${r}, ${g}, ${b}, 0.6)`);
        gradient.addColorStop(0.4, `rgba(${r}, ${g}, ${b}, 0.25)`);
        gradient.addColorStop(1, `rgba(${r}, ${g}, ${b}, 0)`);

        ctx.fillStyle = gradient;
        ctx.fillRect(0, 0, w, h);
      }

      ctx.globalCompositeOperation = "source-over";

      if (!prefersReducedMotion) {
        raf = requestAnimationFrame(draw);
      }
    };

    raf = requestAnimationFrame(draw);

    return () => {
      cancelAnimationFrame(raf);
      window.removeEventListener("resize", resize);
    };
  }, []);

  return (
    <canvas
      ref={canvasRef}
      className="pointer-events-none fixed inset-0"
      aria-hidden="true"
    />
  );
}
