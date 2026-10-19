//! The start screen: the Lernaean Hydra rising from its lake, lim's symbol, over nvdash's buttons. Three drawings
//! of it: lim's ASCII one and two braille ones found online (the `splash` option); by default a random one that fits.

use std::sync::Arc;

use super::icons::{Icons, icons};
use super::row::Grid;
use super::theme::{Group, Theme, theme};
use crate::buffer::text;
use crate::config::{Options, Splash, keys};
use crate::editor::Rect;
use crate::render::theme::Style;
use crate::term::Key;

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

/// The spinner for work in the background: a neck of the hydra rising from its lake and swimming across, in the
/// braille drawings' characters (`⣀` the lake, `⢀⣴⣿` the neck, `⣷` the head).
pub const SPINNER: [&str; 8] = ["⣀⣀⣀⣀", "⢀⣀⣀⣀", "⣴⢀⣀⣀", "⣿⣴⢀⣀", "⣷⣿⣴⢀", "⣀⣷⣿⣴", "⣀⣀⣷⣿", "⣀⣀⣀⣷"];

/// Time each spinner frame stays.
pub const SPINNER_FRAME_MS: u128 = 120;

/// The drawing to show in `area`, if one fits. `Random` picks among those that fit, the same one for the whole run.
fn art(splash: Splash, area: Rect) -> Option<&'static [&'static str]> {
    let fits = |art: &&[&str]| area.h >= art.len() + 2 && area.w >= width(art) + 2;
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

/// nvdash's buttons, with lim's keys for them after the leader.
const BUTTONS: [(&str, &str); 5] =
    [("Find File", "f f"), ("Recent Files", "f o"), ("Find Word", "f w"), ("Themes", "f t"), ("Mappings", "f k")];

/// nvdash's button width: label on the left, keys on the right.
const BUTTON_WIDTH: usize = 36;

pub fn dashboard(grid: &mut Grid, area: Rect, options: &Options) {
    let t: Arc<Theme> = theme();
    let monster: Option<&[&str]> = art(options.splash, area);
    let style: Style = t.style(Group::Normal);
    let mut height: usize = monster.map_or(0, |m| m.len() + 2);
    // The buttons go first when space runs out: the hydra is lim's face.
    let buttons_height: usize = BUTTONS.len() * 2 - 1;
    let show_buttons: bool = height + buttons_height <= area.h && area.w >= BUTTON_WIDTH + 2;
    height += if show_buttons { buttons_height } else { 0 };
    let mut y: usize = area.y + area.h.saturating_sub(height) / 2;
    let centered = |w: usize| area.x + area.w.saturating_sub(w) / 2;
    if let Some(art) = monster {
        let w: usize = width(art);
        for line in art {
            grid.text(centered(w), y, w, style, line);
            y += 1;
        }
        y += 2;
    }
    if show_buttons {
        let leader: String = match options.mapleader {
            Key::Char(' ') => "SPC".into(),
            key => keys::display(&[key]),
        };
        let (label, key): (Style, Style) = (t.style(Group::CheatsheetDesc), t.style(Group::CheatsheetKey));
        let set: &Icons = icons(options);
        for ((text, keys), icon) in BUTTONS.iter().zip(set.dashboard) {
            let left: String = if icon.is_empty() { (*text).to_owned() } else { format!("{icon}  {text}") };
            let right: String = format!("{leader} {keys}");
            let x: usize = centered(BUTTON_WIDTH);
            grid.text(x, y, BUTTON_WIDTH, label, &left);
            let right_width: usize = text::str_width(&right);
            grid.text(x + BUTTON_WIDTH - right_width, y, right_width, key, &right);
            y += 2;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn art_is_rectangular_and_picked_to_fit() {
        for art in [HYDRA, BRAILLE, SMALL] {
            assert!(art.iter().all(|l| text::str_width(l) == width(art)));
        }
        let roomy: Rect = Rect { x: 0, y: 0, w: 120, h: 50 };
        assert_eq!(art(Splash::Braille, roomy), Some(BRAILLE));
        // Only the small one fits 20 rows by 60 columns, so random has to pick it.
        assert_eq!(art(Splash::Random, Rect { x: 0, y: 0, w: 60, h: 20 }), Some(SMALL));
        assert_eq!(art(Splash::Random, Rect { x: 0, y: 0, w: 20, h: 10 }), None);
    }

    #[test]
    fn the_spinner_keeps_its_width_and_swims_through() {
        assert!(SPINNER.iter().all(|f| text::str_width(f) == 4 && f.chars().all(|c| ('⠀'..='⣿').contains(&c))));
        let heads: Vec<Option<usize>> = SPINNER.iter().map(|f| f.chars().position(|c| c == '⣷')).collect();
        assert_eq!(
            heads,
            [None, None, None, None, Some(0), Some(1), Some(2), Some(3)],
            "the head leads, left to right"
        );
    }

    #[test]
    fn buttons_under_the_hydra_go_first_when_space_runs_out() {
        let screen = |h: usize| -> String {
            let mut grid: Grid = Grid::new(100, h, theme().style(Group::Normal));
            let options: Options = Options { splash: Splash::Small, ..Options::default() };
            dashboard(&mut grid, Rect { x: 0, y: 0, w: 100, h }, &options);
            (0..h).map(|y| grid.row(y).iter().map(|c| c.ch).collect::<String>() + "\n").collect()
        };
        let roomy: String = screen(50);
        assert!(roomy.contains("Find File") && roomy.contains("SPC f f") && roomy.contains("⣿"), "{roomy}");
        // The small hydra fits 20 rows, the buttons under it do not.
        let tight: String = screen(20);
        assert!(!tight.contains("Find File") && tight.contains("⣿"), "{tight}");
    }
}
