import { useEmulator } from "./useEmulator";
import { Controls, ErrorLine, Flags, Halted, Registers } from "./Debugger";
import styles from "./App.module.css";

export default function App() {
  const { snapshot, prev, error, busy, send } = useEmulator();
  const hasProgram = snapshot !== null;

  return (
    <main className={styles.app}>
      <h1>emu86</h1>
      <Controls
        onLoad={() => send({ type: "load" })}
        onStep={() => send({ type: "step" })}
        onReset={() => send({ type: "reset" })}
        busy={busy}
        hasProgram={hasProgram}
        halted={snapshot?.halted === true}
      />
      {snapshot !== null && (
        <>
          <Halted halted={snapshot.halted} />
          <Registers snapshot={snapshot} prev={prev} />
          <Flags snapshot={snapshot} prev={prev} />
          <ErrorLine error={error} />
        </>
      )}
      {!hasProgram && <p>Load a program to begin.</p>}
    </main>
  );
}
