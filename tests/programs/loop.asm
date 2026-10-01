cpu 8086
bits 16
org 0

	mov cx, 3
	mov ax, 0
top:
	add ax, 2
	sub cx, 1
	cmp cx, 0
	jne top
	hlt
