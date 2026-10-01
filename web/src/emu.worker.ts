import init, { demo_program, Emu86 } from "./wasm/emu86_wasm.js";
import {
  MAX_PROGRAM_BYTES,
  RESET_SEG,
  RESET_OFF,
  type ProgramInfo,
  type Request,
} from "./protocol";

const ctx = self as unknown as Worker;

const DEMO_NAME = "countdown.bin";

interface LoadedProgram {
  name: string;
  bytes: Uint8Array;
}

let emu: Emu86 | null = null;
let program: LoadedProgram | null = null;

const ready = async () => {
  await init();
  if (emu === null) emu = new Emu86();
  return emu;
};

const programInfo = (): ProgramInfo | null =>
  program === null
    ? null
    : { name: program.name, size: program.bytes.length, seg: RESET_SEG, off: RESET_OFF };

const reply = (snapshot: unknown) =>
  ctx.postMessage({ ok: true, snapshot, program: programInfo() });
const fail = (error: string) => ctx.postMessage({ ok: false, error });

const resetAndLoad = (emu: Emu86) => {
  emu.reset();
  if (program !== null) reply(emu.load(RESET_SEG, RESET_OFF, program.bytes));
  else reply(emu.reset());
};

ctx.onmessage = async (e: MessageEvent) => {
  const req = e.data as Request;
  const emu = await ready();
  switch (req.type) {
    case "load":
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
      program = { name: req.name, bytes: req.bytes };
      resetAndLoad(emu);
      break;
    case "step":
      try {
        reply(emu.step());
      } catch (err) {
        fail(String(err));
      }
      break;
    case "reset":
      resetAndLoad(emu);
      break;
  }
};
