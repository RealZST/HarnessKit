// OpenClaw mascot — a stylized lobster claw (the project's 🦞 identity) in
// brand red-orange, rendered bare so it reads on both light and dark cards.
// Static for now; motion can join the animated family in a follow-up.

interface MascotSvgProps {
  size: number;
}

export function OpenClawMascot({ size }: MascotSvgProps) {
  return (
    <svg
      viewBox="0 0 24 24"
      xmlns="http://www.w3.org/2000/svg"
      width={size}
      height={size}
      style={{ overflow: "visible" }}
    >
      {/* upper pincer: crescent with a wedge cut toward the tip */}
      <path
        fill="#ef4f2c"
        d="M12.6 2.2a9.4 9.4 0 1 0 6.6 16.1l-2.6-3.2a5.4 5.4 0 1 1-4-8.9z"
      />
      {/* lower jaw closing against it, leaving the claw gap */}
      <path
        fill="#c93a1d"
        d="M20.9 15.1c.8 1.6.5 3.6-.9 5-1.5 1.5-3.7 1.7-5.4.7l1.9-3.7c.7.4 1.6.3 2.2-.3.6-.6.7-1.4.4-2.1z"
      />
      {/* joint bead at the base of the claw */}
      <circle cx="5.6" cy="18.6" r="2.1" fill="#ef4f2c" />
    </svg>
  );
}
