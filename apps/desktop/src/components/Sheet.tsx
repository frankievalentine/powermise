import { useEffect, type ReactNode } from "react";
import { AnimatePresence, motion } from "motion/react";

/** A modal sheet: dimming scrim, background pushed back, spring entrance that
 *  scales from slightly smaller and fades — and the same path on exit. */
export function Sheet({
  open,
  onDismiss,
  dismissible = true,
  labelledBy,
  children,
}: {
  open: boolean;
  onDismiss: () => void;
  dismissible?: boolean;
  labelledBy: string;
  children: ReactNode;
}) {
  useEffect(() => {
    if (!open || !dismissible) return;
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") onDismiss();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [open, dismissible, onDismiss]);

  return (
    <AnimatePresence>
      {open && (
        <motion.div
          className="scrim"
          initial={{ opacity: 0 }}
          animate={{ opacity: 1 }}
          exit={{ opacity: 0 }}
          transition={{ duration: 0.2 }}
          onMouseDown={(event) => {
            if (dismissible && event.target === event.currentTarget) onDismiss();
          }}
        >
          <motion.div
            className="sheet"
            role="dialog"
            aria-modal="true"
            aria-labelledby={labelledBy}
            style={{ transformOrigin: "top center" }}
            initial={{ opacity: 0, scale: 0.96, y: -8 }}
            animate={{ opacity: 1, scale: 1, y: 0 }}
            exit={{ opacity: 0, scale: 0.96, y: -8 }}
            transition={{ type: "spring", bounce: 0, duration: 0.35 }}
          >
            {children}
          </motion.div>
        </motion.div>
      )}
    </AnimatePresence>
  );
}
