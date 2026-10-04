import { useCallback, useEffect, useRef, useState } from "react";
import type { AnyResponse, ExecStatus, ProgramInfo, Request, Snapshot } from "./protocol";

export interface EmulatorState {
  snapshot: Snapshot | null;
  prev: Snapshot | null;
  program: ProgramInfo | null;
  error: string | null;
  status: ExecStatus;
  batch: boolean;
  send: (req: Request) => void;
}

export function useEmulator(): EmulatorState {
  const workerRef = useRef<Worker | null>(null);
  const currentRef = useRef<Snapshot | null>(null);
  const [snapshot, setSnapshot] = useState<Snapshot | null>(null);
  const [prev, setPrev] = useState<Snapshot | null>(null);
  const [program, setProgram] = useState<ProgramInfo | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [status, setStatus] = useState<ExecStatus>("paused");
  const [batch, setBatch] = useState(false);

  useEffect(() => {
    const worker = new Worker(new URL("./emu.worker.ts", import.meta.url), { type: "module" });
    workerRef.current = worker;
    worker.onmessage = (e: MessageEvent<AnyResponse>) => {
      const res = e.data;
      if (res.ok) {
        setPrev(currentRef.current);
        currentRef.current = res.snapshot;
        setSnapshot(res.snapshot);
        setProgram(res.program);
        setStatus(res.status);
        setBatch(res.batch);
        setError(null);
      } else {
        setError(res.error);
        setStatus("error");
      }
    };
    return () => worker.terminate();
  }, []);

  const send = useCallback((req: Request) => {
    // Loading or resetting replaces the program, so drop the comparison
    // baseline; Run/Pause only move between snapshots and keep it.
    if (req.type === "load" || req.type === "loadBinary" || req.type === "reset") {
      currentRef.current = null;
    }
    workerRef.current?.postMessage(req);
  }, []);

  return { snapshot, prev, program, error, status, batch, send };
}
