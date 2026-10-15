import type { Doctor, MiseInfo } from "./api";

export interface HealthSummary {
  /** Things that need attention: mise's own errors and warnings, an inactive
   *  shell, and a mise older than recommended. */
  count: number;
  /** `error` when something is broken (mise too old to support, or doctor
   *  errors); `warning` for everything else. */
  severity: "error" | "warning" | null;
}

/** What the Health screen reports, summarized for the sidebar badge. */
export function healthSummary(info: MiseInfo | null, doctor: Doctor | null): HealthSummary {
  const errors = (doctor?.errors.length ?? 0) + (info && !info.supported ? 1 : 0);
  const warnings =
    (doctor?.warnings.length ?? 0) +
    (doctor && !doctor.activated ? 1 : 0) +
    (info && info.supported && !info.recommendedMet ? 1 : 0);
  const count = errors + warnings;
  return { count, severity: errors ? "error" : warnings ? "warning" : null };
}
