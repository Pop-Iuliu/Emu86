use emu86_core::{Cpu, Snapshot, StepError};
use serde::Serialize;
use wasm_bindgen::prelude::*;

const DEMO_WINDOW_START: u32 = 0x0010;
const DEMO_WINDOW_LEN: usize = 32;
const INSN_WINDOW_LEN: usize = 6;

#[derive(Serialize)]
struct SnapshotView {
    ax: u16,
    cx: u16,
    dx: u16,
    bx: u16,
    sp: u16,
    bp: u16,
    si: u16,
    di: u16,
    es: u16,
    cs: u16,
    ss: u16,
    ds: u16,
    ip: u16,
    flags: u16,
    halted: bool,
    linear_ip: u32,
    demo_mem_start: u32,
    demo_mem: Vec<u8>,
    insn_bytes: Vec<u8>,
}

impl From<&Cpu> for SnapshotView {
    fn from(cpu: &Cpu) -> Self {
        let Snapshot {
            ax,
            cx,
            dx,
            bx,
            sp,
            bp,
            si,
            di,
            es,
            cs,
            ss,
            ds,
            ip,
            flags,
        } = cpu.snapshot();
        let linear_ip = ((cs as u32) << 4).wrapping_add(ip as u32) & 0xF_FFFF;
        Self {
            ax,
            cx,
            dx,
            bx,
            sp,
            bp,
            si,
            di,
            es,
            cs,
            ss,
            ds,
            ip,
            flags,
            halted: cpu.halted,
            linear_ip,
            demo_mem_start: DEMO_WINDOW_START,
            demo_mem: (0..DEMO_WINDOW_LEN)
                .map(|i| cpu.mem.read((DEMO_WINDOW_START + i as u32) as usize))
                .collect(),
            insn_bytes: (0..INSN_WINDOW_LEN)
                .map(|i| cpu.mem.read((linear_ip as usize).wrapping_add(i)))
                .collect(),
        }
    }
}

/// Why a bounded batch stopped.
#[derive(Serialize, PartialEq, Eq, Debug)]
#[serde(rename_all = "snake_case")]
enum BatchOutcome {
    /// The instruction budget was exhausted; the CPU is still runnable.
    Budget,
    /// The CPU executed HLT (or was already halted).
    Halted,
    /// An execution error occurred; see `error`.
    Error,
}

#[derive(Serialize)]
struct BatchView {
    snapshot: SnapshotView,
    outcome: BatchOutcome,
    error: Option<String>,
}

fn format_step_error(err: &StepError) -> String {
    match err {
        StepError::Halted => "halted".to_string(),
        StepError::UnknownOpcode(op) => format!("unknown opcode {op:#04x}"),
        StepError::UnsupportedForm { ip, bytes } => {
            format!("unsupported form at {ip:#06x}: {:02x?}", bytes)
        }
    }
}

/// Step the CPU up to `max_steps` instructions, stopping early on HLT or error.
///
/// This is the bounded loop the worker depends on to keep Run/Pause responsive:
/// it never runs past the budget, so the caller is free to yield between calls
/// and process a Pause or Reset message. Returns the reason it stopped, plus a
/// formatted message when that reason is an error.
fn run_steps(cpu: &mut Cpu, max_steps: u32) -> (BatchOutcome, Option<String>) {
    let mut steps = 0u32;
    while steps < max_steps {
        if cpu.halted {
            return (BatchOutcome::Halted, None);
        }
        match cpu.step() {
            Ok(()) => steps += 1,
            Err(StepError::Halted) => return (BatchOutcome::Halted, None),
            Err(err) => return (BatchOutcome::Error, Some(format_step_error(&err))),
        }
    }
    // The budget may run out on the very step that halted the CPU; HLT wins.
    if cpu.halted {
        (BatchOutcome::Halted, None)
    } else {
        (BatchOutcome::Budget, None)
    }
}

#[wasm_bindgen]
pub struct Emu86 {
    cpu: Cpu,
}

#[wasm_bindgen]
pub fn demo_program() -> Vec<u8> {
    include_bytes!("../../../tests/programs/countdown.bin").to_vec()
}

#[wasm_bindgen]
impl Emu86 {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self { cpu: Cpu::new() }
    }

    pub fn reset(&mut self) -> JsValue {
        self.cpu.reset();
        self.view()
    }

    pub fn load(&mut self, seg: u16, off: u16, bytes: &[u8]) -> JsValue {
        self.cpu.load_flat(seg, off, bytes);
        self.view()
    }

    pub fn step(&mut self) -> Result<JsValue, JsValue> {
        match self.cpu.step() {
            Ok(()) => Ok(self.view()),
            Err(err) => Err(format_step_error(&err).into()),
        }
    }

    /// Execute up to `max_steps` instructions, stopping early on HLT or error.
    /// Returns a batch view describing the resulting CPU state and why it stopped.
    pub fn run_batch(&mut self, max_steps: u32) -> JsValue {
        let (outcome, error) = run_steps(&mut self.cpu, max_steps);
        let view = BatchView {
            snapshot: SnapshotView::from(&self.cpu),
            outcome,
            error,
        };
        serde_wasm_bindgen::to_value(&view).unwrap_or(JsValue::NULL)
    }

    /// Current CPU state only, with no execution. Used for status-only updates.
    pub fn view(&self) -> JsValue {
        serde_wasm_bindgen::to_value(&SnapshotView::from(&self.cpu)).unwrap_or(JsValue::NULL)
    }
}

impl Default for Emu86 {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // `Cpu::new` sets CS = FFFF and IP = 0000, so loading at FFFF:0000 places
    // the program exactly where the emulator begins — the same convention the
    // worker and the CLI use.
    fn cpu_with(code: &[u8]) -> Cpu {
        let mut cpu = Cpu::new();
        cpu.load_flat(0xFFFF, 0x0000, code);
        cpu
    }

    #[test]
    fn budget_stops_an_infinite_loop() {
        // EB FE = jmp $ — the sprint's "intentional infinite loop". A bounded
        // batch must return with the guest still runnable, not block forever.
        let mut cpu = cpu_with(&[0xEB, 0xFE]);
        let (outcome, error) = run_steps(&mut cpu, 5);
        assert_eq!(outcome, BatchOutcome::Budget);
        assert!(error.is_none());
        assert!(
            !cpu.halted,
            "the guest is still runnable; only this batch ended"
        );
    }

    #[test]
    fn zero_budget_executes_nothing() {
        let mut cpu = cpu_with(&[0xEB, 0xFE]);
        let (outcome, _) = run_steps(&mut cpu, 0);
        assert_eq!(outcome, BatchOutcome::Budget);
        assert_eq!(cpu.ip, 0x0000);
    }

    #[test]
    fn halts_when_the_program_reaches_hlt() {
        // mov ax, 3 ; hlt
        let mut cpu = cpu_with(&[0xB8, 0x03, 0x00, 0xF4]);
        let (outcome, error) = run_steps(&mut cpu, 10);
        assert_eq!(outcome, BatchOutcome::Halted);
        assert!(error.is_none());
        assert!(cpu.halted);
    }

    #[test]
    fn hlt_wins_over_an_exhausted_budget() {
        // A budget of 1 finishes on the same step that HLT executes: the
        // outcome must be Halted, not Budget.
        let mut cpu = cpu_with(&[0xF4]);
        let (outcome, _) = run_steps(&mut cpu, 1);
        assert_eq!(outcome, BatchOutcome::Halted);
    }

    #[test]
    fn an_unhandled_opcode_stops_the_batch_with_an_error() {
        // 0F is outside the implemented instruction slice for this phase.
        let mut cpu = cpu_with(&[0x0F]);
        let (outcome, error) = run_steps(&mut cpu, 8);
        assert_eq!(outcome, BatchOutcome::Error);
        assert!(error.unwrap().contains("unknown opcode"));
    }
}
