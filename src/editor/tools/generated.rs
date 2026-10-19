//! Text the `<Leader>x` keys insert: dates, UUIDs, passwords, lorem ipsum.

use std::collections::hash_map::RandomState;
use std::fmt::Write;
use std::hash::BuildHasher;

use super::time::{datetime, now, utc_offset};

/// Text the `<Leader>x` keys insert at the cursor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Generated {
    Date,
    DateTime,
    Uuid,
    Password,
    Lorem,
}

/// Random bytes from the system's source (getrandom); if that fails, `SipHash` over a counter
/// keyed by std's per-process random keys.
fn random_bytes(n: usize) -> Vec<u8> {
    random_bytes_from(n, |bytes| getrandom::fill(bytes).is_ok())
}

/// `random_bytes` with the system source given, so the fallback can be tested.
fn random_bytes_from(n: usize, system: fn(&mut [u8]) -> bool) -> Vec<u8> {
    let mut bytes: Vec<u8> = vec![0; n];
    if !system(&mut bytes) {
        let keys: RandomState = RandomState::new();
        bytes = (0..).map(|i: u64| keys.hash_one(i)).flat_map(u64::to_le_bytes).take(n).collect();
    }
    bytes
}

fn uuid() -> String {
    let mut b: Vec<u8> = random_bytes(16);
    b[6] = (b[6] & 0x0f) | 0x40;
    b[8] = (b[8] & 0x3f) | 0x80;
    let hex: String = b.iter().fold(String::new(), |mut s, x| {
        let _: std::fmt::Result = write!(s, "{x:02x}");
        s
    });
    format!("{}-{}-{}-{}-{}", &hex[..8], &hex[8..12], &hex[12..16], &hex[16..20], &hex[20..])
}

/// 20 chars: letters, digits and symbols, picked without bias (bytes past the last whole set are redrawn).
fn password() -> String {
    const SET: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz23456789!@#$%^&*-_=+?";
    let limit: usize = 256 - 256 % SET.len();
    let mut out: String = String::new();
    while out.len() < 20 {
        for b in random_bytes(32) {
            let b: usize = usize::from(b);
            if b < limit && out.len() < 20 {
                out.push(char::from(SET[b % SET.len()]));
            }
        }
    }
    out
}

const LOREM: &str = "Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do eiusmod tempor incididunt ut labore et \
dolore magna aliqua. Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris nisi ut aliquip ex ea commodo \
consequat. Duis aute irure dolor in reprehenderit in voluptate velit esse cillum dolore eu fugiat nulla pariatur.";

impl Generated {
    pub fn text(self) -> String {
        let now: i64 = now();
        let local: String = datetime(now, utc_offset(now));
        match self {
            Self::Date => local[..10].to_owned(),
            Self::DateTime => local,
            Self::Uuid => uuid(),
            Self::Password => password(),
            Self::Lorem => LOREM.to_owned(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated() {
        assert_eq!(random_bytes_from(20, |_| false).len(), 20, "falls back when the system source fails");
        assert_ne!(random_bytes_from(8, |_| false), random_bytes_from(8, |_| false));
        let id: String = uuid();
        assert_eq!((id.len(), &id[14..15]), (36, "4"));
        assert_ne!(uuid(), id);
        assert_eq!(password().len(), 20);
        assert_eq!(Generated::Date.text().len(), 10);
    }
}
