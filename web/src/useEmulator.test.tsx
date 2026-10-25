import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { act, renderHook } from "@testing-library/react";
import { useEmulator } from "./useEmulator";
import type { AnyResponse, ExecStatus, Request, Snapshot } from "./protocol";

class FakeWorker {
  static instances: FakeWorker[] = [];
  onmessage: ((e: MessageEvent<AnyResponse>) => void) | null = null;
  posted: Request[] = [];
  terminated = false;

  constructor(_url: URL, _opts?: unknown) {
    FakeWorker.instances.push(this);
  }
  postMessage(req: Request) {
    this.posted.push(req);
  }
  terminate() {
    this.terminated = true;
  }
  emit(res: AnyResponse) {
    this.onmessage?.({ data: res } as MessageEvent<AnyResponse>);
  }
}

function snapshot(overrides: Partial<Snapshot> = {}): Snapshot {
  return {
    ax: 0, cx: 0, dx: 0, bx: 0,
    sp: 0, bp: 0, si: 0, di: 0,
    es: 0, cs: 0xffff, ss: 0, ds: 0,
    ip: 0x0000, flags: 0xf002, halted: false,
    linear_ip: 0xffff0,
    demo_mem_start: 0x0010,
    demo_mem: Array.from({ length: 32 }, () => 0),
    insn_bytes: [],
    ...overrides,
  };
}

function ok(overrides: Partial<Snapshot> = {}, status: ExecStatus = "paused"): AnyResponse {
  return {
    ok: true,
    snapshot: snapshot(overrides),
    program: { name: "countdown.bin", size: 22, seg: 0xffff, off: 0 },
    status,
    batch: false,
  };
}

beforeEach(() => {
  FakeWorker.instances = [];
  vi.stubGlobal("Worker", FakeWorker);
});

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("useEmulator", () => {
  it("keeps the previous snapshot so the debugger can diff", () => {
    const { result } = renderHook(() => useEmulator());
    const worker = FakeWorker.instances[0];
    act(() => worker.emit(ok({ ax: 0x0001 })));
    act(() => worker.emit(ok({ ax: 0x0002 })));
    expect(result.current.snapshot?.ax).toBe(0x0002);
    expect(result.current.prev?.ax).toBe(0x0001);
    expect(result.current.program?.name).toBe("countdown.bin");
  });

  it("drops the diff baseline when a load is requested", () => {
    const { result } = renderHook(() => useEmulator());
    const worker = FakeWorker.instances[0];
    act(() => worker.emit(ok({ ax: 0x0001 })));
    act(() => result.current.send({ type: "load" }));
    act(() => worker.emit(ok({ ax: 0x0009 })));
    expect(worker.posted).toEqual([{ type: "load" }]);
    expect(result.current.snapshot?.ax).toBe(0x0009);
    expect(result.current.prev).toBeNull();
  });

  it("surfaces the worker error and the status the worker reports", () => {
    const { result } = renderHook(() => useEmulator());
    const worker = FakeWorker.instances[0];
    act(() =>
      worker.emit({ ok: false, error: "program file is empty", status: "paused" }),
    );
    expect(result.current.error).toBe("program file is empty");
    expect(result.current.status).toBe("paused");
    expect(result.current.snapshot).toBeNull();
  });

  it("clears a stale error on the next successful response", () => {
    const { result } = renderHook(() => useEmulator());
    const worker = FakeWorker.instances[0];
    act(() => worker.emit({ ok: false, error: "boom", status: "error" }));
    act(() => worker.emit(ok()));
    expect(result.current.error).toBeNull();
  });

  it("terminates the worker on unmount", () => {
    const { unmount } = renderHook(() => useEmulator());
    const worker = FakeWorker.instances[0];
    unmount();
    expect(worker.terminated).toBe(true);
  });
});
