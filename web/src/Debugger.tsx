import styles from "./App.module.css";
import { hex16, hex20, type Snapshot } from "./protocol";

const WORD_REGS = [
  ["ax", "AX"],
  ["cx", "CX"],
  ["dx", "DX"],
  ["bx", "BX"],
  ["sp", "SP"],
  ["bp", "BP"],
  ["si", "SI"],
  ["di", "DI"],
] as const;

const SEG_REGS = [
  ["es", "ES"],
  ["cs", "CS"],
  ["ss", "SS"],
  ["ds", "DS"],
] as const;

const FLAG_BITS = [
  [0x0001, "CF"],
  [0x0004, "PF"],
  [0x0010, "AF"],
  [0x0040, "ZF"],
  [0x0080, "SF"],
  [0x0100, "TF"],
  [0x0200, "IF"],
  [0x0400, "DF"],
  [0x0800, "OF"],
] as const;

interface ValueProps {
  name: string;
  value: number;
  format: (v: number) => string;
  changed: boolean;
}

function Value({ name, value, format, changed }: ValueProps) {
  return (
    <div
      className={`${styles.cell} ${changed ? styles.changed : ""}`}
      data-testid={`reg-${name.toLowerCase()}`}
    >
      <span className={styles.name}>{name}</span>
      <span className={styles.value}>{format(value)}</span>
    </div>
  );
}

interface PanelProps {
  snapshot: Snapshot;
  prev: Snapshot | null;
}

function Registers({ snapshot, prev }: PanelProps) {
  const changed = (get: (s: Snapshot) => number) => prev !== null && get(prev) !== get(snapshot);
  return (
    <section className={styles.panel}>
      <h2>Registers</h2>
      <div className={styles.grid}>
        {WORD_REGS.map(([key, label]) => (
          <Value
            key={key}
            name={label}
            value={snapshot[key]}
            format={hex16}
            changed={changed((s) => s[key])}
          />
        ))}
      </div>
      <div className={styles.grid}>
        {SEG_REGS.map(([key, label]) => (
          <Value
            key={key}
            name={label}
            value={snapshot[key]}
            format={hex16}
            changed={changed((s) => s[key])}
          />
        ))}
        <Value name="IP" value={snapshot.ip} format={hex16} changed={changed((s) => s.ip)} />
        <Value
          name="LIN"
          value={snapshot.linear_ip}
          format={hex20}
          changed={changed((s) => s.linear_ip)}
        />
      </div>
    </section>
  );
}

function Flags({ snapshot, prev }: PanelProps) {
  return (
    <section className={styles.panel}>
      <h2>Flags</h2>
      <div className={styles.flagRow}>
        {FLAG_BITS.map(([bit, label]) => {
          const on = (snapshot.flags & bit) !== 0;
          const wasOn = prev !== null && (prev.flags & bit) !== 0;
          return (
            <span
              key={label}
              className={`${styles.flag} ${on ? styles.flagOn : ""} ${
                prev !== null && on !== wasOn ? styles.changed : ""
              }`}
            >
              {label}
            </span>
          );
        })}
      </div>
      <div className={styles.flagWord}>FLAGS = {hex16(snapshot.flags)}</div>
    </section>
  );
}

interface ControlsProps {
  onLoad: () => void;
  onStep: () => void;
  onReset: () => void;
  busy: boolean;
  hasProgram: boolean;
  halted: boolean;
}

function Controls({ onLoad, onStep, onReset, busy, hasProgram, halted }: ControlsProps) {
  return (
    <div className={styles.controls}>
      <button onClick={onLoad}>Load</button>
      <button onClick={onStep} disabled={busy || !hasProgram || halted}>
        Step
      </button>
      <button onClick={onReset} disabled={busy || !hasProgram}>
        Reset
      </button>
    </div>
  );
}

function Halted({ halted }: { halted: boolean }) {
  return halted ? (
    <div className={styles.halted} data-testid="halted">
      HALTED
    </div>
  ) : null;
}

function ErrorLine({ error }: { error: string | null }) {
  return error !== null ? <div className={styles.error}>{error}</div> : null;
}

export { Controls, Halted, ErrorLine, Registers, Flags };
