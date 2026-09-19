import { useCallback, useEffect, useRef, useState } from "react";
import type { AnyResponse, Request, Snapshot } from "./protocol";

export interface EmulatorState {
  snapshot: Snapshot | null;
  prev: Snapshot | null;
  error: string | null;
  busy: boolean;
  send: (req: Request) => void;
}

export function useEmulator(): EmulatorState {
  const workerRef = useRef<Worker | null>(null);
  const currentRef = useRef<Snapshot | null>(null);
  const [snapshot, setSnapshot] = useState<Snapshot | null>(null);
  const [prev, setPrev] = useState<Snapshot | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(true);

  useEffect(() => {
    const worker = new Worker(new URL("./emu.worker.ts", import.meta.url), { type: "module" });
    workerRef.current = worker;
    worker.onmessage = (e: MessageEvent<AnyResponse>) => {
      const res = e.data;
      setBusy(false);
      if (res.ok) {
        setPrev(currentRef.current);
        currentRef.current = res.snapshot;
        setSnapshot(res.snapshot);
        setError(null);
      } else {
        setError(res.error);
      }
    };
    return () => worker.terminate();
  }, []);

  const send = useCallback((req: Request) => {
    setBusy(true);
    workerRef.current?.postMessage(req);
  }, []);

  return { snapshot, prev, error, busy, send };
}
