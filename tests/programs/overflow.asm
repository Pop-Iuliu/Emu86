cpu 8086
bits 16
org 0

mov ax, 0x7FFF
add ax, 1
hlt
