//! The start screen: the Lernaean Hydra rising from its lake (limne), lim's symbol, or another creature of the
//! underworld (the `splash` option), in plain ASCII. All but the hydra are classic ASCII art found online, kept
//! as drawn with their artists' signatures.

use std::time::Duration;

use super::row::Grid;
use super::theme::{Group, theme};
use crate::buffer::text;
use crate::config::Splash;
use crate::editor::Rect;
use crate::render::theme::Style;

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

/// Cerberus, by b'ger (ascii.co.uk).
const CERBERUS: &[&str] = &[
    r"                   /\_/\____,    ",
    r"         ,___/\_/\ \  ~     /    ",
    r"         \     ~  \ )   XXX      ",
    r"           XXX     /    /\_/\___,",
    r"              \o-o/-o-o/   ~    /",
    r"               ) /     \    XXX  ",
    r"              _|    / \ \_/      ",
    r"           ,-/   _  \_/   \      ",
    r"          / (   /____,__|  )     ",
    r"         (  |_ (    )  \) _|     ",
    r"        _/ _)   \   \__/   (_    ",
    r"b'ger  (,-(,(,(,/      \,),),)   ",
];

/// A wild-maned horse, by Joan G. Stark (asciiart.eu).
const MARES: &[&str] = &[
    r"                            _(\_/)    ",
    r"                          ,((((^`\    ",
    r"                         ((((  (6 \   ",
    r"                       ,((((( ,    \  ",
    r#"   ,,,_              ,(((((  /"._  ,`,"#,
    r"  ((((\\ ,...       ,((((   /    `-.-'",
    r#"  )))  ;'    `"'"'""((((   (          "#,
    r" (((  /            (((      \         ",
    r"  )) |                      |         ",
    r" ((  |        .       '     |         ",
    r" ))  \     _ '      `t   ,.')         ",
    r#" (   |   y;- -,-""'"-.\   \/          "#,
    r" )   / ./  ) /         `\  \          ",
    r"    |./   ( (           / /'          ",
    r"    ||     \\          //'|           ",
    r"jgs ||      \\       _//'||           ",
    r"    ||       ))     |_/  ||           ",
    r"    \_\     |_/          ||           ",
    r#"    `'"                  \_\          "#,
];

/// Death with its scythe, artist unknown (asciiart.eu).
const REAPER: &[&str] = &[
    r"              ...                             ",
    r"             ;::::;                           ",
    r"           ;::::; :;                          ",
    r"         ;:::::'   :;                         ",
    r"        ;:::::;     ;.                        ",
    r"       ,:::::'       ;           OOO\         ",
    r"       ::::::;       ;          OOOOO\        ",
    r"       ;:::::;       ;         OOOOOOOO       ",
    r"      ,;::::::;     ;'         / OOOOOOO      ",
    r"    ;:::::::::`. ,,,;.        /  / DOOOOOO    ",
    r"  .';:::::::::::::::::;,     /  /     DOOOO   ",
    r" ,::::::;::::::;;;;::::;,   /  /        DOOO  ",
    r";`::::::`'::::::;;;::::: ,#/  /          DOOO ",
    r":`:::::::`;::::::;;::: ;::#  /            DOOO",
    r"::`:::::::`;:::::::: ;::::# /              DOO",
    r"`:`:::::::`;:::::: ;::::::#/               DOO",
    r" :::`:::::::`;; ;:::::::::##                OO",
    r" ::::`:::::::`;::::::::;:::#                OO",
    r" `:::::`::::::::::::;'`:;::#                O ",
    r"  `:::::`::::::::;' /  / `:#                  ",
    r"   ::::::`:::::;'  /  /   `#                   ",
];

/// A horned devil, signed eViL (asciiart.eu).
const DEVIL: &[&str] = &[
    r"                            ,-.                            ",
    r"       ___,---.__          /'|`\          __,---,___       ",
    r"    ,-'    \`    `-.____,-'  |  `-.____,-'    //    `-.    ",
    r"  ,'        |           ~'\     /`~           |        `.  ",
    r" /      ___//              `. ,'          ,  , \___      \ ",
    r"|    ,-'   `-.__   _         |        ,    __,-'   `-.    |",
    r"|   /          /\_  `   .    |    ,      _/\          \   |",
    r"\  |           \ \`-.___ \   |   / ___,-'/ /           |  /",
    r" \  \           | `._   `\\  |  //'   _,' |           /  / ",
    r"  `-.\         /'  _ `---'' , . ``---' _  `\         /,-'  ",
    r"     ``       /     \    ,='/ \`=.    /     \       ''     ",
    r"             |__   /|\_,--.,-.--,--._/|\   __|             ",
    r"             /  `./  \\`\ |  |  | /,//' \,'  \             ",
    r"eViL        /   /     ||--+--|--+-/-|     \   \            ",
    r"           |   |     /'\_\_\ | /_/_/`\     |   |           ",
    r"            \   \__, \_     `~'     _/ .__/   /            ",
    r"             `-._,-'   `-._______,-'   `-._,-'             ",
];

/// "lim" in figlet's standard font.
const WORDMARK: &[&str] =
    &[" _ _           ", "| (_)_ __ ___  ", "| | | '_ ` _ \\ ", "| | | | | | | |", "|_|_|_| |_| |_|"];

/// The art and its tagline; `Random` picks one per run, so it holds still while the editor is open.
fn art(splash: Splash) -> (&'static [&'static str], &'static str) {
    let monsters: [Splash; 5] = [Splash::Hydra, Splash::Cerberus, Splash::Mares, Splash::Reaper, Splash::Devil];
    let splash: Splash = match splash {
        Splash::Random => usize::try_from(std::process::id()).map_or(Splash::Hydra, |id| monsters[id % monsters.len()]),
        other => other,
    };
    match splash {
        Splash::Cerberus => (CERBERUS, "cerberus - the hound at the gates of hades"),
        Splash::Mares => (MARES, "the mares of diomedes - fed on flesh, breathing fire"),
        Splash::Reaper => (REAPER, "thanatos - death comes for every buffer"),
        Splash::Devil => (DEVIL, "hades - lord of the underworld"),
        Splash::Hydra | Splash::Random => (HYDRA, "limne - the hydra of lerna"),
    }
}

pub fn dashboard(grid: &mut Grid, area: Rect, startup: Duration, splash: Splash) {
    let (monster, tagline): (&[&str], &str) = art(splash);
    let style: Style = theme().style(Group::Normal);
    let footer: String = format!(
        "a lightweight modal editor - v{} - {:.2}ms",
        env!("CARGO_PKG_VERSION"),
        startup.as_secs_f64() * 1000.0
    );
    let art_width = |art: &[&str]| art.iter().map(|l| text::str_width(l)).max().unwrap_or(0);
    // Drop the monster, then the wordmark, when the window is too small for them.
    let show_monster: bool = area.h >= monster.len() + WORDMARK.len() + 7 && area.w >= art_width(monster) + 2;
    let show_word: bool = area.h >= WORDMARK.len() + 5 && area.w >= art_width(WORDMARK) + 2;
    let height: usize =
        if show_monster { monster.len() + 2 } else { 0 } + if show_word { WORDMARK.len() + 2 } else { 0 } + 3;
    let mut y: usize = area.y + area.h.saturating_sub(height) / 2;
    let centered = |w: usize| area.x + area.w.saturating_sub(w) / 2;
    for (art, shown) in [(monster, show_monster), (WORDMARK, show_word)] {
        if !shown {
            continue;
        }
        let w: usize = art_width(art);
        for line in art {
            grid.text(centered(w), y, w, style, line);
            y += 1;
        }
        y += 2;
    }
    for line in [tagline, footer.as_str()] {
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
    fn art_is_ascii_and_rectangular() {
        for art in [HYDRA, CERBERUS, MARES, REAPER, DEVIL, WORDMARK] {
            assert!(art.iter().all(|l| l.is_ascii() && l.len() == art[0].len()));
        }
        for splash in Splash::ALL {
            let (monster, tagline): (&[&str], &str) = art(splash);
            assert!(tagline.is_ascii() && monster.len() <= HYDRA.len(), "{splash:?} fits where the hydra does");
        }
    }
}
