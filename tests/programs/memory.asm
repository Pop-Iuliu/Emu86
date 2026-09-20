cpu 8086
bits 16
org 0

mov bx, 0x0020
mov ax, 0x0100
mov [bx], ax
add word [bx], 0x1000
sub word [bx], byte -1
mov cx, [bx]
hlt
