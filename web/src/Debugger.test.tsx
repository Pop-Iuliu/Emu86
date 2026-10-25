import { describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";
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
import type { Snapshot } from "./protocol";

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

describe("Registers", () => {
  it("renders a word register as hex", () => {
    render(<Registers snapshot={snapshot({ ax: 0x1234 })} prev={null} />);
    expect(screen.getByTestId("reg-ax")).toHaveTextContent("1234");
  });

  it("marks only the registers that differ from the previous snapshot", () => {
    const prev = snapshot({ ax: 0x0001, bx: 0x0002 });
    render(<Registers snapshot={snapshot({ ax: 0x0003, bx: 0x0002 })} prev={prev} />);
    expect(screen.getByTestId("reg-ax")).toHaveClass("changed");
    expect(screen.getByTestId("reg-bx")).not.toHaveClass("changed");
  });
});

describe("Flags", () => {
  it("shows the flag word and lights the flags that are set", () => {
    render(<Flags snapshot={snapshot({ flags: 0xf046 })} prev={null} />);
    expect(screen.getByText("FLAGS = F046")).toBeInTheDocument();
    expect(screen.getByText("PF")).toHaveClass("flagOn");
    expect(screen.getByText("ZF")).toHaveClass("flagOn");
    expect(screen.getByText("CF")).not.toHaveClass("flagOn");
  });
});

describe("LastStep", () => {
  it("lists only the register and flag values that changed", () => {
    const prev = snapshot({ ax: 0x0001, flags: 0xf002 });
    const next = snapshot({ ax: 0x0002, flags: 0xf046 });
    render(<LastStep snapshot={next} prev={prev} batch={false} />);
    expect(screen.getByText("AX")).toBeInTheDocument();
    expect(screen.getByText("0001 → 0002")).toBeInTheDocument();
    expect(screen.getByText("F002 → F046 PF ZF")).toBeInTheDocument();
    expect(screen.queryByText("BX")).toBeNull();
  });

  it("reports bytes that changed inside the data window", () => {
    const mem = Array.from({ length: 32 }, () => 0);
    mem[5] = 0xff;
    render(
      <LastStep snapshot={snapshot({ demo_mem: mem })} prev={snapshot()} batch={false} />,
    );
    expect(screen.getByText("mem 00015")).toBeInTheDocument();
    expect(screen.getByText("00 → FF")).toBeInTheDocument();
  });

  it("says only the execution address moved when nothing visible changed", () => {
    render(
      <LastStep
        snapshot={snapshot({ ip: 0x0005 })}
        prev={snapshot({ ip: 0x0002 })}
        batch={false}
      />,
    );
    expect(
      screen.getByText("No register, flag, or visible memory changes. Execution address updated."),
    ).toBeInTheDocument();
    expect(screen.queryByTestId("last-step")).toBeNull();
  });

  it("prompts differently for a single step and a batch", () => {
    const { rerender } = render(<LastStep snapshot={snapshot()} prev={null} batch={false} />);
    expect(screen.getByText(/^Step to see what changed/)).toBeInTheDocument();
    rerender(<LastStep snapshot={snapshot()} prev={null} batch />);
    expect(screen.getByText(/^Run to see what changed/)).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "Changes since last update" })).toBeInTheDocument();
  });
});

describe("MemoryInspector", () => {
  it("lays the window out as rows of sixteen bytes with per-byte testids", () => {
    const mem = Array.from({ length: 32 }, (_, i) => i);
    render(<MemoryInspector snapshot={snapshot({ demo_mem: mem })} prev={null} />);
    expect(screen.getByText("00010")).toBeInTheDocument();
    expect(screen.getByText("00020")).toBeInTheDocument();
    expect(screen.getByTestId("mem-byte-16")).toHaveTextContent("00");
    expect(screen.getByTestId("mem-byte-47")).toHaveTextContent("1F");
  });

  it("marks a byte that changed", () => {
    const mem = Array.from({ length: 32 }, () => 0);
    const next = mem.slice();
    next[5] = 0xff;
    render(
      <MemoryInspector
        snapshot={snapshot({ demo_mem: next })}
        prev={snapshot({ demo_mem: mem })}
      />,
    );
    expect(screen.getByTestId("mem-byte-21")).toHaveClass("changed");
    expect(screen.getByTestId("mem-byte-20")).not.toHaveClass("changed");
  });
});

describe("Execution", () => {
  it("shows the last and next execution addresses plus the fetched bytes", () => {
    const prev = snapshot({ ip: 0x000e, linear_ip: 0xffffe });
    const next = snapshot({ ip: 0x0011, linear_ip: 0x00001, insn_bytes: [0xbb, 0x20] });
    render(<Execution snapshot={next} prev={prev} />);
    expect(screen.getByTestId("exec-last")).toHaveTextContent("FFFF:000E");
    expect(screen.getByTestId("exec-last")).toHaveTextContent("FFFFE");
    expect(screen.getByTestId("exec-next")).toHaveTextContent("FFFF:0011");
    expect(screen.getByTestId("exec-next")).toHaveTextContent("00001");
    expect(screen.getByText("BB 20")).toBeInTheDocument();
  });

  it("labels itself halted and explains that IP advanced past HLT", () => {
    render(<Execution snapshot={snapshot({ halted: true })} prev={null} />);
    expect(screen.getByRole("heading", { name: "Execution — halted" })).toBeInTheDocument();
    expect(screen.getByText(/IP advanced past HLT/)).toBeInTheDocument();
  });
});

describe("Controls", () => {
  const handlers = () => ({
    onLoadDemo: vi.fn(),
    onLoadBinary: vi.fn(),
    onStep: vi.fn(),
    onRun: vi.fn(),
    onPause: vi.fn(),
    onReset: vi.fn(),
  });

  it("disables the execution controls until a program exists", () => {
    render(<Controls {...handlers()} status="paused" hasProgram={false} />);
    expect(screen.getByRole("button", { name: "Step" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "Reset" })).toBeDisabled();
    expect(screen.queryByTestId("status")).toBeNull();
  });

  it("keeps Step focusable once halted but refuses to step", () => {
    const onStep = vi.fn();
    render(<Controls {...handlers()} onStep={onStep} status="halted" hasProgram />);
    const step = screen.getByRole("button", { name: "Step" });
    expect(step).not.toHaveAttribute("disabled");
    expect(step).toHaveAttribute("aria-disabled", "true");
    fireEvent.click(step);
    expect(onStep).not.toHaveBeenCalled();
  });

  it("offers Pause and withholds Run while running", () => {
    render(<Controls {...handlers()} status="running" hasProgram />);
    expect(screen.getByRole("button", { name: "Run" })).toHaveAttribute("aria-disabled", "true");
    expect(screen.getByRole("button", { name: "Pause" })).toHaveAttribute("aria-disabled", "false");
    expect(screen.getByTestId("status")).toHaveTextContent("running");
    expect(screen.getByTestId("status")).toHaveAttribute("data-status", "running");
  });

  it("hands the picked file to the loader and clears the input", () => {
    const onLoadBinary = vi.fn();
    render(<Controls {...handlers()} onLoadBinary={onLoadBinary} status="paused" hasProgram />);
    const input = screen.getByTestId("binary-input") as HTMLInputElement;
    const file = new File([new Uint8Array([0xf4])], "hlt.bin");
    fireEvent.change(input, { target: { files: [file] } });
    expect(onLoadBinary).toHaveBeenCalledWith(file);
  });
});

describe("Program, Halted and ErrorLine", () => {
  it("shows the program name, size and load address", () => {
    render(<Program info={{ name: "countdown.bin", size: 22, seg: 0xffff, off: 0 }} />);
    expect(screen.getByTestId("program-info")).toHaveTextContent("countdown.bin");
    expect(screen.getByTestId("program-info")).toHaveTextContent("22 bytes");
    expect(screen.getByTestId("program-info")).toHaveTextContent("FFFF:0000");
  });

  it("renders the halt banner and error line only when present", () => {
    const { rerender } = render(<Halted halted={false} />);
    expect(screen.queryByTestId("halted")).toBeNull();
    rerender(<Halted halted />);
    expect(screen.getByTestId("halted")).toHaveTextContent("HALTED");
    rerender(<ErrorLine error="program file is empty" />);
    expect(screen.getByText("program file is empty")).toBeInTheDocument();
  });
});
