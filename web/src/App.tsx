import { useCallback } from "react";
import { useEmulator } from "./useEmulator";
import {
  Controls,
  ErrorLine,
  Execution,
  Flags,
  Halted,
  LastStep,
  MemoryInspector,
  Program,
  Registers,
} from "./Debugger";
import styles from "./App.module.css";

export default function App() {
  const { snapshot, prev, program, error, status, batch, send } = useEmulator();
  const hasProgram = snapshot !== null;

  const loadBinary = useCallback(
    (file: File) => {
      void file.arrayBuffer().then((buf) => {
        send({ type: "loadBinary", name: file.name, bytes: new Uint8Array(buf) });
      });
    },
    [send],
  );

  return (
    <main className={styles.app}>
      <h1>emu86</h1>
      <Controls
        onLoadDemo={() => send({ type: "load" })}
        onLoadBinary={loadBinary}
        onStep={() => send({ type: "step" })}
        onRun={() => send({ type: "run" })}
        onPause={() => send({ type: "pause" })}
        onReset={() => send({ type: "reset" })}
        status={status}
        hasProgram={hasProgram}
      />
      {snapshot !== null && (
        <>
          <Halted halted={snapshot.halted} />
          {program !== null && <Program info={program} />}
          <Execution snapshot={snapshot} />
          <Registers snapshot={snapshot} prev={prev} />
          <Flags snapshot={snapshot} prev={prev} />
          <MemoryInspector snapshot={snapshot} prev={prev} />
          <LastStep snapshot={snapshot} prev={prev} batch={batch} />
        </>
      )}
      <ErrorLine error={error} />
      {!hasProgram && (
        <>
          <p>Load a program to begin.</p>
          <p className={styles.note}>
            Load binary… takes a raw flat image (nasm -f bin output) and runs it at FFFF:0000 from
            reset state.
          </p>
        </>
      )}
    </main>
  );
}
