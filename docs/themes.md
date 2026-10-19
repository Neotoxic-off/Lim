# Themes and icons

Every picture below is a real frame drawn by lim's renderer (`cargo test gallery -- --ignored` redraws them).

## Colorschemes

30 themes: colors come from each theme's own repository, mapped onto the astrotheme palette lim
draws with. nvchad (NvChad's base46 onedark) is the default.

|   |   |   |
|---|---|---|
| <img src="gallery/nvchad.svg" alt="nvchad" width="300"> | <img src="gallery/astrodark.svg" alt="astrodark" width="300"> | <img src="gallery/astrolight.svg" alt="astrolight" width="300"> |
| `nvchad` | `astrodark` | `astrolight` |
| <img src="gallery/astromars.svg" alt="astromars" width="300"> | <img src="gallery/astrojupiter.svg" alt="astrojupiter" width="300"> | <img src="gallery/catppuccin-mocha.svg" alt="catppuccin-mocha" width="300"> |
| `astromars` | `astrojupiter` | `catppuccin-mocha` |
| <img src="gallery/catppuccin-macchiato.svg" alt="catppuccin-macchiato" width="300"> | <img src="gallery/catppuccin-frappe.svg" alt="catppuccin-frappe" width="300"> | <img src="gallery/catppuccin-latte.svg" alt="catppuccin-latte" width="300"> |
| `catppuccin-macchiato` | `catppuccin-frappe` | `catppuccin-latte` |
| <img src="gallery/tokyonight-night.svg" alt="tokyonight-night" width="300"> | <img src="gallery/tokyonight-storm.svg" alt="tokyonight-storm" width="300"> | <img src="gallery/tokyonight-moon.svg" alt="tokyonight-moon" width="300"> |
| `tokyonight-night` | `tokyonight-storm` | `tokyonight-moon` |
| <img src="gallery/tokyonight-day.svg" alt="tokyonight-day" width="300"> | <img src="gallery/dracula.svg" alt="dracula" width="300"> | <img src="gallery/onedark.svg" alt="onedark" width="300"> |
| `tokyonight-day` | `dracula` | `onedark` |
| <img src="gallery/gruvbox.svg" alt="gruvbox" width="300"> | <img src="gallery/nord.svg" alt="nord" width="300"> | <img src="gallery/nordic.svg" alt="nordic" width="300"> |
| `gruvbox` | `nord` | `nordic` |
| <img src="gallery/night-owl.svg" alt="night-owl" width="300"> | <img src="gallery/sonokai.svg" alt="sonokai" width="300"> | <img src="gallery/kanagawa.svg" alt="kanagawa" width="300"> |
| `night-owl` | `sonokai` | `kanagawa` |
| <img src="gallery/rose-pine.svg" alt="rose-pine" width="300"> | <img src="gallery/rose-pine-moon.svg" alt="rose-pine-moon" width="300"> | <img src="gallery/rose-pine-dawn.svg" alt="rose-pine-dawn" width="300"> |
| `rose-pine` | `rose-pine-moon` | `rose-pine-dawn` |
| <img src="gallery/everforest.svg" alt="everforest" width="300"> | <img src="gallery/gruvbox-light.svg" alt="gruvbox-light" width="300"> | <img src="gallery/solarized-dark.svg" alt="solarized-dark" width="300"> |
| `everforest` | `gruvbox-light` | `solarized-dark` |
| <img src="gallery/solarized-light.svg" alt="solarized-light" width="300"> | <img src="gallery/github-dark.svg" alt="github-dark" width="300"> | <img src="gallery/ayu-dark.svg" alt="ayu-dark" width="300"> |
| `solarized-light` | `github-dark` | `ayu-dark` |

Switch in the settings (`<Leader>,`), with `<Leader>ft` (the editor recolors as you move through the list;
`<Esc>` puts the old one back), or `<Leader>ub` for the light/dark sibling. The last one picked is remembered; set
`"colorscheme"` in the config to pin one. [`palette` and `highlights`](configuration.md#palette) overrides apply on
top of any theme.

## Icon sets

`nerd` (default, Nerd Font devicons as AstroNvim), `unicode` (any font), `emoji` (color emoji,
two cells each) and `ascii`. Switch in the settings (`<Leader>,`), `<Leader>fI`, or the `iconset` option.

|   |   |
|---|---|
| <img src="gallery/icons-nerd.svg" alt="nerd" width="300"> | <img src="gallery/icons-unicode.svg" alt="unicode" width="300"> |
| `nerd` | `unicode` |
| <img src="gallery/icons-emoji.svg" alt="emoji" width="300"> | <img src="gallery/icons-ascii.svg" alt="ascii" width="300"> |
| `emoji` | `ascii` |

## Statusline styles

`statusline_style`, Nerd Font icon set; the pictures need a Nerd Font installed to show the section ends.

|   |   |
|---|---|
| <img src="gallery/statusline-rounded.svg" alt="rounded" width="300"> | <img src="gallery/statusline-slanted.svg" alt="slanted" width="300"> |
| `rounded` | `slanted` |
| <img src="gallery/statusline-arrow.svg" alt="arrow" width="300"> | <img src="gallery/statusline-flat.svg" alt="flat" width="300"> |
| `arrow` | `flat` |

## Start screen

The Lernaean Hydra, lim's symbol, in three drawings. By default (`splash` = `"random"`) each start
shows a random one among those that fit the window; set `"classic"`, `"braille"` or `"small"` to keep one. The
braille ones are NvChad's hydra as found online: the [gist by kevinfengcs88](https://gist.github.com/kevinfengcs88/75457d6b401fbab2326450dcbf64a429)
and the small one from [twitchquotes](https://www.twitchquotes.com/copypastas/3638).

| | | |
|---|---|---|
| <img src="gallery/splash-classic.svg" alt="classic" width="300"> | <img src="gallery/splash-braille.svg" alt="braille" width="300"> | <img src="gallery/splash-small.svg" alt="small" width="300"> |
| `classic` (ASCII) | `braille` | `small` (fits small windows) |
