use serde::{Deserialize, Serialize};
use std::ops::{Add, Div, Mul, Sub};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Price(i128);

impl Price {
    pub const PRECISION: i128 = 10_000_000_000_000_000;
    pub const ZERO: Self = Self(0);

    pub const fn new(raw: i128) -> Self {
        Self(raw)
    }
    pub fn from_float(f: f64) -> Self {
        Self((f * Self::PRECISION as f64).round() as i128)
    }
    pub fn from_rubles(rub: f64) -> Self {
        Self::from_float(rub)
    }
    pub fn as_float(&self) -> f64 {
        self.0 as f64 / Self::PRECISION as f64
    }
    pub fn raw(&self) -> i128 {
        self.0
    }
}

impl Add for Price {
    type Output = Self;
    fn add(self, other: Self) -> Self {
        Self(self.0 + other.0)
    }
}
impl Sub for Price {
    type Output = Self;
    fn sub(self, other: Self) -> Self {
        Self(self.0 - other.0)
    }
}
impl Mul<i64> for Price {
    type Output = Self;
    fn mul(self, scalar: i64) -> Self {
        Self(self.0 * scalar as i128)
    }
}
impl Div<i64> for Price {
    type Output = Self;
    fn div(self, scalar: i64) -> Self {
        Self(self.0 / scalar as i128)
    }
}
