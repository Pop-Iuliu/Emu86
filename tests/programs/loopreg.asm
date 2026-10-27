cpu 8086
bits 16
org 0

	mov cx, 3
	mov ax, 0
top:
	add ax, 2
	loop top
	hlt
