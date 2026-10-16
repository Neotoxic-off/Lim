//! The start screen: the Lernaean Hydra rising from its lake (limne), lim's symbol, in plain ASCII.

use std::time::Duration;

use super::row::Grid;
use super::theme::{Group, theme};
use crate::buffer::text;
use crate::editor::Rect;

/// '#' body, '@' eyes, '~' the lake; `'` and `.` are half-filled cells.
const HYDRA: &[&str] = &[
    "                         #.                               ",
    "                     '#.. '#.                             ",
    "             .          '#####.               .           ",
    "           .#'            ####@@###.          '#.         ",
    "         .###.#'          ####@@''''        '#..##.       ",
    "       .#####'            ######. .           '#####.     ",
    "  .####@@####             ####''###.           ####@@####.",
    "       ######            '####                 ######     ",
    "    #.#####'              #####                 '#####.#  ",
    "   .#####'               ''####.                  '#####. ",
    "    #####'                .#####.                 '#####  ",
    "    #####'                  #####.                '#####  ",
    "     #####' .              '######             . '#####   ",
    "     '#######  .            ######          .  #######'   ",
    "       '########.. .        ######      . ..########'     ",
    "         '###########......#######.....###########'     . ",
    "            ''################################''        ##",
    "                '''######################'''            ##",
    "                    ####################.            ..## ",
    "                     '#######################.....#####'  ",
    "    ~~~~~~~~~~~~~~~   ~##############~'''#########'''~~   ",
];

/// "lim" in figlet's standard font.
const WORDMARK: &[&str] = &[
    " _ _           ",
    "| (_)_ __ ___  ",
    "| | | '_ ` _ \\ ",
    "| | | | | | | |",
    "|_|_|_| |_| |_|",
];

const TAGLINE: &str = "limne - the hydra of lerna";

pub fn dashboard(grid: &mut Grid, area: Rect, startup: Duration) {
    let style = theme().style(Group::Normal);
    let footer = format!("a lightweight modal editor - v{} - {:.2}ms", env!("CARGO_PKG_VERSION"), startup.as_secs_f64() * 1000.0);
    let art_width = |art: &[&str]| art.iter().map(|l| text::str_width(l)).max().unwrap_or(0);
    // Drop the hydra, then the wordmark, when the window is too small for them.
    let show_hydra = area.h >= HYDRA.len() + WORDMARK.len() + 7 && area.w >= art_width(HYDRA) + 2;
    let show_word = area.h >= WORDMARK.len() + 5 && area.w >= art_width(WORDMARK) + 2;
    let height = if show_hydra { HYDRA.len() + 2 } else { 0 } + if show_word { WORDMARK.len() + 2 } else { 0 } + 3;
    let mut y = area.y + area.h.saturating_sub(height) / 2;
    let centered = |w: usize| area.x + area.w.saturating_sub(w) / 2;
    for (art, shown) in [(HYDRA, show_hydra), (WORDMARK, show_word)] {
        if !shown {
            continue;
        }
        let w = art_width(art);
        for line in art {
            grid.text(centered(w), y, w, style, line);
            y += 1;
        }
        y += 2;
    }
    for line in [TAGLINE, footer.as_str()] {
        if y >= area.y + area.h {
            return;
        }
        let w = text::str_width(line);
        grid.text(centered(w), y, w, style, line);
        y += 2;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn art_is_ascii_and_rectangular() {
        for art in [HYDRA, WORDMARK] {
            assert!(art.iter().all(|l| l.is_ascii() && l.len() == art[0].len()));
        }
        assert!(TAGLINE.is_ascii());
    }
}
