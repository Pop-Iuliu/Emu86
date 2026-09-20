import init, { demo_program, Emu86 } from "./wasm/emu86_wasm.js";
import { RESET_SEG, RESET_OFF } from "./protocol";

const ctx = self as unknown as Worker;

let emu: Emu86 | null = null;

const ready = async () => {
  await init();
  if (emu === null) emu = new Emu86();
  return emu;
};

const reply = (snapshot: unknown) => ctx.postMessage({ ok: true, snapshot });
const fail = (error: string) => ctx.postMessage({ ok: false, error });

ctx.onmessage = async (e: MessageEvent) => {
  const req = e.data as { type: string };
  const emu = await ready();
  switch (req.type) {
    case "load":
      emu.reset();
      reply(emu.load(RESET_SEG, RESET_OFF, demo_program()));
      break;
    case "step":
      try {
        reply(emu.step());
      } catch (err) {
        fail(String(err));
      }
      break;
    case "reset":
      reply(emu.reset());
      break;
  }
};
