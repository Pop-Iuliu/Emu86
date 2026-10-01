cpu 8086
bits 16
org 0

	mov bx, 0x0020
	mov ax, 3
	mov cx, 0xFFFF
	mov [bx], ax

countdown:
	sub word [bx], byte 1
	cmp word [bx], byte 0
	jne countdown

	mov cx, [bx]
	hlt
