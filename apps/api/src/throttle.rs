//! Sign-in throttling: guessing is slowed down, but no User is ever locked out.

use std::{
    collections::HashMap,
    net::IpAddr,
    sync::{Mutex, MutexGuard},
    time::{Duration, Instant},
};

const ADDRESS_LIMIT: usize = 20;
const ADDRESS_WINDOW: Duration = Duration::from_secs(60);
const FREE_FAILURES: u32 = 5;
const FAILURE_WINDOW: Duration = Duration::from_mins(15);
const FIRST_DELAY: Duration = Duration::from_secs(30);
const MAX_DELAY: Duration = Duration::from_mins(15);

/// Recent sign-in attempts per client address and failures per username.
// shortcut: in-memory state assumes a single API process (ADR-0001) and resets on a restart;
// keep it in Postgres if the API ever scales out. Entries are only pruned by age, so many
// addresses spraying distinct usernames grow it; add a size cap if that is ever seen.
pub(crate) struct Throttle {
    addresses: RateLimit,
    attempts: Mutex<Attempts>,
}

impl Default for Throttle {
    fn default() -> Self {
        Self {
            addresses: RateLimit::new(ADDRESS_LIMIT, ADDRESS_WINDOW),
            attempts: Mutex::default(),
        }
    }
}

#[derive(Default)]
struct Attempts {
    failures: HashMap<String, Failures>,
}

/// At most `limit` requests from each client address in any `window`.
// shortcut: in memory, like [`Throttle`], for the same single API process.
pub(crate) struct RateLimit {
    limit: usize,
    window: Duration,
    by_address: Mutex<HashMap<Option<IpAddr>, Vec<Instant>>>,
}

impl RateLimit {
    pub(crate) fn new(limit: usize, window: Duration) -> Self {
        Self {
            limit,
            window,
            by_address: Mutex::default(),
        }
    }

    /// Admits a request from `ip`; `None` stands for every request without one.
    pub(crate) fn admit(&self, ip: Option<IpAddr>) -> bool {
        self.admit_at(ip, Instant::now())
    }

    fn admit_at(&self, ip: Option<IpAddr>, now: Instant) -> bool {
        let mut by_address = self.by_address.lock().expect("rate limit lock poisoned");
        by_address.retain(|_, times| {
            times.retain(|time| now.duration_since(*time) < self.window);
            !times.is_empty()
        });
        let times = by_address.entry(ip).or_default();
        let admitted = times.len() < self.limit;
        if admitted {
            times.push(now);
        }
        admitted
    }
}

/// A username's run of failures, each within 15 minutes of the end of the delay before it.
struct Failures {
    count: u32,
    last: Instant,
}

impl Failures {
    /// After 5 failures, 30 seconds after the last one, doubling with each further failure up
    /// to 15 minutes.
    fn delay(&self) -> Duration {
        match self.count.checked_sub(FREE_FAILURES) {
            // 30 seconds × 2⁵ already passes the cap, so the shift never needs to go higher.
            Some(extra) => FIRST_DELAY.saturating_mul(1 << extra.min(5)).min(MAX_DELAY),
            None => Duration::ZERO,
        }
    }

    fn admits_at(&self) -> Instant {
        self.last + self.delay()
    }
}

impl Throttle {
    /// Admits an attempt from `ip`, at most 20 a minute; `None` stands for every request without one.
    pub(crate) fn admit_address(&self, ip: Option<IpAddr>) -> bool {
        self.addresses.admit(ip)
    }

    /// Admits a password attempt from `ip` for `username`, which counts as a failure until
    /// [`Self::succeeded`] clears it, so parallel guesses cannot slip past the delay.
    pub(crate) fn admit_password(&self, ip: Option<IpAddr>, username: &str) -> bool {
        self.addresses.admit(ip) && self.lock().admit_username(username, Instant::now())
    }

    /// Forgets the failures of `username`, who just signed in.
    pub(crate) fn succeeded(&self, username: &str) {
        self.lock().failures.remove(key(username));
    }

    fn lock(&self) -> MutexGuard<'_, Attempts> {
        self.attempts.lock().expect("throttle lock poisoned")
    }
}

/// No username is longer than 32 characters, so a longer one is cut rather than kept whole; to
/// 33, so it never shares a key with a real username.
fn key(username: &str) -> &str {
    username
        .char_indices()
        .nth(33)
        .map_or(username, |(end, _)| &username[..end])
}

impl Attempts {
    fn admit_username(&mut self, username: &str, now: Instant) -> bool {
        // Forgotten 15 minutes after the delay ends, so a steady guesser stays at the cap.
        self.failures
            .retain(|_, failures| now < failures.admits_at() + FAILURE_WINDOW);
        let failures = self
            .failures
            .entry(key(username).to_owned())
            .or_insert(Failures {
                count: 0,
                last: now,
            });
        if now < failures.admits_at() {
            return false;
        }
        failures.count += 1;
        failures.last = now;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const fn secs(n: u64) -> Duration {
        Duration::from_secs(n)
    }

    #[test]
    fn the_delay_doubles_from_30_seconds_up_to_15_minutes() {
        let mut attempts = Attempts::default();
        let mut now = Instant::now();
        for _ in 0..5 {
            assert!(attempts.admit_username("admin", now));
        }
        for expected in [30, 60, 120, 240, 480, 900, 900] {
            assert!(!attempts.admit_username("admin", now + secs(expected - 1)));
            now += secs(expected);
            assert!(attempts.admit_username("admin", now), "{expected}");
        }
        assert!(attempts.admit_username("other", now));
    }

    #[test]
    fn failures_are_forgotten_15_minutes_after_the_delay_ends() {
        let mut attempts = Attempts::default();
        let start = Instant::now();
        for _ in 0..5 {
            assert!(attempts.admit_username("admin", start));
        }
        let forgotten = start + secs(30) + FAILURE_WINDOW;
        for _ in 0..5 {
            assert!(attempts.admit_username("admin", forgotten));
        }
        assert!(!attempts.admit_username("admin", forgotten));
    }

    #[test]
    fn an_address_gets_20_attempts_in_any_minute() {
        let addresses = Throttle::default().addresses;
        let start = Instant::now();
        let ip = Some(IpAddr::from([203, 0, 113, 7]));
        for _ in 0..20 {
            assert!(addresses.admit_at(ip, start));
        }
        assert!(!addresses.admit_at(ip, start + secs(59)));
        assert!(addresses.admit_at(None, start + secs(59)));
        assert!(addresses.admit_at(ip, start + secs(60)));
    }

    #[test]
    fn an_overlong_username_is_cut_on_a_character_boundary() {
        let long = "é".repeat(40);
        assert_eq!(key(&long).chars().count(), 33);
        assert_eq!(key("admin"), "admin");
    }
}
