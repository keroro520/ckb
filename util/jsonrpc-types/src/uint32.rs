use ckb_types::packed;
use ckb_types::prelude::{Pack, Unpack};
use serde::de::{Error, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;

#[derive(Clone, Default, PartialEq, Eq, Hash, Debug)]
pub struct Uint32(u32);

impl Uint32 {
    pub fn value(&self) -> u32 {
        self.0
    }
}

impl From<u32> for Uint32 {
    fn from(value: u32) -> Self {
        Uint32(value)
    }
}

impl From<Uint32> for u32 {
    fn from(value: Uint32) -> Self {
        value.value()
    }
}

impl Pack<packed::Uint32> for Uint32 {
    fn pack(&self) -> packed::Uint32 {
        self.value().pack()
    }
}

impl Unpack<Uint32> for packed::Uint32 {
    fn unpack(&self) -> Uint32 {
        Uint32(Unpack::<u32>::unpack(self))
    }
}

impl Serialize for Uint32 {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&format!("0x{:x}", self.0))
    }
}

impl<'a> Deserialize<'a> for Uint32 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'a>,
    {
        deserializer.deserialize_any(Uint32Visitor)
    }
}

struct Uint32Visitor;

impl<'a> Visitor<'a> for Uint32Visitor {
    type Value = Uint32;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        write!(formatter, "a hex-encoded or decimal uint32")
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: Error,
    {
        if !value.starts_with("0x") {
            return Err(Error::custom(format!(
                "Invalid uint32 {}: without `0x` prefix",
                value
            )));
        }

        u32::from_str_radix(&value[2..], 16)
            .map(Uint32)
            .map_err(|e| Error::custom(format!("Invalid uint32 {}: {}", value, e)))
    }

    fn visit_string<E>(self, value: String) -> Result<Self::Value, E>
    where
        E: Error,
    {
        self.visit_str(&value)
    }
}

#[cfg(test)]
mod tests {
    use crate::uint32::Uint32;

    #[test]
    fn serialize() {
        assert_eq!(r#""0xd""#, serde_json::to_string(&Uint32(13)).unwrap());
        assert_eq!(r#""0x0""#, serde_json::to_string(&Uint32(0)).unwrap());
    }

    #[test]
    fn deserialize_heximal() {
        let s = r#""0xa""#;
        let deserialized: Uint32 = serde_json::from_str(s).unwrap();
        assert_eq!(deserialized, Uint32(10));
    }

    #[test]
    fn deserialize_decimal() {
        let s = r#""10""#;
        let ret: Result<Uint32, _> = serde_json::from_str(s);
        assert!(ret.is_err(), ret);
    }

    #[test]
    fn deserialize_overflow() {
        let s = format!(r#""0x{:x}""#, u128::from(u32::max_value()) + 1);
        let ret: Result<Uint32, _> = serde_json::from_str(&s);
        assert!(ret.is_err(), ret);

        let err = ret.unwrap_err();
        assert!(
            err.to_string()
                .contains("number too large to fit in target type"),
            err,
        );
    }
}