export interface Snapshot {
  ax: number;
  cx: number;
  dx: number;
  bx: number;
  sp: number;
  bp: number;
  si: number;
  di: number;
  es: number;
  cs: number;
  ss: number;
  ds: number;
  ip: number;
  flags: number;
  halted: boolean;
  linear_ip: number;
}

export interface Request {
  type: "load" | "step" | "reset";
}

export interface Response {
  ok: true;
  snapshot: Snapshot;
}

export interface ErrorResponse {
  ok: false;
  error: string;
}

export type AnyResponse = Response | ErrorResponse;

export const hex16 = (v: number): string => v.toString(16).toUpperCase().padStart(4, "0");
export const hex20 = (v: number): string => v.toString(16).toUpperCase().padStart(5, "0");

export const RESET_SEG = 0xffff;
export const RESET_OFF = 0x0000;
