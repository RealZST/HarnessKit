import { useCallback, useRef } from "react";

interface MascotSvgProps {
  size: number;
  clicked?: boolean;
}

const COLORS = ["#FFE432", "#FC413D", "#00B95C", "#3186FF", "#FBBC04", "#749BFF", "#FFEE48"];

export function AntigravityMascot({ size, clicked }: MascotSvgProps) {
  const particleRef = useRef<HTMLDivElement>(null);
  const prevClicked = useRef(false);

  const spawnPuff = useCallback((delay: number, sizeBase: number, distBase: number, blurAmt: number) => {
    const layer = particleRef.current;
    if (!layer) return;
    const el = document.createElement("div");
    const s = sizeBase + Math.random() * sizeBase;
    const color = COLORS[Math.floor(Math.random() * COLORS.length)];
    const isCenter = Math.random() < 0.3;
    const dist = isCenter ? Math.random() * distBase * 0.3 : distBase + Math.random() * distBase * 0.8;
    const x = isCenter ? (Math.random() - 0.5) * distBase * 0.5 : (Math.random() > 0.5 ? -dist : dist);
    const y = (Math.random() - 0.6) * 15;
    const rot = Math.random() * 60;
    const duration = 500 + Math.random() * 400;

    el.style.cssText = `position:absolute;border-radius:${1 + Math.random() * 2}px;opacity:0;width:${s}px;height:${s * (0.7 + Math.random() * 0.6)}px;background:${color};filter:blur(${blurAmt}px);transform:translate(0,0) rotate(${rot}deg) scale(0.3);`;
    layer.appendChild(el);

    setTimeout(() => {
      el.style.transition = `all ${duration}ms cubic-bezier(0.2,0.8,0.3,1)`;
      el.style.opacity = "0.9";
      el.style.transform = `translate(${x}px,${y}px) rotate(${rot + 20}deg) scale(1)`;
      setTimeout(() => {
        el.style.transition = `all ${duration * 0.6}ms ease-out`;
        el.style.opacity = "0";
        el.style.transform = `translate(${x * 1.4}px,${y * 1.3}px) rotate(${rot + 40}deg) scale(1.8)`;
      }, duration * 0.5);
      setTimeout(() => el.remove(), duration * 1.2);
    }, delay);
  }, []);

  const launchExplosion = useCallback(() => {
    for (let i = 0; i < 8; i++) spawnPuff(i * 50, 4, 15, 1);
    for (let i = 0; i < 20; i++) spawnPuff(280 + i * 12, 6, 30, 1.5);
    for (let i = 0; i < 35; i++) spawnPuff(320 + i * 8, 8, 45, 2);
    for (let i = 0; i < 20; i++) spawnPuff(400 + i * 15, 10, 35, 3);
    for (let i = 0; i < 15; i++) spawnPuff(600 + i * 20, 3, 50, 0.5);
  }, [spawnPuff]);

  // Trigger explosion when clicked transitions to true
  if (clicked && !prevClicked.current) {
    // Use setTimeout(0) to run after render
    setTimeout(() => launchExplosion(), 0);
  }
  prevClicked.current = !!clicked;

  return (
    <div style={{ position: "relative", width: size, height: size, display: "flex", justifyContent: "center", alignItems: "center" }}>
      {/* Particle layer */}
      <div
        ref={particleRef}
        style={{ position: "absolute", top: "68%", left: "50%", width: 0, height: 0, zIndex: 1, pointerEvents: "none" }}
      />
      {/* Icon */}
      <div className="ag-icon-wrapper">
        <svg viewBox="0 0 24 24" xmlns="http://www.w3.org/2000/svg" width={size} height={size}>
          <mask height="23" id="ag-mask" maskUnits="userSpaceOnUse" width="24" x="0" y="1">
            <path d="M21.751 22.607c1.34 1.005 3.35.335 1.508-1.508C17.73 15.74 18.904 1 12.037 1 5.17 1 6.342 15.74.815 21.1c-2.01 2.009.167 2.511 1.507 1.506 5.192-3.517 4.857-9.714 9.715-9.714 4.857 0 4.522 6.197 9.714 9.715z" fill="#fff" />
          </mask>
          <g mask="url(#ag-mask)">
            <g filter="url(#ag-f1)"><path d="M-1.018-3.992c-.408 3.591 2.686 6.89 6.91 7.37 4.225.48 7.98-2.043 8.387-5.633.408-3.59-2.686-6.89-6.91-7.37-4.225-.479-7.98 2.043-8.387 5.633z" fill="#FFE432" /></g>
            <g filter="url(#ag-f2)"><path d="M15.269 7.747c1.058 4.557 5.691 7.374 10.348 6.293 4.657-1.082 7.575-5.653 6.516-10.21-1.058-4.556-5.691-7.374-10.348-6.292-4.657 1.082-7.575 5.653-6.516 10.21z" fill="#FC413D" /></g>
            <g filter="url(#ag-f3)"><path d="M-12.443 10.804c1.338 4.703 7.36 7.11 13.453 5.378 6.092-1.733 9.947-6.95 8.61-11.652C8.282-.173 2.26-2.58-3.833-.848-9.925.884-13.78 6.1-12.443 10.804z" fill="#00B95C" /></g>
            <g filter="url(#ag-f4)"><path d="M-12.443 10.804c1.338 4.703 7.36 7.11 13.453 5.378 6.092-1.733 9.947-6.95 8.61-11.652C8.282-.173 2.26-2.58-3.833-.848-9.925.884-13.78 6.1-12.443 10.804z" fill="#00B95C" /></g>
            <g filter="url(#ag-f5)"><path d="M-7.608 14.703c3.352 3.424 9.126 3.208 12.896-.483 3.77-3.69 4.108-9.459.756-12.883C2.69-2.087-3.083-1.871-6.853 1.82c-3.77 3.69-4.108 9.458-.755 12.883z" fill="#00B95C" /></g>
            <g filter="url(#ag-f6)"><path d="M9.932 27.617c1.04 4.482 5.384 7.303 9.7 6.3 4.316-1.002 6.971-5.448 5.93-9.93-1.04-4.483-5.384-7.304-9.7-6.301-4.316 1.002-6.971 5.448-5.93 9.93z" fill="#3186FF" /></g>
            <g filter="url(#ag-f7)"><path d="M2.572-8.185C.392-3.329 2.778 2.472 7.9 4.771c5.122 2.3 11.042.227 13.222-4.63 2.18-4.855-.205-10.656-5.327-12.955-5.122-2.3-11.042-.227-13.222 4.63z" fill="#FBBC04" /></g>
            <g filter="url(#ag-f8)"><path d="M-3.267 38.686c-5.277-2.072 3.742-19.117 5.984-24.83 2.243-5.712 8.34-8.664 13.616-6.592 5.278 2.071 11.533 13.482 9.29 19.195-2.242 5.713-23.613 14.298-28.89 12.227z" fill="#3186FF" /></g>
            <g filter="url(#ag-f9)"><path d="M28.71 17.471c-1.413 1.649-5.1.808-8.236-1.878-3.135-2.687-4.531-6.201-3.118-7.85 1.412-1.649 5.1-.808 8.235 1.878s4.532 6.2 3.119 7.85z" fill="#749BFF" /></g>
            <g filter="url(#ag-f10)"><path d="M18.163 9.077c5.81 3.93 12.502 4.19 14.946.577 2.443-3.612-.287-9.727-6.098-13.658-5.81-3.931-12.502-4.19-14.946-.577-2.443 3.612.287 9.727 6.098 13.658z" fill="#FC413D" /></g>
            <g filter="url(#ag-f11)"><path d="M-.915 2.684c-1.44 3.473-.97 6.967 1.05 7.804 2.02.837 4.824-1.3 6.264-4.772 1.44-3.473.97-6.967-1.05-7.804-2.02-.837-4.824 1.3-6.264 4.772z" fill="#FFEE48" /></g>
          </g>
          <defs>
            <filter colorInterpolationFilters="sRGB" filterUnits="userSpaceOnUse" height="17.587" id="ag-f1" width="19.838" x="-3.288" y="-11.917"><feFlood floodOpacity="0" result="bg" /><feBlend in="SourceGraphic" in2="bg" result="shape" /><feGaussianBlur result="e" stdDeviation="1.117" /></filter>
            <filter colorInterpolationFilters="sRGB" filterUnits="userSpaceOnUse" height="38.565" id="ag-f2" width="38.9" x="4.251" y="-13.493"><feFlood floodOpacity="0" result="bg" /><feBlend in="SourceGraphic" in2="bg" result="shape" /><feGaussianBlur result="e" stdDeviation="5.4" /></filter>
            <filter colorInterpolationFilters="sRGB" filterUnits="userSpaceOnUse" height="36.517" id="ag-f3" width="40.955" x="-21.889" y="-10.592"><feFlood floodOpacity="0" result="bg" /><feBlend in="SourceGraphic" in2="bg" result="shape" /><feGaussianBlur result="e" stdDeviation="4.591" /></filter>
            <filter colorInterpolationFilters="sRGB" filterUnits="userSpaceOnUse" height="36.517" id="ag-f4" width="40.955" x="-21.889" y="-10.592"><feFlood floodOpacity="0" result="bg" /><feBlend in="SourceGraphic" in2="bg" result="shape" /><feGaussianBlur result="e" stdDeviation="4.591" /></filter>
            <filter colorInterpolationFilters="sRGB" filterUnits="userSpaceOnUse" height="36.595" id="ag-f5" width="36.632" x="-19.099" y="-10.278"><feFlood floodOpacity="0" result="bg" /><feBlend in="SourceGraphic" in2="bg" result="shape" /><feGaussianBlur result="e" stdDeviation="4.591" /></filter>
            <filter colorInterpolationFilters="sRGB" filterUnits="userSpaceOnUse" height="34.087" id="ag-f6" width="33.533" x=".981" y="8.758"><feFlood floodOpacity="0" result="bg" /><feBlend in="SourceGraphic" in2="bg" result="shape" /><feGaussianBlur result="e" stdDeviation="4.363" /></filter>
            <filter colorInterpolationFilters="sRGB" filterUnits="userSpaceOnUse" height="35.276" id="ag-f7" width="35.978" x="-6.143" y="-21.659"><feFlood floodOpacity="0" result="bg" /><feBlend in="SourceGraphic" in2="bg" result="shape" /><feGaussianBlur result="e" stdDeviation="3.954" /></filter>
            <filter colorInterpolationFilters="sRGB" filterUnits="userSpaceOnUse" height="46.523" id="ag-f8" width="45.114" x="-11.96" y="-.46"><feFlood floodOpacity="0" result="bg" /><feBlend in="SourceGraphic" in2="bg" result="shape" /><feGaussianBlur result="e" stdDeviation="3.531" /></filter>
            <filter colorInterpolationFilters="sRGB" filterUnits="userSpaceOnUse" height="24.054" id="ag-f9" width="25.094" x="10.485" y=".58"><feFlood floodOpacity="0" result="bg" /><feBlend in="SourceGraphic" in2="bg" result="shape" /><feGaussianBlur result="e" stdDeviation="3.159" /></filter>
            <filter colorInterpolationFilters="sRGB" filterUnits="userSpaceOnUse" height="30.007" id="ag-f10" width="33.508" x="5.833" y="-12.467"><feFlood floodOpacity="0" result="bg" /><feBlend in="SourceGraphic" in2="bg" result="shape" /><feGaussianBlur result="e" stdDeviation="2.669" /></filter>
            <filter colorInterpolationFilters="sRGB" filterUnits="userSpaceOnUse" height="26.151" id="ag-f11" width="22.194" x="-8.355" y="-8.876"><feFlood floodOpacity="0" result="bg" /><feBlend in="SourceGraphic" in2="bg" result="shape" /><feGaussianBlur result="e" stdDeviation="3.303" /></filter>
          </defs>
        </svg>
      </div>
    </div>
  );
}
