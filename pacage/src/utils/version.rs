use std::cmp::Ordering;

#[derive(Debug)]
pub struct Version {
    version: String,
    release: Option<String>,
    epoch: Option<u32>,
}

// From [epoch:]version[-release]
impl TryFrom<&str> for Version {
    type Error = &'static str;
    fn try_from(value: &str) -> Result<Self, Self::Error> {
        let (semi, minus) = match (value.find(':'), value.find('-')) {
            (Some(semi), Some(minus)) => {
                if semi > minus {
                    // If '-' is before the ':' then ':' is part of the version name
                    (None, Some(minus))
                } else {
                    (Some(semi), Some(minus))
                }
            }
            a => a,
        };
        let epoch = if let Some(semi) = semi {
            Some(
                value[..semi]
                    .parse::<u32>()
                    .or(Err("Invalid epoch number"))?,
            )
        } else {
            None
        };
        let release = if let Some(minus) = minus {
            if minus + 1 >= value.len() {
                return Err("Empty release number");
            }
            Some(value[minus + 1..].to_string())
        } else {
            None
        };
        let version =
            value[semi.map(|s| s + 1).unwrap_or(0)..minus.unwrap_or(value.len())].to_string();
        if version.is_empty() {
            return Err("Empty version");
        }
        Ok(Self {
            version,
            release,
            epoch,
        })
    }
}

impl PartialEq for Version {
    fn eq(&self, other: &Self) -> bool {
        self._cmp(other) == Ordering::Equal
    }
}
impl Eq for Version {}

impl PartialOrd for Version {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self._cmp(other))
    }
}

impl Ord for Version {
    fn cmp(&self, other: &Self) -> Ordering {
        self._cmp(other)
    }
}

impl ToString for Version {
    fn to_string(&self) -> String {
        match (self.epoch, &self.release) {
            (Some(epoch), Some(release)) => format!("{}:{}-{}", epoch, self.version, release),
            (None, Some(release)) => format!("{}-{}", self.version, release),
            (Some(epoch), None) => format!("{}:{}", epoch, self.version),
            (None, None) => self.version.clone(),
        }
    }
}

impl Version {
    pub fn new(version: &str, release: Option<&str>, epoch: Option<u32>) -> Self {
        Self {
            version: version.to_string(),
            release: release.map(|a| a.to_string()),
            epoch,
        }
    }
    pub fn _cmp(&self, other: &Self) -> Ordering {
        let self_epoch = self.epoch.unwrap_or(0);
        let other_epoch = other.epoch.unwrap_or(0);
        if self_epoch != other_epoch {
            return self_epoch.cmp(&other_epoch);
        }
        let v = Self::rpmvercmp(&self.version, &other.version);
        if v == Ordering::Equal {
            let self_release = self.release.as_deref().unwrap_or("1");
            let other_release = other.release.as_deref().unwrap_or("1");
            Self::rpmvercmp(self_release, other_release)
        } else {
            v
        }
    }
    // https://gitlab.archlinux.org/pacman/pacman/-/blob/master/lib/libalpm/version.c#L83
    fn rpmvercmp(a: &str, b: &str) -> Ordering {
        #[derive(Debug)]
        enum State {
            NonAlphaNum,
            Number(char, char),
            Alpha(char, char),
        }
        let mut state = State::NonAlphaNum;
        let mut one = a.chars();
        let mut two = b.chars();
        for i in 0..9999 {
            match state {
                State::NonAlphaNum => match (one.next(), two.next()) {
                    (Some(c_a), Some(c_b)) => {
                        match (!c_a.is_ascii_alphanumeric(), !c_b.is_ascii_alphanumeric()) {
                            (true, true) => continue,
                            (true, false) => return Ordering::Greater,
                            (false, true) => return Ordering::Less,
                            (false, false) => {}
                        }
                        if c_a.is_ascii_digit() {
                            if !c_b.is_ascii_digit() {
                                return Ordering::Greater;
                            }
                            state = State::Number(c_a, c_b);
                        } else if c_a.is_ascii_alphabetic() {
                            if !c_b.is_ascii_alphabetic() {
                                return Ordering::Less;
                            }
                            state = State::Alpha(c_a, c_b);
                        } else {
                            unreachable!(/* alphanumer but not alpha neither numeric */)
                        }
                    }
                    (Some(_), None) => return Ordering::Less,
                    (None, Some(_)) => return Ordering::Greater,
                    (None, None) => {
                        return Ordering::Less; /* arbitrary */
                    }
                },
                State::Number(mut c_a, mut c_b) => {
                    while c_a == '0' {
                        if let Some(cc_a) = one.next() {
                            c_a = cc_a;
                        } else {
                            break;
                        }
                    }
                    while c_b == '0' {
                        if let Some(cc_b) = two.next() {
                            c_b = cc_b;
                        } else {
                            break;
                        }
                    }
                    let (mut c_a, mut c_b) = (Some(c_a), Some(c_b));
                    let mut res_a: u64 = 0;
                    let mut res_b: u64 = 0;
                    for _ in 0..9999 {
                        match (c_a, c_b) {
                            (Some(c_a), Some(c_b)) => {
                                match (c_a.is_ascii_digit(), c_b.is_ascii_digit()) {
                                    (true, true) => {
                                        res_a *= 10;
                                        res_b *= 10;
                                        res_a += c_a as u8 as u64;
                                        res_b += c_b as u8 as u64;
                                    }
                                    (true, false) => return Ordering::Greater,
                                    (false, true) => return Ordering::Less,
                                    (false, false) => {
                                        if res_a != res_b {
                                            return res_a.cmp(&res_b);
                                        }
                                        match (c_a.is_ascii_alphabetic(), c_b.is_ascii_alphabetic())
                                        {
                                            (true, true) => {
                                                state = State::Alpha(c_a, c_b);
                                                break;
                                            }
                                            (true, false) => {
                                                return Ordering::Less;
                                            }
                                            (false, true) => {
                                                return Ordering::Greater;
                                            }
                                            (false, false) => {
                                                state = State::NonAlphaNum;
                                                break;
                                            }
                                        }
                                    }
                                }
                            }
                            (Some(a), None) => {
                                if a.is_ascii_alphabetic() {
                                    return Ordering::Less;
                                } else {
                                    return Ordering::Greater;
                                }
                            }
                            (None, Some(_)) => return Ordering::Less,
                            (None, None) => return res_a.cmp(&res_b),
                        }
                        (c_a, c_b) = (one.next(), two.next());
                    }
                    // panic!("Version number length is over the limit")
                }
                State::Alpha(c_a, c_b) => {
                    let (mut c_a, mut c_b) = (Some(c_a), Some(c_b));
                    for _ in 0..9999 {
                        match (c_a, c_b) {
                            (Some(c_a), Some(c_b)) => {
                                match (c_a.is_ascii_alphabetic(), c_b.is_ascii_alphabetic()) {
                                    (true, true) => {
                                        if c_a == c_b {
                                        } else {
                                            return c_a.cmp(&c_b);
                                        }
                                    }
                                    (true, false) => return Ordering::Greater,
                                    (false, true) => return Ordering::Less,
                                    (false, false) => {
                                        match (c_a.is_ascii_digit(), c_b.is_ascii_digit()) {
                                            (true, true) => state = State::Number(c_a, c_b),
                                            (true, false) => return Ordering::Less,
                                            (false, true) => return Ordering::Greater,
                                            (false, false) => state = State::NonAlphaNum,
                                        }
                                    }
                                }
                            }
                            (Some(_), None) => return Ordering::Greater,
                            (None, Some(_)) => return Ordering::Less,
                            (None, None) => return Ordering::Equal,
                        }
                        (c_a, c_b) = (one.next(), two.next());
                    }
                    // panic!("Version alpha length is over the limit")
                }
            }
        }
        panic!("Version length is over the limit")
    }

    #[cfg(test)]
    pub fn rand() -> Self {
        use fake::{faker::number::en::NumberWithFormat, Fake};
        let versioner = NumberWithFormat("^^^");
        Self {
            version: versioner.fake(),
            release: versioner.fake(),
            epoch: (1..99).fake(),
        }
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn version_cmp() {
        for (a, b, expected) in [
            ("1", "1", Ordering::Equal),
            ("1.5", "1.5", Ordering::Equal),
            ("1.5.0", "1.5.0", Ordering::Equal),
            ("1.5.1", "1.5.0", Ordering::Greater),
            // mixed length
            ("1.5.1", "1.5", Ordering::Greater),
            // with pkgrel, simple
            ("1.5.0-1", "1.5.0-1", Ordering::Equal),
            ("1.5.0-1", "1.5.0-2", Ordering::Less),
            ("1.5.0-1", "1.5.1-1", Ordering::Less),
            ("1.5.0-2", "1.5.1-1", Ordering::Less),
            // with pkgrel, mixed lengths
            ("1.5-1", "1.5.1-1", Ordering::Less),
            ("1.5-2", "1.5.1-1", Ordering::Less),
            ("1.5-2", "1.5.1-2", Ordering::Less),
            // mixed pkgrel inclusion
            ("1.5", "1.5-1", Ordering::Equal),
            ("1.5-1", "1.5", Ordering::Equal),
            ("1.1-1", "1.1", Ordering::Equal),
            ("1.0-1", "1.1", Ordering::Less),
            ("1.1-1", "1.0", Ordering::Greater),
            // alphanumeric versions
            ("1.5b-1", " 1.5-1 ", Ordering::Less),
            ("1.5b", "1.5", Ordering::Less),
            ("1.5b-1", "1.5", Ordering::Less),
            ("1.5b", "1.5.1", Ordering::Less),
            // from the manpage
            ("1.0a", "1.0alpha", Ordering::Less),
            ("1.0alpha", "1.0b", Ordering::Less),
            ("1.0b", "1.0beta", Ordering::Less),
            ("1.0beta", "1.0rc", Ordering::Less),
            ("1.0rc", "1.0", Ordering::Less),
            // going crazy? alpha-dotted versions
            ("1.5.a", "1.5", Ordering::Greater),
            ("1.5.b", "1.5.a", Ordering::Greater),
            ("1.5.1", "1.5.b", Ordering::Greater),
            // alpha dots and dashes
            ("1.5.b-1", "1.5.b", Ordering::Equal),
            ("1.5-1", "1.5.b", Ordering::Less),
            // same/similar content, differing separators
            ("2.0", "2_0", Ordering::Equal),
            ("2.0_a", "2_0.a", Ordering::Equal),
            ("2.0a", "2.0.a", Ordering::Less),
            ("2___a", "2_a", Ordering::Greater),
            // epoch included version comparisons
            ("0:1.0", "0:1.0", Ordering::Equal),
            ("0:1.0", "0:1.1", Ordering::Less),
            ("1:1.0", "0:1.0", Ordering::Greater),
            ("1:1.0", "0:1.1", Ordering::Greater),
            ("1:1.0", "2:1.1", Ordering::Less),
            // epoch + sometimes present pkgrel
            ("1:1.0", "0:1.0-1", Ordering::Greater),
            ("1:1.0-1", "0:1.1-1", Ordering::Greater),
            // epoch included on one version
            ("0:1.0", "1.0", Ordering::Equal),
            ("0:1.0", "1.1", Ordering::Less),
            ("0:1.1", "1.0", Ordering::Greater),
            ("1:1.0", "1.0", Ordering::Greater),
            ("1:1.0", "1.1", Ordering::Greater),
            ("1:1.1", "1.1", Ordering::Greater),
        ] {
            let va = Version::try_from(a).expect(&format!("Failed to parse Version for {}", a));
            let vb = Version::try_from(b).expect(&format!("Failed to parse Version for {}", b));
            assert_eq!(
                va._cmp(&vb),
                expected,
                "Comparing '{}' to '{}' parsed to {:?} and {:?}",
                a,
                b,
                va,
                vb
            );
        }
    }
}
