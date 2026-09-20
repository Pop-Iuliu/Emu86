import styles from "./App.module.css";
import { hex8, hex16, hex20, type Snapshot } from "./protocol";

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

const FLAG_INFO = [
  [0x0001, "CF", "Carry — unsigned result out of range (borrow on subtract)"],
  [0x0004, "PF", "Parity — low byte has an even number of 1 bits"],
  [0x0010, "AF", "Auxiliary carry — carry out of bit 3"],
  [0x0040, "ZF", "Zero — result is zero"],
  [0x0080, "SF", "Sign — result is negative (bit 15 set)"],
  [0x0100, "TF", "Trap — single-step interrupts"],
  [0x0200, "IF", "Interrupt enable"],
  [0x0400, "DF", "Direction — string operations run backward"],
  [0x0800, "OF", "Overflow — signed result out of range"],
] as const;

interface PanelProps {
  snapshot: Snapshot;
  prev: Snapshot | null;
}

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

function changedIn(prev: Snapshot | null, snapshot: Snapshot, get: (s: Snapshot) => number) {
  return prev !== null && get(prev) !== get(snapshot);
}

function Registers({ snapshot, prev }: PanelProps) {
  const changed = (get: (s: Snapshot) => number) => changedIn(prev, snapshot, get);
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
      </div>
    </section>
  );
}

function Execution({ snapshot }: { snapshot: Snapshot }) {
  const halted = snapshot.halted;
  const lastExecuted = (snapshot.linear_ip - 1) & 0xfffff;
  return (
    <section className={styles.panel}>
      <h2>{halted ? "Execution — halted" : "Execution — ready"}</h2>
      <div className={styles.execRow}>
        <span className={styles.name}>{halted ? "IP (past HLT)" : "next at CS:IP"}</span>
        <span className={styles.value}>
          {hex16(snapshot.cs)}:{hex16(snapshot.ip)}
        </span>
        <span className={styles.name}>physical</span>
        <span className={styles.value}>{hex20(snapshot.linear_ip)}</span>
      </div>
      <div className={styles.execRow}>
        <span className={styles.name}>{halted ? "bytes at IP" : "bytes"}</span>
        <span className={styles.value}>{snapshot.insn_bytes.map(hex8).join(" ")}</span>
      </div>
      {halted && (
        <>
          <div className={styles.execRow}>
            <span className={styles.name}>last executed</span>
            <span className={styles.value}>HLT at {hex20(lastExecuted)}</span>
          </div>
          <p className={styles.note}>
            IP advanced past HLT. No further instruction executes until reset and load.
          </p>
        </>
      )}
    </section>
  );
}

function Flags({ snapshot, prev }: PanelProps) {
  return (
    <section className={styles.panel}>
      <h2>Flags</h2>
      <div className={styles.flagRow}>
        {FLAG_INFO.map(([bit, label, title]) => {
          const on = (snapshot.flags & bit) !== 0;
          const wasOn = prev !== null && (prev.flags & bit) !== 0;
          return (
            <span
              key={label}
              title={title}
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
      <p className={styles.note}>
        CF is the unsigned carry/borrow; OF is the signed overflow of the same bit-15 arithmetic.
      </p>
    </section>
  );
}

function MemoryInspector({ snapshot, prev }: PanelProps) {
  const { demo_mem_start: start, demo_mem } = snapshot;
  const rows: number[][] = [];
  for (let i = 0; i < demo_mem.length; i += 16) {
    rows.push(demo_mem.slice(i, i + 16));
  }
  return (
    <section className={styles.panel}>
      <h2>Memory — demo data window</h2>
      {rows.map((row, r) => (
        <div key={r} className={styles.memRow}>
          <span className={styles.memAddr}>{hex20(start + r * 16)}</span>
          {row.map((b, i) => {
            const idx = r * 16 + i;
            const changed = prev !== null && prev.demo_mem[idx] !== b;
            return (
              <span
                key={idx}
                data-testid={`mem-byte-${start + idx}`}
                className={`${styles.memByte} ${changed ? styles.changed : ""}`}
              >
                {hex8(b)}
              </span>
            );
          })}
        </div>
      ))}
      <p className={styles.note}>demo data lives at DS:0020 (physical 00020)</p>
    </section>
  );
}

interface DiffRow {
  label: string;
  before: string;
  after: string;
}

function lastStepRows(snapshot: Snapshot, prev: Snapshot): DiffRow[] {
  const rows: DiffRow[] = [];
  for (const [key, label] of WORD_REGS) {
    if (snapshot[key] !== prev[key]) {
      rows.push({ label, before: hex16(prev[key]), after: hex16(snapshot[key]) });
    }
  }
  if (snapshot.flags !== prev.flags) {
    const bits = FLAG_INFO.filter(
      ([bit]) => (prev.flags & bit) !== (snapshot.flags & bit),
    )
      .map(([, label]) => label)
      .join(" ");
    rows.push({
      label: "FLAGS",
      before: hex16(prev.flags),
      after: bits === "" ? hex16(snapshot.flags) : `${hex16(snapshot.flags)} ${bits}`,
    });
  }
  for (let i = 0; i < snapshot.demo_mem.length; i++) {
    if (prev.demo_mem[i] !== snapshot.demo_mem[i]) {
      rows.push({
        label: `mem ${hex20(snapshot.demo_mem_start + i)}`,
        before: hex8(prev.demo_mem[i]),
        after: hex8(snapshot.demo_mem[i]),
      });
    }
  }
  return rows;
}

function LastStep({ snapshot, prev }: PanelProps) {
  const rows = prev === null ? [] : lastStepRows(snapshot, prev);
  const note =
    prev === null
      ? "Step to see what changed. Load and reset clear this list."
      : rows.length === 0
        ? "No displayed values changed by that step."
        : null;
  return (
    <section className={styles.panel}>
      <h2>Last step</h2>
      {note !== null ? (
        <p className={styles.note}>{note}</p>
      ) : (
        <ul className={styles.diffList} data-testid="last-step">
          {rows.map((d) => (
            <li key={d.label}>
              <span className={styles.diffLabel}>{d.label}</span>
              <span className={styles.diffValues}>
                {d.before} → {d.after}
              </span>
            </li>
          ))}
        </ul>
      )}
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

export {
  Controls,
  Execution,
  Flags,
  Halted,
  LastStep,
  MemoryInspector,
  Registers,
  ErrorLine,
};
