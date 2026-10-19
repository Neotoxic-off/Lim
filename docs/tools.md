# Tools

`<Leader>x` opens the tools menu (which-key shows it; the cheat sheet documents every syntax under "Tools").
For the `:` text tools (`:base64`, `:json`, `:case`…), see [Vim editing › Text tools](vim.md#text-tools).

The calculator, converter, bases, time and color tools are **windows**: each opens once, split below, on its own
buffer (`[Calculator]`, `[Convert]`, `[Bases]`, `[Time]`, `[Color]`), in insert mode. Type a line and `<CR>`: the
answer lands on the line below (`= 152.4 cm`, or `! why` when it is wrong) and a fresh line waits. The buffer is
the history: scroll it, search it, yank from it, edit an old question and `<CR>` on it (in insert or normal mode)
to answer it again in place. Each answer also goes to the `"` register, so `p` in your code pastes it. `:q`
closes the window without asking to save; the same key brings it back with everything still in it.

```
2+3*4
= 14
ans^2
= 196
sqrt(-1)
! undefined result (like sqrt(-1), log(0) or 0/0)
```

| Keys | Tool | Examples |
|---|---|---|
| `<Leader>xc` | Calculator | `2+3*4` · `2pi` · `3(4+1)` · `2^10` · `200*15%` · `10%3` · `5!` · `0xff+0b11` · `sqrt(2)` · `log(8, 2)` · `max(1, 7)` · `ans*2` |
| `<Leader>xu` | Unit converter | `5 ft to cm` · `70 kg en lb` · `100 f c` · `1.5 GiB MB` · `90 km/h mph` · `2 m` (every unit of its kind) |
| `<Leader>xb` | Number bases | `255` → `255 · 0xff · 0o377 · 0b1111_1111 · 'ÿ' U+00FF`; also `-0b101`, `2^40` |
| `<Leader>xt` | Time | empty or `now` · `1700000000` (or milliseconds) · `2026-10-18 14:30` (local; `Z` for UTC) · `2026-01-01 to 2026-03-01` (difference) |
| `<Leader>xh` | Color | `#f80` · `#ff8800` · `rgb(255, 136, 0)` · `hsl(32, 100%, 50%)` → all three forms |
| `<Leader>xk` | Case of the WORD under the cursor | `snake` `camel` `pascal` `kebab` `constant` `title` `lower` `upper` (`sn`, `k`…) |
| `<Leader>xd` `xD` | Insert the date / date and time (local) | `2026-10-18` · `2026-10-18 14:30:05` |
| `<Leader>xi` `xp` `xl` | Insert a UUID v4 / a 20-char password / lorem ipsum | random bytes come from the system (`/dev/urandom`, `RtlGenRandom`) |
| `<Leader>xj` `xJ` | Format / minify the buffer as JSON (`:json`, `:json!` on a range) | invalid JSON is left alone with the parser's message |
| `<Leader>xw` `xs` | Count lines, words, chars / SHA-256 of the buffer | |

## Calculator

`+ - * / ^` (right to left, `-2^2` is -4; `**` too), `%` (a remainder between two numbers, else a
percentage), `!`, `√`, `²`, `³`, parentheses (`[ ]` and `{ }` too; those still open at the end are closed), a value
next to a name or `(` multiplies (`2pi(1+1)`, `(1+1)(2)`); `×` `·` `÷` `−` are read as `*` `*` `/` `-`; literals
`1.5`, `.5`, `1,5` (decimal comma outside function arguments), `1 000 000` (blanks are ignored), `1e-3`, `0xff`,
`0b101`, `0o17`; functions `sqrt cbrt abs floor ceil round trunc sign exp ln log log2 log(x, base) sin cos tan asin
acos atan sinh cosh tanh rad deg hypot pow min max`; constants `pi` (`π`) `e` `tau` `phi`, and `ans` for the
previous answer. Answers show at most 10 decimals (`0.1+0.2` is `0.3`). A wrong line says what and where:
`unmatched ) at column 4`, `unknown name foo`, `incomplete: something is missing after +`, `infinite result (a
division by zero or an overflow)`, `undefined result (like sqrt(-1)…)`, `! needs a whole number from 0 to 170`.

## Units

French names work too: `pied`, `livre`, `pouce`, `tasse`, `octet`…

length `m km cm mm µm nm in ft yd mi nmi au ly` · area `m2 km2 cm2 mm2 ha acre ft2 in2 mi2` · volume `l ml cl dl m3
cc gal ukgal qt pt cup floz tbsp tsp` · mass `kg g mg t lb oz st ct` · time `s ms µs ns min h d wk mo yr` · speed
`m/s km/h mph kn ft/s` · temperature `c f k` · data `B bit KB MB GB TB PB KiB MiB GiB TiB Kb Mb Gb` (case tells bits
from bytes) · energy `J kJ cal kcal Wh kWh eV BTU` · power `W kW MW hp ch` · pressure `Pa hPa kPa bar mbar atm psi
mmHg` · angle `rad deg grad turn`. US cups, pints and gallons.
