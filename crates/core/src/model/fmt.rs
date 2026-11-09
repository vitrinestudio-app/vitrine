//! Serde helpers that keep `project.yaml` terse and stable.
//!
//! - [`num`]: numbers without a fractional part are written as integers
//!   (`x: 1000`, not `x: 1000.0`).
//! - [`flow_style!`]: small value types (points, boxes, page specs …) are
//!   written as YAML flow maps (`at: { x: 1000, y: 2172 }`).
//! - [`double_option`]: distinguishes "field missing" from "field set to
//!   null" in patches (`Option<Option<T>>`).

use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// Largest integer that an `f64` represents exactly.
const MAX_EXACT_INT: f64 = 9_007_199_254_740_992.0;

/// Write an `f64` as integer when it has no fractional part.
fn write_f64<S: Serializer>(v: f64, s: S) -> Result<S::Ok, S::Error> {
    if v.fract() == 0.0 && v.abs() < MAX_EXACT_INT {
        if v == 0.0 {
            // Normalises -0.0 as well.
            return s.serialize_i64(0);
        }
        s.serialize_i64(v as i64)
    } else {
        s.serialize_f64(v)
    }
}

/// Values that contain numbers which should be written tersely.
pub trait Num {
    fn ser_num<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error>;
}

impl Num for f64 {
    fn ser_num<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        write_f64(*self, s)
    }
}

impl<T: Num> Num for Option<T> {
    fn ser_num<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            Some(v) => s.serialize_some(&NumRef(v)),
            None => s.serialize_none(),
        }
    }
}

/// Adapter that serializes a [`Num`] through [`Serialize`].
pub struct NumRef<'a, T: Num + ?Sized>(pub &'a T);

impl<T: Num + ?Sized> Serialize for NumRef<'_, T> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        self.0.ser_num(s)
    }
}

/// `serialize_with` helper for `f64`, `Option<f64>` and `Option<Option<f64>>`.
pub fn num<T: Num, S: Serializer>(v: &T, s: S) -> Result<S::Ok, S::Error> {
    v.ser_num(s)
}

/// `deserialize_with` helper for patch fields: a present field (even `null`)
/// becomes `Some(..)`, a missing one stays `None` via `#[serde(default)]`.
#[allow(clippy::option_option)]
pub fn double_option<'de, D, T>(d: D) -> Result<Option<Option<T>>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(d).map(Some)
}

thread_local! {
    /// Set while a flow collection is being serialized.
    static IN_FLOW: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Run `f` with the flow flag set, unless it is already set (then the value
/// is written plainly: inside a flow collection everything is flow anyway).
///
/// Workaround for serde-saphyr 2.1.1: a `FlowMap` nested inside another
/// `FlowMap` leaves the emitter in flow mode, so following *block*
/// collections (e.g. the next list item) are written in flow style too.
fn with_flow<S: Serializer>(
    s: S,
    plain: impl FnOnce(S) -> Result<S::Ok, S::Error>,
    flow: impl FnOnce(S) -> Result<S::Ok, S::Error>,
) -> Result<S::Ok, S::Error> {
    struct Reset;
    impl Drop for Reset {
        fn drop(&mut self) {
            IN_FLOW.with(|f| f.set(false));
        }
    }
    if IN_FLOW.with(std::cell::Cell::get) {
        plain(s)
    } else {
        IN_FLOW.with(|f| f.set(true));
        let _reset = Reset;
        flow(s)
    }
}

/// Serialize a sequence in YAML flow style (`[a, b]`).
pub fn flow_seq<T: Serialize + ?Sized, S: Serializer>(v: &T, s: S) -> Result<S::Ok, S::Error> {
    with_flow(
        s,
        |s| v.serialize(s),
        |s| serde_saphyr::FlowSeq(v).serialize(s),
    )
}

/// Serialize a map in YAML flow style (`{ a: 1 }`).
pub fn flow_map<T: Serialize + ?Sized, S: Serializer>(v: &T, s: S) -> Result<S::Ok, S::Error> {
    with_flow(
        s,
        |s| v.serialize(s),
        |s| serde_saphyr::FlowMap(v).serialize(s),
    )
}

#[allow(clippy::trivially_copy_pass_by_ref)] // serde passes references
pub(crate) fn is_zero(v: &f64) -> bool {
    *v == 0.0
}

/// Give a struct with `#[serde(remote = "Self")]` derives a `Serialize`
/// impl that writes it as YAML flow map, and a forwarding `Deserialize`.
macro_rules! flow_style {
    ($ty:ty) => {
        impl ::serde::Serialize for $ty {
            fn serialize<S: ::serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
                struct Inner<'a>(&'a $ty);
                impl ::serde::Serialize for Inner<'_> {
                    fn serialize<S: ::serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
                        <$ty>::serialize(self.0, s)
                    }
                }
                $crate::model::fmt::flow_map(&Inner(self), s)
            }
        }
        impl<'de> ::serde::Deserialize<'de> for $ty {
            fn deserialize<D: ::serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
                <$ty>::deserialize(d)
            }
        }
    };
}
pub(crate) use flow_style;

/// Implement `Display`/`FromStr`-style string serialization for a type
/// with `fn parse(&str) -> Result<Self, String>` and `Display`.
macro_rules! string_serde {
    ($ty:ty, $expecting:expr) => {
        impl ::serde::Serialize for $ty {
            fn serialize<S: ::serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
                s.collect_str(self)
            }
        }
        impl<'de> ::serde::Deserialize<'de> for $ty {
            fn deserialize<D: ::serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
                struct V;
                impl ::serde::de::Visitor<'_> for V {
                    type Value = $ty;
                    fn expecting(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
                        f.write_str($expecting)
                    }
                    fn visit_str<E: ::serde::de::Error>(self, v: &str) -> Result<$ty, E> {
                        <$ty>::parse(v).map_err(E::custom)
                    }
                }
                d.deserialize_str(V)
            }
        }
    };
}
pub(crate) use string_serde;
