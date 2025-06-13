use serde::de::{self, MapAccess, Visitor};
use serde::{Deserialize, Serialize};
use serde::{Deserializer, Serializer};
use std::fmt;

/// A type that can be serialized as either a string or an object.
/// This is useful for APIs that accept both forms.
#[derive(Debug, Clone, PartialEq)]
pub enum StringOrObject<T> {
    String(String),
    Object(T),
}

impl<T> Serialize for StringOrObject<T>
where
    T: Serialize,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            StringOrObject::String(s) => serializer.serialize_str(s),
            StringOrObject::Object(obj) => obj.serialize(serializer),
        }
    }
}

impl<'de, T> Deserialize<'de> for StringOrObject<T>
where
    T: Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct StringOrObjectVisitor<T>(std::marker::PhantomData<T>);

        impl<'de, T> Visitor<'de> for StringOrObjectVisitor<T>
        where
            T: Deserialize<'de>,
        {
            type Value = StringOrObject<T>;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a string or an object")
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(StringOrObject::String(value.to_owned()))
            }

            fn visit_string<E>(self, value: String) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(StringOrObject::String(value))
            }

            fn visit_map<M>(self, map: M) -> Result<Self::Value, M::Error>
            where
                M: MapAccess<'de>,
            {
                let obj = T::deserialize(de::value::MapAccessDeserializer::new(map))?;
                Ok(StringOrObject::Object(obj))
            }
        }

        deserializer.deserialize_any(StringOrObjectVisitor(std::marker::PhantomData))
    }
}
