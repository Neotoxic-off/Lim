# Hex editing and reverse engineering

A file that is not UTF-8 opens as a hex dump, 16 bytes a line:
`00000010: 02 00 3e 00 68 65 6c 6c  6f ff 00 00 00 00 00 00  |..>.hello.......|`. Edit the hex digits between `:`
and `|` with any Vim key (`r`, `x`, `i`, `.`); bytes can be changed, inserted or deleted, and `:w` writes them back.
The offsets and the ASCII column are only for reading. A line that is not hex (an odd digit, a `z`) is refused with
E513 and the file is left as it was. `:hexedit` switches any buffer to this view and back.

| Command | Does |
|---|---|
| `:detect` | File type from its magic bytes (ELF, PE, Mach-O, DEX, wasm, zip, gzip, xz, zstd, 7z, PDF, PNG…), with bitness, endianness, architecture, DLL and .NET for executables, size and entropy; UPX is named, and entropy over 7.2 is flagged as packed, compressed or encrypted |
| `:strings` | Printable runs of 4+ chars, ASCII and UTF-16LE, with their offsets, in a new buffer |
| `:inspect` | The bytes at the cursor as u8/i8, u16/i16, u32/i32, u64/i64 (little / big endian), f32 and f64 |
| `:pack {type} {value}` | Writes a number at the cursor: `u8` `i8`, `u16` … `i64` with `le`/`be`, `f32le` `f64be`…; `0x` hex or decimal |
| `:[range]xor {key}` | XORs the bytes of the lines with a repeating key: `0x41ff` in hex, anything else as text |

`:inspect`, `:pack` and `:xor` work in a hex buffer; `:detect` and `:strings` on any buffer. A typical first look:

```
lim sample.exe        opens as hex
:detect               PE32+ executable x86-64 · 182272 bytes · entropy 7.61 · UPX packed
:strings              every string with its offset, searchable with / and the lines picker
<C-^>  then  /4d 5a   back to the dump, find bytes
:inspect              what the bytes at the cursor read as
:5,8xor 0x5a  :w      decode a block and save
```

Binaries up to 64 MB open (their dump takes about 7 times their size in memory); a 64 MB one opens in
under half a second, and jumping, scrolling and editing a byte stay under 3 ms. `:xor`, `:pack` and `:w` rebuild the whole
file, about 1 s at 64 MB. There is no disassembler.
