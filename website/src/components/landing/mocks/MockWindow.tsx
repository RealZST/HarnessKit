import type { ReactNode, Ref } from "react";

interface Props {
  title: string; titleAside?: ReactNode; label?: string; className?: string; ref?: Ref<HTMLDivElement>;
  /** In the tab order: a looping picture holds still while it has focus (use-mock-cycle.ts), for keyboard readers. */
  focusable?: boolean;
  children: ReactNode;
}

// role="img" with a label: the window is a picture of the App, so assistive tech gets one sentence, not a table.
// Without a label the window holds real controls (the live Overview), so its body stays in the accessibility tree.
// `titleAside` sits at the end of the title row; it is hidden from the tree with the title, so a control that needs
// it points at it (aria-describedby still reads it).
export default function MockWindow({ title, titleAside, label, className, ref, focusable, children }: Props) {
  return (
    <div ref={ref} className={className ? `mockwin ${className}` : "mockwin"} role={label ? "img" : undefined} aria-label={label} tabIndex={focusable ? 0 : undefined}>
      <div className="mockwin-title" aria-hidden="true">{title}{titleAside}</div>
      <div className="mockwin-body" aria-hidden={label ? "true" : undefined}>{children}</div>
    </div>
  );
}
