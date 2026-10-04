import init, { demo_program, Emu86 } from "./wasm/emu86_wasm.js";
import {
  MAX_PROGRAM_BYTES,
  RESET_SEG,
  RESET_OFF,
  RUN_BATCH_STEPS,
  type ExecStatus,
  type ProgramInfo,
  type Request,
  type Snapshot,
} from "./protocol";

const ctx = self as unknown as Worker;

const DEMO_NAME = "countdown.bin";

interface LoadedProgram {
  name: string;
  bytes: Uint8Array;
}

interface BatchResult {
  snapshot: Snapshot;
  outcome: "budget" | "halted" | "error";
  error?: string | null;
}

let emu: Emu86 | null = null;
let program: LoadedProgram | null = null;
let running = false;

const ready = async () => {
  await init();
  if (emu === null) emu = new Emu86();
  return emu;
};

const programInfo = (): ProgramInfo | null =>
  program === null
    ? null
    : { name: program.name, size: program.bytes.length, seg: RESET_SEG, off: RESET_OFF };

const reply = (snapshot: unknown, status: ExecStatus, batch: boolean) =>
  ctx.postMessage({ ok: true, snapshot, program: programInfo(), status, batch });
const fail = (error: string) => ctx.postMessage({ ok: false, error });

const resetAndLoad = (emu: Emu86) => {
  emu.reset();
  const snapshot = program !== null ? emu.load(RESET_SEG, RESET_OFF, program.bytes) : emu.reset();
  reply(snapshot, "paused", false);
};

// Schedule the next batch as a macrotask so `pause`/`reset` messages queued in
// the worker's event loop are handled *between* batches. This is the yield that
// keeps Pause responsive against a guest program that never halts.
const scheduleBatch = () => {
  setTimeout(runBatch, 0);
};

const runBatch = () => {
  if (!running || emu === null) return;
  const result = emu.run_batch(RUN_BATCH_STEPS) as unknown as BatchResult;

  if (result.outcome === "error") {
    running = false;
    fail(result.error ?? "execution error");
    return;
  }
  if (result.outcome === "halted") {
    running = false;
    reply(result.snapshot, "halted", true);
    return;
  }
  // Budget exhausted: publish one coherent snapshot for the batch.
  reply(result.snapshot, running ? "running" : "paused", true);
  if (running) scheduleBatch();
};

ctx.onmessage = async (e: MessageEvent) => {
  const req = e.data as Request;
  const emu = await ready();
  switch (req.type) {
    case "load":
      running = false;
      program = { name: DEMO_NAME, bytes: demo_program() };
      resetAndLoad(emu);
      break;
    case "loadBinary":
      if (req.bytes.length === 0) {
        fail("program file is empty — nothing to load");
        break;
      }
      if (req.bytes.length > MAX_PROGRAM_BYTES) {
        fail(
          `program file is ${req.bytes.length} bytes — the 8086 address space holds at most ${MAX_PROGRAM_BYTES}`,
        );
        break;
      }
      running = false;
      program = { name: req.name, bytes: req.bytes };
      resetAndLoad(emu);
      break;
    case "step":
      running = false;
      try {
        const snapshot = emu.step() as unknown as Snapshot;
        reply(snapshot, snapshot.halted ? "halted" : "paused", false);
      } catch (err) {
        fail(String(err));
      }
      break;
    case "run":
      if (running) break;
      running = true;
      reply(emu.view() as unknown as Snapshot, "running", true);
      scheduleBatch();
      break;
    case "pause":
      if (!running) break;
      running = false;
      reply(emu.view() as unknown as Snapshot, "paused", true);
      break;
    case "reset":
      running = false;
      resetAndLoad(emu);
      break;
  }
};
