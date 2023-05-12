//! Implements a subset of dotnet's [`Convert`](https://learn.microsoft.com/en-us/dotnet/api/system.convert?view=net-8.0) type.
use thiserror::Error;

/// Represents a Yarn value. The chosen variant corresponds to the last assignment of the value,
/// with the type being inferred from the type checker.
///
/// The type implements meaningful conversions between types through [`TryFrom`] and [`From`].
/// A failure to convert one variant to another will result in an [`InvalidCastError`].
///
/// ## Implementation Notes
///
/// Corresponds to C#'s [`Convert`](https://docs.microsoft.com/en-us/dotnet/api/system.convert?view=net-5.0) class.
#[derive(Debug, Clone, PartialEq)]
pub enum YarnValue {
    /// Any kind of Rust number, i.e. one of `f32`, `f64`, `i8`, `i16`, `i32`, `i64`, `i128`, `u8`, `u16`, `u32`, `u64`, `u128`, `usize`, `isize`.
    /// They are internally stored as `f32` through simple type casts.
    Number(f32),
    /// An owned Rust string.
    String(String),
    /// A Rust boolean.
    Boolean(bool),
}

/// Needed to ensure that the return type of a registered function is
/// able to be turned into a [`Value`], but not a [`Value`] itself.
pub trait IntoYarnValueFromNonYarnValue {
    fn into_untyped_value(self) -> YarnValue;
}

impl YarnValue {
    pub fn eq(&self, other: &Self, epsilon: f32) -> bool {
        match (self, other) {
            (Self::Number(a), Self::Number(b)) => (a - b).abs() < epsilon,
            (a, b) => a == b,
        }
    }
}

impl<T> From<&T> for YarnValue
where
    T: Copy,
    YarnValue: From<T>,
{
    fn from(value: &T) -> Self {
        Self::from(*value)
    }
}

macro_rules! impl_floating_point {
        ($($from_type:ty,)*) => {
        $(
            impl From<$from_type> for YarnValue {
                fn from(value: $from_type) -> Self {
                    Self::Number(value as f32)
                }
            }

            impl TryFrom<YarnValue> for $from_type {
                type Error = InvalidCastError;

                fn try_from(value: YarnValue) -> Result<Self, Self::Error> {
                    match value {
                        YarnValue::Number(value) => Ok(value as $from_type),
                        YarnValue::String(value) => value.parse().map_err(Into::into),
                        YarnValue::Boolean(value) => Ok(if value { 1.0 as $from_type } else { 0.0 }),
                    }
                }
            }


            impl IntoYarnValueFromNonYarnValue for $from_type {
                fn into_untyped_value(self) -> YarnValue {
                    self.into()
                }
            }
        )*
    };
}

impl_floating_point![f32, f64,];

macro_rules! impl_whole_number {
    ($($from_type:ty,)*) => {
        $(
            impl From<$from_type> for YarnValue {
                fn from(value: $from_type) -> Self {
                    Self::Number(value as f32)
                }
            }

            impl TryFrom<YarnValue> for $from_type {
                type Error = InvalidCastError;

                fn try_from(value: YarnValue) -> Result<Self, Self::Error> {
                    f32::try_from(value).map(|value| value as $from_type)
                }
            }


            impl IntoYarnValueFromNonYarnValue for $from_type {
                fn into_untyped_value(self) -> YarnValue {
                    self.into()
                }
            }
        )*
    };
}

impl_whole_number![i8, i16, i32, i64, i128, u8, u16, u32, u64, u128, usize, isize,];

impl From<YarnValue> for String {
    fn from(value: YarnValue) -> Self {
        match value {
            YarnValue::Number(value) => value.to_string(),
            YarnValue::String(value) => value,
            YarnValue::Boolean(value) => value.to_string(),
        }
    }
}

impl From<String> for YarnValue {
    fn from(value: String) -> Self {
        Self::String(value)
    }
}

impl From<&str> for YarnValue {
    fn from(value: &str) -> Self {
        Self::String(value.to_string())
    }
}

impl IntoYarnValueFromNonYarnValue for String {
    fn into_untyped_value(self) -> YarnValue {
        self.into()
    }
}

impl TryFrom<YarnValue> for bool {
    type Error = InvalidCastError;

    fn try_from(value: YarnValue) -> Result<Self, Self::Error> {
        match value {
            YarnValue::Number(value) => Ok(value != 0.0),
            YarnValue::String(value) => value.parse().map_err(Into::into),
            YarnValue::Boolean(value) => Ok(value),
        }
    }
}

impl From<bool> for YarnValue {
    fn from(value: bool) -> Self {
        Self::Boolean(value)
    }
}

impl IntoYarnValueFromNonYarnValue for bool {
    fn into_untyped_value(self) -> YarnValue {
        self.into()
    }
}

#[derive(Error, Debug)]
/// Represents a failure to convert one variant of [`YarnValue`] to a base type.
pub enum InvalidCastError {
    #[error(transparent)]
    ParseFloatError(#[from] std::num::ParseFloatError),
    #[error(transparent)]
    ParseIntError(#[from] std::num::ParseIntError),
    #[error(transparent)]
    ParseBoolError(#[from] std::str::ParseBoolError),
}