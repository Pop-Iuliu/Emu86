; call — nested near calls pass arguments on the stack and clean up.
;
; main pushes two word arguments and calls `outer` (E8 rel16). `outer`
; calls `inner`, so two return addresses sit above the arguments.
; `inner` builds a BP frame (push bp / mov bp, sp), reads both
; arguments through [bp+6] and [bp+8], tears the frame down and RETs.
; `outer` then uses RET imm16 (C2) to pop its own return address and
; drop the two arguments main pushed.
;
; Final state: AX keeps its sentinel, DX/CX hold the arguments read
; from the stack, SP is back where main left it (0x0000), flags F002.

cpu 8086
bits 16
org 0

	mov ax, 0x0001
	mov bx, 0x00AA
	mov cx, 0x00BB
	push bx
	push cx
	call outer
	hlt

outer:
	call inner
	ret 4

inner:
	push bp
	mov bp, sp
	mov dx, [bp+6]
	mov cx, [bp+8]
	pop bp
	ret
