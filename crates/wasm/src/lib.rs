use emu86_core::{Cpu, Snapshot, StepError};
use serde::Serialize;
use wasm_bindgen::prelude::*;

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
            linear_ip: ((cs as u32) << 4).wrapping_add(ip as u32) & 0xF_FFFF,
        }
    }
}

#[wasm_bindgen]
pub struct Emu86 {
    cpu: Cpu,
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
            Err(StepError::Halted) => Err("halted".into()),
            Err(StepError::UnknownOpcode(op)) => Err(format!("unknown opcode {op:#04x}").into()),
            Err(StepError::UnsupportedForm { ip, bytes }) => {
                Err(format!("unsupported form at {ip:#06x}: {:02x?}", bytes).into())
            }
        }
    }

    fn view(&self) -> JsValue {
        serde_wasm_bindgen::to_value(&SnapshotView::from(&self.cpu)).unwrap_or(JsValue::NULL)
    }
}

impl Default for Emu86 {
    fn default() -> Self {
        Self::new()
    }
}
