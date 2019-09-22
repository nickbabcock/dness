use std::fmt;
use std::ops::{Add, AddAssign};

#[derive(Clone, Debug, Copy, Default, PartialEq, Eq)]
pub struct Updates {
    pub updated: i32,
    pub current: i32,
    pub missing: i32,
}

impl AddAssign for Updates {
    fn add_assign(&mut self, other: Self) {
        self.updated += other.updated;
        self.current += other.current;
        self.missing += other.missing;
    }
}

impl Add for Updates {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        let mut new = self;
        new += other;
        new
    }
}

impl fmt::Display for Updates {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "updated: {}, already current: {}, missing: {}",
            self.updated, self.current, self.missing
        )
    }
}
