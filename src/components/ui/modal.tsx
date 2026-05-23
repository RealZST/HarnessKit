import { useRef } from "react";
import { createPortal } from "react-dom";
import { useEscape } from "@/hooks/use-escape";
import { useFocusTrap } from "@/hooks/use-focus-trap";

interface ModalProps {
  onClose(): void;
  children: React.ReactNode;
  /** Class on the dialog container (controls width/height/rounded/shadow). */
  containerClassName?: string;
  /** Class on the backdrop wrapper (z-index + alignment + background). */
  backdropClassName?: string;
  /** When true (default) ESC closes the modal. */
  closeOnEscape?: boolean;
  /** When true (default) clicking outside the dialog closes the modal. */
  closeOnBackdropClick?: boolean;
  /** Suppress close interactions (e.g. during async submit). */
  busy?: boolean;
  ariaLabelledBy?: string;
  ariaLabel?: string;
}

const DEFAULT_BACKDROP =
  "fixed inset-0 z-50 flex items-center justify-center bg-black/40";
const DEFAULT_CONTAINER =
  "flex max-h-[90vh] w-[480px] flex-col rounded-xl border border-border bg-background shadow-xl";

/** Portal-rendered modal shell. Handles backdrop, focus trap, ESC, and
 *  backdrop-click-to-close. Renders children directly inside the dialog
 *  container so each consumer controls its own header/body/footer. */
export function Modal({
  onClose,
  children,
  containerClassName = DEFAULT_CONTAINER,
  backdropClassName = DEFAULT_BACKDROP,
  closeOnEscape = true,
  closeOnBackdropClick = true,
  busy = false,
  ariaLabelledBy,
  ariaLabel,
}: ModalProps) {
  const dlgRef = useRef<HTMLDivElement>(null);
  useFocusTrap(dlgRef, true);
  useEscape(onClose, closeOnEscape && !busy);

  return createPortal(
    <div
      className={backdropClassName}
      onClick={(e) => {
        if (e.target === e.currentTarget && closeOnBackdropClick && !busy)
          onClose();
      }}
    >
      <div
        ref={dlgRef}
        role="dialog"
        aria-modal="true"
        aria-labelledby={ariaLabelledBy}
        aria-label={ariaLabel}
        tabIndex={-1}
        className={containerClassName}
      >
        {children}
      </div>
    </div>,
    document.body,
  );
}
