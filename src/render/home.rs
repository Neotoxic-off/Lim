//! The start screen: the Lernaean Hydra rising from its lake (limne), lim's symbol. Three drawings of it: lim's
//! ASCII one and two braille ones found online (the `splash` option); by default a random one that fits.

use std::time::Duration;

use super::row::Grid;
use super::theme::{Group, theme};
use crate::buffer::text;
use crate::config::Splash;
use crate::editor::Rect;
use crate::render::theme::Style;

/// lim's own: '#' body, '@' eyes, '~' the lake; `'` and `.` are half-filled cells.
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

/// `NvChad`'s hydra in braille, from the gist "Epic hydra ASCII art (`NvChad`)" by `kevinfengcs88`.
const BRAILLE: &[&str] = &[
    "⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⢀⡀⠀⠀⠀⠠⡆⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀",
    "⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⣀⣀⣀⡀⠀⠙⢶⣄⠀⠀⣧⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀",
    "⠀⠀⠀⠀⢠⣶⣦⣤⣀⣀⣤⣤⣄⣀⠀⢀⣀⣴⠂⠀⠀⠀⠀⠀⠀⠀⠐⠉⠉⣉⣉⣽⣿⣿⣷⣾⣿⣷⣄⡸⣷⡀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀",
    "⠀⠀⠀⠀⠿⠿⢿⣿⣿⣿⣭⣭⣿⣿⣿⣿⣟⣁⠀⠀⠀⠀⠀⠀⠀⠀⣠⣶⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣦⣄⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀",
    "⠀⠀⠀⠀⠀⠀⠀⠈⠻⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣷⠶⠤⠀⠀⢠⡾⢿⣿⣿⣿⣿⡿⠉⠀⠀⠀⠈⠙⢻⣿⣿⣿⡛⢻⣷⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀",
    "⠀⠀⠀⠀⠀⠀⠀⠀⠀⠈⠛⠋⠀⠀⠀⠉⠻⣿⣿⣿⣿⣦⡀⠀⠁⠀⣿⣿⣿⣿⣿⡇⠀⠀⠀⠀⠀⠀⠀⣿⣿⣿⣿⣦⣿⣧⡀⠀⠀⠀⠀⠀⠀⠀⠀⠀",
    "⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠹⣿⣿⣿⣯⡙⢦⠀⠀⣿⣿⣿⣿⣿⣿⡄⠀⠀⠀⠀⠀⠀⠙⠻⠿⠿⣿⣿⣿⣿⣶⣄⠀⠀⠀⠀⠀⠀⠀",
    "⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⣿⣿⣿⣿⣿⡄⠀⠀⣿⣿⣿⣿⣿⣿⣿⣦⠀⠀⠀⠀⠰⣄⠀⠀⠀⠀⠈⠛⢿⣿⡏⠀⠀⠀⠀⠀⠀⠀",
    "⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⢰⣿⣿⣿⣿⡝⡇⠀⠀⠹⡇⠙⢿⣿⣿⣿⣿⣿⣶⣦⣄⣀⣈⣳⣶⣤⣤⣄⣀⠈⠋⠀⠀⠀⠀⠀⠀⠀⠀",
    "⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⣾⣿⣿⣿⣿⡇⠁⠀⠀⠀⠙⣠⠤⠿⢿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⡛⠻⣷⣄⡀⠀⠀⠀⠀⠀⠀⠀",
    "⠀⠀⠀⠀⠈⢲⡄⠀⢀⡠⠔⠂⠀⠀⠀⠀⣸⣿⣿⣿⡿⢹⠇⠀⠀⠀⠀⠈⢀⣤⣶⣾⣿⣿⣿⣿⣿⣿⣿⡟⠻⢿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣧⠀⠀⠀⠀",
    "⠀⠀⠀⠀⠀⣾⣧⣾⣿⣶⣶⣶⣤⣀⠀⠀⣿⣿⣿⣿⠇⠋⠀⠀⠀⠀⢀⣴⣿⣿⣿⣿⣿⠟⠛⢿⣿⣿⣿⣿⡄⠀⠻⣿⡿⠿⠛⠛⠛⠛⠿⡿⠀⠀⠀⠀",
    "⠀⠀⠀⢀⣼⣿⣿⣿⣿⣿⣿⣿⣷⣮⡁⠀⣿⣿⣿⣿⠀⠀⠀⠀⠀⢠⠞⣻⣿⣿⣿⡿⠁⠀⠀⠈⣿⣿⣿⣿⣧⠀⠀⠀⢀⡀⠀⠀⠀⣴⠀⠀⠀⠀⠀⠀",
    "⠀⠀⢠⡿⢹⣿⣿⡋⠀⠈⢻⣿⣿⣿⡟⠆⢻⣿⣿⣿⡇⠀⠀⠀⠀⠀⢰⣿⣿⣿⣿⡇⠀⠀⠀⠀⢸⣿⣿⣿⣿⠀⠀⠀⣀⣭⣽⣶⣬⣿⡄⠀⠀⠀⠀⠀",
    "⠀⠀⣰⣷⣿⣿⠿⠃⠀⠀⢸⣿⣿⣿⣿⡄⠘⣿⣿⣿⣿⣄⠀⠀⠀⠀⣿⣿⣿⣿⣿⣿⣄⠀⠀⠀⣾⣿⣿⣿⣿⠀⠴⣻⣿⣿⣿⣿⣿⣿⣿⣦⡀⠀⠀⠀",
    "⠀⣴⣿⡿⠋⠀⠀⠀⠀⠀⣼⣿⣿⣿⢿⡇⠀⠘⣿⣿⣿⣿⣦⡀⠀⠀⢸⡟⢿⣿⣿⣿⣿⣧⡀⣰⣿⣿⣿⣿⡏⠀⣼⣿⣿⣿⠋⠀⠉⣿⣿⣌⣷⠀⠀⠀",
    "⠀⠈⠛⠁⠀⠀⠀⠀⠀⢸⣿⣿⣿⡏⠘⠀⠀⠀⠈⢻⣿⣿⣿⣿⣷⣤⡀⠳⠀⠻⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⡇⠈⣿⣿⣿⣿⠀⠀⠈⠛⠻⢿⣿⣷⡄⠀",
    "⠀⠀⠀⠀⠀⠀⠀⠀⠀⢸⣿⣿⣿⣇⠀⠀⠀⠀⠀⠀⠉⠻⣿⣿⣿⣿⣿⣷⣶⣤⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⠃⠀⣿⢿⣿⣿⣧⡀⠀⠀⠀⠀⠈⠿⠇⠀",
    "⠀⠀⠀⠀⠀⠀⠀⠀⠀⠸⣿⣿⣿⣿⣦⣀⠀⠀⠀⠀⠀⠀⠈⢻⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⡏⠀⠀⠘⠌⣿⣿⣿⡇⠀⠀⠀⠀⠀⠀⠀⠀",
    "⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠙⣿⣿⣿⣿⣿⣿⣶⣶⣤⣤⣤⣄⣼⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⡟⠀⠀⠀⠀⢀⣿⣿⣿⡇⠀⠀⠀⠀⠀⠀⠀⠀",
    "⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠈⠻⢿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣟⣀⣤⣤⣴⣾⣿⣿⣿⠟⠀⠀⠀⠀⠀⠀⠀⠀⠀",
    "⠀⠀⠀⠀⠀⠀⠀⠀⣀⣀⡀⠀⠀⢤⣬⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⠟⡁⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀",
    "⠀⠀⠀⠀⠀⠀⠠⠾⣿⣿⣿⣶⣤⣤⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣠⣶⣦⣄⡀⠀⠀⣶⢒⠲⣄",
    "⣾⣥⣤⣼⣿⣶⣶⣶⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣿⣶⣾⣵⣾⡿",
];

/// The same hydra, small, from twitchquotes.com's "[ASCII Art] Hydra". Its "HYDRA" lettering is left out.
const SMALL: &[&str] = &[
    "⠀⠀⣴⣶⣤⡤⠦⣤⣀⣤⠆⠀⠀⠀⠀⠀⣈⣭⣭⣿⣶⣿⣦⣼⣆⠀⠀⠀⠀⠀⠀⠀⠀",
    "⠀⠀⠀⠉⠻⢿⣿⠿⣿⣿⣶⣦⠤⠀⡠⢾⣿⣿⡿⠋⠉⠉⠻⣿⣿⡛⣦⠀⠀⠀⠀⠀⠀",
    "⠀⠀⠀⠀⠀⠈⠀⠀⠀⠈⢿⣿⣟⠦⠀⣾⣿⣿⣷⠀⠀⠀⠀⠻⠿⢿⣿⣧⣄⠀⠀⠀⠀",
    "⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⣸⣿⣿⢧⠀⢻⠻⣿⣿⣷⣄⣀⠀⠢⣀⡀⠈⠙⠿⠀⠀⠀⠀",
    "⠀⠀⢀⠀⠀⠀⠀⠀⠀⢠⣿⣿⣿⠈⠀⠀⠡⠌⣻⣿⣿⣿⣿⣿⣿⣿⣛⣳⣤⣀⣀⠀⠀",
    "⠀⠀⢠⣧⣶⣥⡤⢄⠀⣸⣿⣿⠘⠀⠀⢀⣴⣿⣿⡿⠛⣿⣿⣧⠈⢿⠿⠟⠛⠻⠿⠀⠀",
    "⠀⣰⣿⣿⠛⠻⣿⣿⡦⢹⣿⣷⠀⠀⠀⢊⣿⣿⡏⠀⠀⢸⣿⣿⡇⠀⢀⣠⣄⣾⠀⠀⠀",
    "⣠⣿⠿⠛⠀⢀⣿⣿⣷⠘⢿⣿⣦⡀⠀⢸⢿⣿⣿⣄⠀⣸⣿⣿⡇⣪⣿⡿⠿⣿⣷⡄⠀",
    "⠙⠃⠀⠀⠀⣼⣿⡟⠌⠀⠈⠻⣿⣿⣦⣌⡇⠻⣿⣿⣷⣿⣿⣿⠐⣿⣿⡇⠀⠛⠻⢷⣄",
    "⠀⠀⠀⠀⠀⢻⣿⣿⣄⠀⠀⠀⠈⠻⣿⣿⣿⣷⣿⣿⣿⣿⣿⡟⠀⠫⢿⣿⡆⠀⠀⠀⠁",
    "⠀⠀⠀⠀⠀⠀⠻⣿⣿⣿⣿⣶⣶⣾⣿⣿⣿⣿⣿⣿⣿⣿⡟⢀⣀⣤⣾⡿⠃⠀⠀⠀⠀",
];

/// "lim" in figlet's standard font.
const WORDMARK: &[&str] =
    &[" _ _           ", "| (_)_ __ ___  ", "| | | '_ ` _ \\ ", "| | | | | | | |", "|_|_|_| |_| |_|"];

const TAGLINE: &str = "limne - the hydra of lerna";

/// The drawing to show in `area` (with room for the wordmark), if one fits. `Random` picks among those that fit,
/// the same one for the whole run.
fn art(splash: Splash, area: Rect) -> Option<&'static [&'static str]> {
    let fits = |art: &&[&str]| area.h >= art.len() + WORDMARK.len() + 7 && area.w >= width(art) + 2;
    let wanted: &[&[&str]] = match splash {
        Splash::Classic => &[HYDRA],
        Splash::Braille => &[BRAILLE],
        Splash::Small => &[SMALL],
        Splash::Random => &[HYDRA, BRAILLE, SMALL],
    };
    let fitting: Vec<&[&str]> = wanted.iter().copied().filter(|a| fits(a)).collect();
    let seed: usize = usize::try_from(std::process::id()).unwrap_or(0);
    fitting.get(seed % fitting.len().max(1)).copied()
}

fn width(art: &[&str]) -> usize {
    art.iter().map(|l| text::str_width(l)).max().unwrap_or(0)
}

pub fn dashboard(grid: &mut Grid, area: Rect, startup: Duration, splash: Splash) {
    let monster: Option<&[&str]> = art(splash, area);
    let style: Style = theme().style(Group::Normal);
    let footer: String =
        format!("neovim, batteries included - v{} - {:.2}ms", env!("LIM_VERSION"), startup.as_secs_f64() * 1000.0);
    // Without room for a hydra the wordmark stays, then only the text.
    let show_word: bool = area.h >= WORDMARK.len() + 5 && area.w >= width(WORDMARK) + 2;
    let height: usize = monster.map_or(0, |m| m.len() + 2) + if show_word { WORDMARK.len() + 2 } else { 0 } + 3;
    let mut y: usize = area.y + area.h.saturating_sub(height) / 2;
    let centered = |w: usize| area.x + area.w.saturating_sub(w) / 2;
    for art in [monster, show_word.then_some(WORDMARK)].into_iter().flatten() {
        let w: usize = width(art);
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
        let w: usize = text::str_width(line);
        grid.text(centered(w), y, w, style, line);
        y += 2;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn art_is_rectangular_and_picked_to_fit() {
        for art in [HYDRA, BRAILLE, SMALL, WORDMARK] {
            assert!(art.iter().all(|l| text::str_width(l) == width(art)));
        }
        let roomy: Rect = Rect { x: 0, y: 0, w: 120, h: 50 };
        assert_eq!(art(Splash::Braille, roomy), Some(BRAILLE));
        // Only the small one fits 27 rows by 60 columns, so random has to pick it.
        assert_eq!(art(Splash::Random, Rect { x: 0, y: 0, w: 60, h: 27 }), Some(SMALL));
        assert_eq!(art(Splash::Random, Rect { x: 0, y: 0, w: 20, h: 10 }), None);
    }
}
