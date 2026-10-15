import { useState, type ReactNode } from "react";
import { Check, Copy } from "lucide-react";
import { PLATFORMS, PLATFORM_LABEL, type Platform, type ToolProvider } from "../api";
import { PROVIDER_LABEL, monogramColors, monogramLetters } from "../catalog";

export function Monogram({ id, title, size = 36 }: { id: string; title: string; size?: number }) {
  const [from, to] = monogramColors(id);
  return (
    <div
      className="monogram"
      aria-hidden
      style={{
        width: size,
        height: size,
        fontSize: size * 0.4,
        background: `linear-gradient(180deg, ${from}, ${to})`,
      }}
    >
      {monogramLetters(title)}
    </div>
  );
}

export function PlatformChips({ supported }: { supported: Platform[] }) {
  return (
    <span className="platforms">
      {PLATFORMS.map((platform) => {
        const on = supported.includes(platform);
        return (
          <span
            key={platform}
            className={`platform-chip${on ? "" : " off"}`}
            title={`${PLATFORM_LABEL[platform]}: ${on ? "supported" : "not supported"}`}
          >
            {PLATFORM_LABEL[platform]}
          </span>
        );
      })}
    </span>
  );
}

export function ProviderBadge({
  provider,
}: {
  provider: (Pick<ToolProvider, "kind" | "version"> & { package?: string; key?: string }) | undefined;
}) {
  if (!provider) return <span className="badge red">Not available</span>;
  return (
    <span className="badge" title={`${provider.key ?? provider.package ?? ""} ${provider.version}`.trim()}>
      {PROVIDER_LABEL[provider.kind]}
    </span>
  );
}

/** A shell command shown verbatim with a copy button — the app's way of never
 *  hiding what it runs. */
export function CommandBlock({ command }: { command: string }) {
  const [copied, setCopied] = useState(false);
  const copy = async () => {
    try {
      await navigator.clipboard.writeText(command);
      setCopied(true);
      setTimeout(() => setCopied(false), 1400);
    } catch {
      // Clipboard can be unavailable (permissions); the text stays selectable.
    }
  };
  return (
    <div className="command">
      <span className="prompt">$</span>
      <code>{command}</code>
      <button className="icon-button" onClick={copy} aria-label="Copy command" title="Copy">
        {copied ? <Check size={14} /> : <Copy size={14} />}
      </button>
    </div>
  );
}

export function Switch({
  checked,
  onChange,
  label,
}: {
  checked: boolean;
  onChange: (checked: boolean) => void;
  label: string;
}) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={checked}
      aria-label={label}
      className="switch"
      onClick={() => onChange(!checked)}
    />
  );
}

export function Checkbox({ checked }: { checked: boolean }) {
  return (
    <span className={`checkbox${checked ? " checked" : ""}`} aria-hidden>
      {checked && <Check size={12} strokeWidth={3} />}
    </span>
  );
}

export function Segmented<T extends string>({
  value,
  options,
  onChange,
  label,
}: {
  value: T;
  options: { value: T; label: string }[];
  onChange: (value: T) => void;
  label: string;
}) {
  return (
    <div className="segmented" role="group" aria-label={label}>
      {options.map((option) => (
        <button
          key={option.value}
          type="button"
          aria-pressed={option.value === value}
          onClick={() => onChange(option.value)}
        >
          {option.label}
        </button>
      ))}
    </div>
  );
}

export function Section({
  title,
  note,
  children,
}: {
  title: string;
  note?: ReactNode;
  children: ReactNode;
}) {
  return (
    <section className="section">
      <h2 className="section-title">{title}</h2>
      {note && <p className="section-note">{note}</p>}
      {children}
    </section>
  );
}

/** Renders a unified diff with added/removed lines colored. */
export function DiffView({ diff }: { diff: string }) {
  return (
    <pre className="code-view">
      {diff.split("\n").map((line, index) => {
        const kind = line.startsWith("+++") || line.startsWith("---")
          ? "meta"
          : line.startsWith("+")
            ? "add"
            : line.startsWith("-")
              ? "del"
              : "";
        return (
          <div key={index} className={kind}>
            {line || " "}
          </div>
        );
      })}
    </pre>
  );
}
