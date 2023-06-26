// Copyright 2019-2021 Parity Technologies (UK) Ltd.
// This file is part of substrate-desub.
//
// substrate-desub is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// substrate-desub is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with substrate-desub.  If not, see <http://www.gnu.org/licenses/>.

use super::{BitSequence, Composite, Primitive, Value, ValueDef, Variant};
use serde::{
	de::{self, EnumAccess, IntoDeserializer, SeqAccess, VariantAccess},
	forward_to_deserialize_any, ser, Deserialize, Deserializer, Serialize, Serializer,
};
use std::borrow::Cow;
use std::fmt::Display;

/*
This module implements the Deserializer trait on our Value enum
===============================================================

Deserializing using Serde is a bit weird to wrap your head around at first (at least, it was for me).
I'd def recommend checking out the serde book, and inparticular https://serde.rs/impl-deserializer.html,
but here's a very quick explainer on how things work:

We have a `Deserialize` trait (commonly automatically implemented via `#[derive(Deserialize)]`). This trait
(and the `Visitor` trait which I'll talk about in a moment) is concerned with getting the right values needed to
create an instance of the data type (struct, enum, whatever it is) in question.

We also have a `Deserializer` trait (note the R at the end). this guy is responsible for plucking values out of some
format (could be JSON or TOML or, as we have here, another rust data type!) and handing them to a Deserialize impl.
That way, the Deserialize impl doesn't have to care about any particular format; only what it wants to be given back).

So, how it works is that the `Deserialize` impl asks this guy for data of a certain type by calling methods like
`deserializer.deserialize_bool` or `deserializer.deserialize_i32` or whatever. (the actual methods available define
the "serde data model"; that is; the known types that can be passed between a Deserialize and Deserializer).

But! Calling methods like `deserialize_bool` or `deserialize_i32` is really just the Deserialize impls way of
hinting to the Deserializer what it wants to be given back. In reality, the Deserializer might want to give
back something different (maybe it is being asked for a u8 but it knows it only has a u16 to give back, say).

How? Well, the Deserialize impl calls something like `deserializer.deserialize_i32(visitor)`; it says "I want an i32, but
here's this visitor thing where you can give me back whatever you have, and I'll try and handle it if I can". So maybe
when the Deserialize impl calls `deserializer.deserialize_i32(visitor)`, the Deserializer impl for `deserialize_i32`
actually calls `visitor.visit_i64`. Who knows!

It's basically a negotiation. The Deserialize impl asks for a value of a certain type, and it provides a visitor that will
try to accept as many types as it can. The Deserializer impl then does it's best to give back what it's asked for. If
the visitor can't handle the type given back, we are given back an error trying to deserialize; we can't convert a map
into an i32 for instance, or whatever.

Here, we want to allow people to deserialize a `Value` type into some arbitrary struct or enum. So we implement the
Deserializer trait, and do our best to hand the visitor we're given back the data it's asking for. Since we know exactly
what data we actually have, we can often just give it back whatever we have and hgope the visitor will accept it! We have
various "special cases" though (like newtype wrapper structs) where we try to be more accomodating.
*/

/// An opaque error to describe in human terms what went wrong.
/// Many internal serialization/deserialization errors are relayed
/// to this in string form, and so we use basic strings for custom
/// errors as well for simplicity.
#[derive(thiserror::Error, Debug, Clone, PartialEq)]
#[error("{0}")]
pub struct Error(Cow<'static, str>);

impl Error {
	fn from_string<S: Into<String>>(s: S) -> Error {
		Error(Cow::Owned(s.into()))
	}
	fn from_str(s: &'static str) -> Error {
		Error(Cow::Borrowed(s))
	}
}

impl de::Error for Error {
	fn custom<T: Display>(msg: T) -> Self {
		Error::from_string(msg.to_string())
	}
}
impl ser::Error for Error {
	fn custom<T: Display>(msg: T) -> Self {
		Error::from_string(msg.to_string())
	}
}

/// Spit out the simple deserialize methods to avoid loads of repetition.
macro_rules! deserialize_x {
	($fn_name:ident) => {
		fn $fn_name<V>(self, visitor: V) -> Result<V::Value, Self::Error>
		where
			V: de::Visitor<'de>,
		{
			self.value.$fn_name(visitor)
		}
	};
}

// Our Value type has some context, which we ignore, and some definition, whose deserializer
// impl we forward to.
impl<'de, T> Deserializer<'de> for Value<T> {
	type Error = Error;

	deserialize_x!(deserialize_any);
	deserialize_x!(deserialize_bool);
	deserialize_x!(deserialize_i8);
	deserialize_x!(deserialize_i16);
	deserialize_x!(deserialize_i32);
	deserialize_x!(deserialize_i64);
	deserialize_x!(deserialize_i128);
	deserialize_x!(deserialize_u8);
	deserialize_x!(deserialize_u16);
	deserialize_x!(deserialize_u32);
	deserialize_x!(deserialize_u64);
	deserialize_x!(deserialize_u128);
	deserialize_x!(deserialize_f32);
	deserialize_x!(deserialize_f64);
	deserialize_x!(deserialize_char);
	deserialize_x!(deserialize_str);
	deserialize_x!(deserialize_string);
	deserialize_x!(deserialize_bytes);
	deserialize_x!(deserialize_byte_buf);
	deserialize_x!(deserialize_option);
	deserialize_x!(deserialize_unit);
	deserialize_x!(deserialize_seq);
	deserialize_x!(deserialize_map);
	deserialize_x!(deserialize_identifier);
	deserialize_x!(deserialize_ignored_any);

	fn deserialize_unit_struct<V>(self, name: &'static str, visitor: V) -> Result<V::Value, Self::Error>
	where
		V: de::Visitor<'de>,
	{
		self.value.deserialize_unit_struct(name, visitor)
	}

	fn deserialize_newtype_struct<V>(self, name: &'static str, visitor: V) -> Result<V::Value, Self::Error>
	where
		V: de::Visitor<'de>,
	{
		self.value.deserialize_newtype_struct(name, visitor)
	}

	fn deserialize_tuple<V>(self, len: usize, visitor: V) -> Result<V::Value, Self::Error>
	where
		V: de::Visitor<'de>,
	{
		self.value.deserialize_tuple(len, visitor)
	}

	fn deserialize_tuple_struct<V>(self, name: &'static str, len: usize, visitor: V) -> Result<V::Value, Self::Error>
	where
		V: de::Visitor<'de>,
	{
		self.value.deserialize_tuple_struct(name, len, visitor)
	}

	fn deserialize_struct<V>(
		self,
		name: &'static str,
		fields: &'static [&'static str],
		visitor: V,
	) -> Result<V::Value, Self::Error>
	where
		V: de::Visitor<'de>,
	{
		self.value.deserialize_struct(name, fields, visitor)
	}

	fn deserialize_enum<V>(
		self,
		name: &'static str,
		variants: &'static [&'static str],
		visitor: V,
	) -> Result<V::Value, Self::Error>
	where
		V: de::Visitor<'de>,
	{
		self.value.deserialize_enum(name, variants, visitor)
	}
}

// Our ValueDef deserializer needs to handle BitSeq itself, but otherwise delegates to
// the inner implementations of things to handle. This macro makes that less repetitive
// to write by only requiring a bitseq impl.
macro_rules! delegate_except_bitseq {
    (
        $name:ident ( $self:ident, $($arg:ident),* ),
            $seq:pat => $expr:expr
    ) => {
        match $self {
            ValueDef::BitSequence($seq) => {
                $expr
            },
            ValueDef::Composite(composite) => {
                composite.$name( $($arg),* )
            },
            ValueDef::Variant(variant) => {
                variant.$name( $($arg),* )
            },
            ValueDef::Primitive(prim) => {
                prim.$name( $($arg),* )
            },
        }
    }
}

// The goal here is simply to forward deserialization methods of interest to
// the relevant subtype. The exception is our BitSequence type, which doesn't
// have a sub type to forward to and so is handled here.
impl<'de, T> Deserializer<'de> for ValueDef<T> {
	type Error = Error;

	fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
	where
		V: serde::de::Visitor<'de>,
	{
		delegate_except_bitseq! { deserialize_any(self, visitor),
			seq => {
				BitVecPieces::new(seq)?.deserialize_any(visitor)
			}
		}
	}

	fn deserialize_newtype_struct<V>(self, name: &'static str, visitor: V) -> Result<V::Value, Self::Error>
	where
		V: de::Visitor<'de>,
	{
		delegate_except_bitseq! { deserialize_newtype_struct(self, name, visitor),
			_ => {
				Err(Error::from_str("Cannot deserialize BitSequence into a newtype struct"))
			}
		}
	}

	fn deserialize_tuple<V>(self, len: usize, visitor: V) -> Result<V::Value, Self::Error>
	where
		V: de::Visitor<'de>,
	{
		delegate_except_bitseq! { deserialize_tuple(self, len, visitor),
			_ => {
				Err(Error::from_str("Cannot deserialize BitSequence into a tuple"))
			}
		}
	}

	fn deserialize_tuple_struct<V>(self, name: &'static str, len: usize, visitor: V) -> Result<V::Value, Self::Error>
	where
		V: de::Visitor<'de>,
	{
		delegate_except_bitseq! { deserialize_tuple_struct(self, name, len, visitor),
			_ => {
				Err(Error::from_str("Cannot deserialize BitSequence into a tuple struct"))
			}
		}
	}

	fn deserialize_unit<V>(self, visitor: V) -> Result<V::Value, Self::Error>
	where
		V: de::Visitor<'de>,
	{
		delegate_except_bitseq! { deserialize_unit(self, visitor),
			_ => {
				Err(Error::from_str("Cannot deserialize BitSequence into a ()"))
			}
		}
	}

	fn deserialize_unit_struct<V>(self, name: &'static str, visitor: V) -> Result<V::Value, Self::Error>
	where
		V: de::Visitor<'de>,
	{
		delegate_except_bitseq! { deserialize_unit_struct(self, name, visitor),
			_ => {
				Err(Error::from_string(format!("Cannot deserialize BitSequence into the unit struct {}", name)))
			}
		}
	}

	fn deserialize_enum<V>(
		self,
		name: &'static str,
		variants: &'static [&'static str],
		visitor: V,
	) -> Result<V::Value, Self::Error>
	where
		V: de::Visitor<'de>,
	{
		delegate_except_bitseq! { deserialize_enum(self, name, variants, visitor),
			_ => {
				Err(Error::from_string(format!("Cannot deserialize BitSequence into the enum {}", name)))
			}
		}
	}

	fn deserialize_bytes<V>(self, visitor: V) -> Result<V::Value, Self::Error>
	where
		V: de::Visitor<'de>,
	{
		delegate_except_bitseq! { deserialize_bytes(self, visitor),
			_ => {
				Err(Error::from_str("Cannot deserialize BitSequence into raw bytes"))
			}
		}
	}

	fn deserialize_byte_buf<V>(self, visitor: V) -> Result<V::Value, Self::Error>
	where
		V: de::Visitor<'de>,
	{
		delegate_except_bitseq! { deserialize_byte_buf(self, visitor),
			_ => {
				Err(Error::from_str("Cannot deserialize BitSequence into raw bytes"))
			}
		}
	}

	fn deserialize_seq<V>(self, visitor: V) -> Result<V::Value, Self::Error>
	where
		V: de::Visitor<'de>,
	{
		delegate_except_bitseq! { deserialize_seq(self, visitor),
			_ => {
				Err(Error::from_str("Cannot deserialize BitSequence into a sequence"))
			}
		}
	}

	fn deserialize_map<V>(self, visitor: V) -> Result<V::Value, Self::Error>
	where
		V: de::Visitor<'de>,
	{
		delegate_except_bitseq! { deserialize_map(self, visitor),
			_ => {
				Err(Error::from_str("Cannot deserialize BitSequence into a map"))
			}
		}
	}

	// None of the sub types particularly care about these, so we just allow them to forward to
	// deserialize_any and go from there.
	forward_to_deserialize_any! {
		bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string
		option struct identifier ignored_any
	}
}

impl<'de, T> IntoDeserializer<'de, Error> for Value<T> {
	type Deserializer = Value<T>;
	fn into_deserializer(self) -> Self::Deserializer {
		self
	}
}

impl<'de, T> Deserializer<'de> for Composite<T> {
	type Error = Error;

	fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
	where
		V: serde::de::Visitor<'de>,
	{
		match self {
			Composite::Named(values) => visitor.visit_map(de::value::MapDeserializer::new(values.into_iter())),
			Composite::Unnamed(values) => visitor.visit_seq(de::value::SeqDeserializer::new(values.into_iter())),
		}
	}

	fn deserialize_seq<V>(self, visitor: V) -> Result<V::Value, Self::Error>
	where
		V: de::Visitor<'de>,
	{
		match self {
			Composite::Named(values) => {
				visitor.visit_seq(de::value::SeqDeserializer::new(values.into_iter().map(|(_, v)| v)))
			}
			Composite::Unnamed(values) => visitor.visit_seq(de::value::SeqDeserializer::new(values.into_iter())),
		}
	}

	fn deserialize_tuple<V>(self, len: usize, visitor: V) -> Result<V::Value, Self::Error>
	where
		V: de::Visitor<'de>,
	{
		match self {
			// A sequence of named values? just ignores the names:
			Composite::Named(values) => {
				if values.len() != len {
					return Err(Error::from_string(format!(
						"Cannot deserialize composite of length {} into tuple of length {}",
						values.len(),
						len
					)));
				}
				visitor.visit_seq(de::value::SeqDeserializer::new(values.into_iter().map(|(_, v)| v)))
			}
			// A sequence of unnamed values is ideal:
			Composite::Unnamed(values) => {
				if values.len() != len {
					return Err(Error::from_string(format!(
						"Cannot deserialize composite of length {} into tuple of length {}",
						values.len(),
						len
					)));
				}
				visitor.visit_seq(de::value::SeqDeserializer::new(values.into_iter()))
			}
		}
	}

	fn deserialize_tuple_struct<V>(self, _name: &'static str, len: usize, visitor: V) -> Result<V::Value, Self::Error>
	where
		V: de::Visitor<'de>,
	{
		self.deserialize_tuple(len, visitor)
	}

	fn deserialize_unit<V>(self, visitor: V) -> Result<V::Value, Self::Error>
	where
		V: de::Visitor<'de>,
	{
		// 0 length composite types can be treated as the unit type:
		if self.is_empty() {
			visitor.visit_unit()
		} else {
			Err(Error::from_str("Cannot deserialize non-empty Composite into a unit value"))
		}
	}

	fn deserialize_unit_struct<V>(self, _name: &'static str, visitor: V) -> Result<V::Value, Self::Error>
	where
		V: de::Visitor<'de>,
	{
		self.deserialize_unit(visitor)
	}

	fn deserialize_newtype_struct<V>(self, _name: &'static str, visitor: V) -> Result<V::Value, Self::Error>
	where
		V: de::Visitor<'de>,
	{
		visitor.visit_seq(de::value::SeqDeserializer::new(Some(self).into_iter()))
	}

	fn deserialize_byte_buf<V>(self, visitor: V) -> Result<V::Value, Self::Error>
	where
		V: de::Visitor<'de>,
	{
		match self {
			Composite::Named(values) => {
				let bytes = values
					.into_iter()
					.map(|(_n, v)| {
						if let ValueDef::Primitive(Primitive::U8(byte)) = v.value {
							Ok(byte)
						} else {
							Err(Error::from_str("Cannot deserialize composite that is not entirely U8's into bytes"))
						}
					})
					.collect::<Result<_, Error>>()?;
				visitor.visit_byte_buf(bytes)
			}
			Composite::Unnamed(values) => {
				let bytes = values
					.into_iter()
					.map(|v| {
						if let ValueDef::Primitive(Primitive::U8(byte)) = v.value {
							Ok(byte)
						} else {
							Err(Error::from_str("Cannot deserialize composite that is not entirely U8's into bytes"))
						}
					})
					.collect::<Result<_, Error>>()?;
				visitor.visit_byte_buf(bytes)
			}
		}
	}

	fn deserialize_bytes<V>(self, visitor: V) -> Result<V::Value, Self::Error>
	where
		V: de::Visitor<'de>,
	{
		self.deserialize_byte_buf(visitor)
	}

	forward_to_deserialize_any! {
		bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string
		option struct map
		enum identifier ignored_any
	}
}

impl<'de, T> IntoDeserializer<'de, Error> for Composite<T> {
	type Deserializer = Composite<T>;
	fn into_deserializer(self) -> Self::Deserializer {
		self
	}
}

// Because composite types are used to represent variant fields, we allow
// variant accesses to be called on it, which just delegate to methods defined above.
impl<'de, T> VariantAccess<'de> for Composite<T> {
	type Error = Error;

	fn unit_variant(self) -> Result<(), Self::Error> {
		Deserialize::deserialize(self)
	}

	fn newtype_variant_seed<S>(self, seed: S) -> Result<S::Value, Self::Error>
	where
		S: de::DeserializeSeed<'de>,
	{
		seed.deserialize(self)
	}

	fn tuple_variant<V>(self, len: usize, visitor: V) -> Result<V::Value, Self::Error>
	where
		V: de::Visitor<'de>,
	{
		self.deserialize_tuple(len, visitor)
	}

	fn struct_variant<V>(self, _fields: &'static [&'static str], visitor: V) -> Result<V::Value, Self::Error>
	where
		V: de::Visitor<'de>,
	{
		self.deserialize_any(visitor)
	}
}

impl<'de, T> Deserializer<'de> for Variant<T> {
	type Error = Error;

	fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
	where
		V: serde::de::Visitor<'de>,
	{
		visitor.visit_enum(self)
	}

	fn deserialize_enum<V>(
		self,
		_name: &'static str,
		_variants: &'static [&'static str],
		visitor: V,
	) -> Result<V::Value, Self::Error>
	where
		V: de::Visitor<'de>,
	{
		visitor.visit_enum(self)
	}

	fn deserialize_newtype_struct<V>(self, _name: &'static str, visitor: V) -> Result<V::Value, Self::Error>
	where
		V: de::Visitor<'de>,
	{
		visitor.visit_seq(de::value::SeqDeserializer::new(Some(self).into_iter()))
	}

	// All of the below functions delegate to the Composite deserializing methods using the enum values.

	fn deserialize_tuple<V>(self, len: usize, visitor: V) -> Result<V::Value, Self::Error>
	where
		V: de::Visitor<'de>,
	{
		self.values.deserialize_tuple(len, visitor)
	}

	fn deserialize_tuple_struct<V>(self, name: &'static str, len: usize, visitor: V) -> Result<V::Value, Self::Error>
	where
		V: de::Visitor<'de>,
	{
		self.values.deserialize_tuple_struct(name, len, visitor)
	}

	fn deserialize_unit_struct<V>(self, name: &'static str, visitor: V) -> Result<V::Value, Self::Error>
	where
		V: de::Visitor<'de>,
	{
		self.values.deserialize_unit_struct(name, visitor)
	}

	fn deserialize_unit<V>(self, visitor: V) -> Result<V::Value, Self::Error>
	where
		V: de::Visitor<'de>,
	{
		self.values.deserialize_unit(visitor)
	}

	fn deserialize_struct<V>(
		self,
		name: &'static str,
		fields: &'static [&'static str],
		visitor: V,
	) -> Result<V::Value, Self::Error>
	where
		V: de::Visitor<'de>,
	{
		self.values.deserialize_struct(name, fields, visitor)
	}

	fn deserialize_map<V>(self, visitor: V) -> Result<V::Value, Self::Error>
	where
		V: de::Visitor<'de>,
	{
		self.values.deserialize_map(visitor)
	}

	fn deserialize_seq<V>(self, visitor: V) -> Result<V::Value, Self::Error>
	where
		V: de::Visitor<'de>,
	{
		self.values.deserialize_seq(visitor)
	}

	forward_to_deserialize_any! {
		bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string
		bytes byte_buf option identifier ignored_any
	}
}

impl<'de, T> IntoDeserializer<'de, Error> for Variant<T> {
	type Deserializer = Variant<T>;
	fn into_deserializer(self) -> Self::Deserializer {
		self
	}
}

// Variant types can be treated as serde enums. Here we just hand back
// the pair of name and values, where values is a composite type that impls
// VariantAccess to actually allow deserializing of those values.
impl<'de, T> EnumAccess<'de> for Variant<T> {
	type Error = Error;

	type Variant = Composite<T>;

	fn variant_seed<V>(self, seed: V) -> Result<(V::Value, Self::Variant), Self::Error>
	where
		V: de::DeserializeSeed<'de>,
	{
		let name = self.name.into_deserializer();
		let values = self.values;
		seed.deserialize(name).map(|name| (name, values))
	}
}

impl<'de> Deserializer<'de> for Primitive {
	type Error = Error;

	fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
	where
		V: serde::de::Visitor<'de>,
	{
		match self {
			Primitive::Bool(v) => visitor.visit_bool(v),
			Primitive::Char(v) => visitor.visit_char(v),
			Primitive::Str(v) => visitor.visit_string(v),
			Primitive::U8(v) => visitor.visit_u8(v),
			Primitive::U16(v) => visitor.visit_u16(v),
			Primitive::U32(v) => visitor.visit_u32(v),
			Primitive::U64(v) => visitor.visit_u64(v),
			Primitive::U128(v) => visitor.visit_u128(v),
			Primitive::U256(v) => visitor.visit_bytes(&v),
			Primitive::I8(v) => visitor.visit_i8(v),
			Primitive::I16(v) => visitor.visit_i16(v),
			Primitive::I32(v) => visitor.visit_i32(v),
			Primitive::I64(v) => visitor.visit_i64(v),
			Primitive::I128(v) => visitor.visit_i128(v),
			Primitive::I256(v) => visitor.visit_bytes(&v),
		}
	}

	fn deserialize_newtype_struct<V>(self, _name: &'static str, visitor: V) -> Result<V::Value, Self::Error>
	where
		V: de::Visitor<'de>,
	{
		visitor.visit_seq(de::value::SeqDeserializer::new(Some(self).into_iter()))
	}

	forward_to_deserialize_any! {
		bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string
		bytes byte_buf option unit unit_struct seq tuple
		tuple_struct map struct enum identifier ignored_any
	}
}

impl<'de> IntoDeserializer<'de, Error> for Primitive {
	type Deserializer = Primitive;
	fn into_deserializer(self) -> Self::Deserializer {
		self
	}
}

/// This is a somewhat insane approach to extracting the data that we need from a
/// BitVec and allowing it to be deserialized from as part of the [`Value`] enum.
/// First, we serialize the BitVec, which grabs the relevant data out of it (that isn't
/// otherwise publically accessible), and then we implement a Deserializer that aligns
/// with what the Deserialize impl for BitVec expects.
///
/// See <https://docs.rs/bitvec/0.20.2/src/bitvec/serdes.rs.html> for the Serialize/Deserialize
/// impls we are aligning with.
struct BitVecPieces {
	head: u8,
	bits: u64,
	data: Vec<u8>,
	// Track which field we're currently deserializing:
	current_field: Option<Field>,
}

#[derive(PartialEq, Copy, Clone)]
enum Field {
	Head,
	Bits,
	Data,
}

impl<'de> Deserializer<'de> for BitVecPieces {
	type Error = Error;

	fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Self::Error>
	where
		V: de::Visitor<'de>,
	{
		// We hand back each field in order as part of a sequence, just because
		// it's the least verbose approach:
		visitor.visit_seq(self)
	}

	forward_to_deserialize_any! {
		bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string
		bytes byte_buf option unit unit_struct newtype_struct seq tuple
		tuple_struct map struct enum identifier ignored_any
	}
}

impl<'de> SeqAccess<'de> for BitVecPieces {
	type Error = Error;

	fn next_element_seed<T>(&mut self, seed: T) -> Result<Option<T::Value>, Self::Error>
	where
		T: de::DeserializeSeed<'de>,
	{
		match self.current_field {
			Some(Field::Head) => {
				let res = seed.deserialize(self.head.into_deserializer()).map(Some);
				self.current_field = Some(Field::Bits);
				res
			}
			Some(Field::Bits) => {
				let res = seed.deserialize(self.bits.into_deserializer()).map(Some);
				self.current_field = Some(Field::Data);
				res
			}
			Some(Field::Data) => {
				let bytes = std::mem::take(&mut self.data);
				let res = seed.deserialize(bytes.into_deserializer()).map(Some);
				self.current_field = None;
				res
			}
			None => Ok(None),
		}
	}
}

impl BitVecPieces {
	fn new(bit_vec: BitSequence) -> Result<BitVecPieces, Error> {
		// Step 1. "Serialize" the bitvec into this struct. Essentially,
		// we are just writing out the values we need for deserializing,
		// but with a silly amount of boilerplate/indirection..
		struct BitVecSerializer {
			head: Option<u8>,
			bits: Option<u64>,
			data: Vec<u8>,
			current_field: Option<Field>,
		}

		// Make note of what field we're trying to serialize and
		// delegate back to the main impl to actually do the work.
		impl ser::SerializeStruct for &mut BitVecSerializer {
			type Ok = ();
			type Error = Error;

			fn serialize_field<T: ?Sized + Serialize>(
				&mut self,
				key: &'static str,
				value: &T,
			) -> Result<(), Self::Error> {
				match key {
					"head" => {
						self.current_field = Some(Field::Head);
					}
					"bits" => {
						self.current_field = Some(Field::Bits);
					}
					"data" => {
						self.current_field = Some(Field::Data);
					}
					_ => {
						return Err(Error::from_string(format!(
							"BitVec serialization encountered unexpected field '{}'",
							key
						)))
					}
				}
				value.serialize(&mut **self)
			}
			fn end(self) -> Result<Self::Ok, Self::Error> {
				self.current_field = None;
				Ok(())
			}
		}

		// This is only expected to be called for serializing the data. We delegate
		// straight back to our main impl which should know what to do already, since
		// we know what struct field we're trying to serialize.
		impl ser::SerializeSeq for &mut BitVecSerializer {
			type Ok = ();
			type Error = Error;

			fn serialize_element<T: ?Sized + Serialize>(&mut self, value: &T) -> Result<(), Self::Error> {
				value.serialize(&mut **self)
			}
			fn end(self) -> Result<Self::Ok, Self::Error> {
				Ok(())
			}
		}

		// A slightly insane serializer impl whose only purpose is to be called by
		// the BitVec serialize impl, which itself only calls `serialize_struct` and
		// passes relevant data to that (so we only implement that method..)
		impl Serializer for &mut BitVecSerializer {
			type Ok = ();
			type Error = Error;

			type SerializeStruct = Self;
			type SerializeSeq = Self;

			type SerializeTuple = serde::ser::Impossible<(), Error>;
			type SerializeTupleStruct = serde::ser::Impossible<(), Error>;
			type SerializeTupleVariant = serde::ser::Impossible<(), Error>;
			type SerializeMap = serde::ser::Impossible<(), Error>;
			type SerializeStructVariant = serde::ser::Impossible<(), Error>;

			fn serialize_struct(self, _: &'static str, _: usize) -> Result<Self::SerializeStruct, Self::Error> {
				Ok(self)
			}
			fn serialize_seq(self, _: Option<usize>) -> Result<Self::SerializeSeq, Self::Error> {
				match self.current_field {
					Some(Field::Data) => Ok(self),
					_ => Err(Error::from_str(
						"BitVec serialization only expects serialize_seq to be called for 'data' prop",
					)),
				}
			}
			fn serialize_u8(self, v: u8) -> Result<Self::Ok, Self::Error> {
				match self.current_field {
					Some(Field::Head) => {
						self.head = Some(v);
						Ok(())
					}
					Some(Field::Data) => {
						self.data.push(v);
						Ok(())
					}
					_ => Err(Error::from_str(
						"BitVec serialization only expects serialize_u8 to be called for 'head' prop",
					)),
				}
			}
			fn serialize_u64(self, v: u64) -> Result<Self::Ok, Self::Error> {
				match self.current_field {
					Some(Field::Bits) => {
						self.bits = Some(v);
						Ok(())
					}
					_ => Err(Error::from_str(
						"BitVec serialization only expects serialize_u64 to be called for 'len' prop",
					)),
				}
			}

			// All of the below are never expected to be called when serializing a BitVec,
			// so we just return an error since we'd have no idea what to do!
			fn serialize_bool(self, _v: bool) -> Result<Self::Ok, Self::Error> {
				Err(Error::from_str("Unsupported BitVec serialization method"))
			}
			fn serialize_i8(self, _v: i8) -> Result<Self::Ok, Self::Error> {
				Err(Error::from_str("Unsupported BitVec serialization method"))
			}
			fn serialize_i16(self, _v: i16) -> Result<Self::Ok, Self::Error> {
				Err(Error::from_str("Unsupported BitVec serialization method"))
			}
			fn serialize_i32(self, _v: i32) -> Result<Self::Ok, Self::Error> {
				Err(Error::from_str("Unsupported BitVec serialization method"))
			}
			fn serialize_i64(self, _v: i64) -> Result<Self::Ok, Self::Error> {
				Err(Error::from_str("Unsupported BitVec serialization method"))
			}
			fn serialize_u16(self, _v: u16) -> Result<Self::Ok, Self::Error> {
				Err(Error::from_str("Unsupported BitVec serialization method"))
			}
			fn serialize_u32(self, _v: u32) -> Result<Self::Ok, Self::Error> {
				Err(Error::from_str("Unsupported BitVec serialization method"))
			}
			fn serialize_f32(self, _v: f32) -> Result<Self::Ok, Self::Error> {
				Err(Error::from_str("Unsupported BitVec serialization method"))
			}
			fn serialize_f64(self, _v: f64) -> Result<Self::Ok, Self::Error> {
				Err(Error::from_str("Unsupported BitVec serialization method"))
			}
			fn serialize_char(self, _v: char) -> Result<Self::Ok, Self::Error> {
				Err(Error::from_str("Unsupported BitVec serialization method"))
			}
			fn serialize_str(self, _v: &str) -> Result<Self::Ok, Self::Error> {
				Err(Error::from_str("Unsupported BitVec serialization method"))
			}
			fn serialize_bytes(self, _v: &[u8]) -> Result<Self::Ok, Self::Error> {
				Err(Error::from_str("Unsupported BitVec serialization method"))
			}
			fn serialize_none(self) -> Result<Self::Ok, Self::Error> {
				Err(Error::from_str("Unsupported BitVec serialization method"))
			}
			fn serialize_some<T: Serialize + ?Sized>(self, _: &T) -> Result<Self::Ok, Self::Error> {
				Err(Error::from_str("Unsupported BitVec serialization method"))
			}
			fn serialize_unit(self) -> Result<Self::Ok, Self::Error> {
				Err(Error::from_str("Unsupported BitVec serialization method"))
			}
			fn serialize_unit_struct(self, _: &'static str) -> Result<Self::Ok, Self::Error> {
				Err(Error::from_str("Unsupported BitVec serialization method"))
			}
			fn serialize_unit_variant(self, _: &'static str, _: u32, _: &'static str) -> Result<Self::Ok, Self::Error> {
				Err(Error::from_str("Unsupported BitVec serialization method"))
			}
			fn serialize_newtype_struct<T: ?Sized + Serialize>(
				self,
				_: &'static str,
				_: &T,
			) -> Result<Self::Ok, Self::Error> {
				Err(Error::from_str("Unsupported BitVec serialization method"))
			}
			fn serialize_newtype_variant<T: ?Sized + Serialize>(
				self,
				_: &'static str,
				_: u32,
				_: &'static str,
				_: &T,
			) -> Result<Self::Ok, Self::Error> {
				Err(Error::from_str("Unsupported BitVec serialization method"))
			}
			fn serialize_tuple(self, _: usize) -> Result<Self::SerializeTuple, Self::Error> {
				Err(Error::from_str("Unsupported BitVec serialization method"))
			}
			fn serialize_tuple_struct(
				self,
				_: &'static str,
				_: usize,
			) -> Result<Self::SerializeTupleStruct, Self::Error> {
				Err(Error::from_str("Unsupported BitVec serialization method"))
			}
			fn serialize_tuple_variant(
				self,
				_: &'static str,
				_: u32,
				_: &'static str,
				_: usize,
			) -> Result<Self::SerializeTupleVariant, Self::Error> {
				Err(Error::from_str("Unsupported BitVec serialization method"))
			}
			fn serialize_map(self, _: Option<usize>) -> Result<Self::SerializeMap, Self::Error> {
				Err(Error::from_str("Unsupported BitVec serialization method"))
			}
			fn serialize_struct_variant(
				self,
				_: &'static str,
				_: u32,
				_: &'static str,
				_: usize,
			) -> Result<Self::SerializeStructVariant, Self::Error> {
				Err(Error::from_str("Unsupported BitVec serialization method"))
			}
		}

		// Serialize the BitVec based on our above serializer: this basically
		// extracts the data out of it that we'll need for deserialization.
		let mut se = BitVecSerializer { head: None, bits: None, data: Vec::new(), current_field: None };
		bit_vec.serialize(&mut se)?;

		match se {
			BitVecSerializer { data, bits: Some(bits), head: Some(head), .. } => {
				Ok(BitVecPieces { data, bits, head, current_field: Some(Field::Head) })
			}
			_ => Err(Error::from_str("Could not gather together the BitVec pieces required during serialization")),
		}
	}
}

// We want to make sure that we can transform our various Value types into the sorts of output we'd expect.
#[cfg(test)]
mod test {

	use crate::value::BitSequence;
	use serde::Deserialize;

	use super::*;

	#[test]
	fn de_into_struct() {
		#[derive(Deserialize, Debug, PartialEq)]
		struct Foo {
			a: u8,
			b: bool,
		}

		let val = ValueDef::Composite(Composite::Named(vec![
			// Order shouldn't matter; match on names:
			("b".into(), Value::bool(true)),
			("a".into(), Value::u8(123)),
		]));

		assert_eq!(Foo::deserialize(val), Ok(Foo { a: 123, b: true }))
	}

	#[test]
	fn de_unwrapped_into_struct() {
		#[derive(Deserialize, Debug, PartialEq)]
		struct Foo {
			a: u8,
			b: bool,
		}

		let val = Composite::Named(vec![
			// Order shouldn't matter; match on names:
			("b".into(), Value::bool(true)),
			("a".into(), Value::u8(123)),
		]);

		assert_eq!(Foo::deserialize(val), Ok(Foo { a: 123, b: true }))
	}

	#[test]
	fn de_into_tuple_struct() {
		#[derive(Deserialize, Debug, PartialEq)]
		struct Foo(u8, bool, String);

		let val = ValueDef::Composite(Composite::Unnamed(vec![
			Value::u8(123),
			Value::bool(true),
			Value::str("hello".into()),
		]));

		assert_eq!(Foo::deserialize(val), Ok(Foo(123, true, "hello".into())))
	}

	#[test]
	fn de_unwrapped_into_tuple_struct() {
		#[derive(Deserialize, Debug, PartialEq)]
		struct Foo(u8, bool, String);

		let val = Composite::Unnamed(vec![Value::u8(123), Value::bool(true), Value::str("hello".into())]);

		assert_eq!(Foo::deserialize(val), Ok(Foo(123, true, "hello".into())))
	}

	#[test]
	fn de_into_newtype_struct() {
		#[derive(Deserialize, Debug, PartialEq)]
		struct FooStr(String);
		let val = ValueDef::<()>::Primitive(Primitive::Str("hello".into()));
		assert_eq!(FooStr::deserialize(val), Ok(FooStr("hello".into())));
		let val = Value::str("hello".into());
		assert_eq!(FooStr::deserialize(val), Ok(FooStr("hello".into())));

		#[derive(Deserialize, Debug, PartialEq)]
		struct FooVecU8(Vec<u8>);
		let val = ValueDef::Composite(Composite::Unnamed(vec![Value::u8(1), Value::u8(2), Value::u8(3)]));
		assert_eq!(FooVecU8::deserialize(val), Ok(FooVecU8(vec![1, 2, 3])));

		#[derive(Deserialize, Debug, PartialEq)]
		enum MyEnum {
			Foo(u8, u8, u8),
		}
		#[derive(Deserialize, Debug, PartialEq)]
		struct FooVar(MyEnum);
		let val = ValueDef::Variant(Variant {
			name: "Foo".into(),
			values: Composite::Unnamed(vec![Value::u8(1), Value::u8(2), Value::u8(3)]),
		});
		assert_eq!(FooVar::deserialize(val), Ok(FooVar(MyEnum::Foo(1, 2, 3))));
	}

	#[test]
	fn de_unwrapped_into_newtype_struct() {
		#[derive(Deserialize, Debug, PartialEq)]
		struct FooStr(String);
		let val = Primitive::Str("hello".into());
		assert_eq!(FooStr::deserialize(val), Ok(FooStr("hello".into())));

		#[derive(Deserialize, Debug, PartialEq)]
		struct FooVecU8(Vec<u8>);
		let val = Composite::Unnamed(vec![Value::u8(1), Value::u8(2), Value::u8(3)]);
		assert_eq!(FooVecU8::deserialize(val), Ok(FooVecU8(vec![1, 2, 3])));

		#[derive(Deserialize, Debug, PartialEq)]
		enum MyEnum {
			Foo(u8, u8, u8),
		}
		#[derive(Deserialize, Debug, PartialEq)]
		struct FooVar(MyEnum);
		let val =
			Variant { name: "Foo".into(), values: Composite::Unnamed(vec![Value::u8(1), Value::u8(2), Value::u8(3)]) };
		assert_eq!(FooVar::deserialize(val), Ok(FooVar(MyEnum::Foo(1, 2, 3))));
	}

	#[test]
	fn de_into_vec() {
		let val = ValueDef::Composite(Composite::Unnamed(vec![Value::u8(1), Value::u8(2), Value::u8(3)]));
		assert_eq!(<Vec<u8>>::deserialize(val), Ok(vec![1, 2, 3]));

		let val = ValueDef::Composite(Composite::Unnamed(vec![
			Value::str("a".into()),
			Value::str("b".into()),
			Value::str("c".into()),
		]));
		assert_eq!(<Vec<String>>::deserialize(val), Ok(vec!["a".into(), "b".into(), "c".into()]));
	}

	#[test]
	fn de_unwrapped_into_vec() {
		let val = Composite::Unnamed(vec![Value::u8(1), Value::u8(2), Value::u8(3)]);
		assert_eq!(<Vec<u8>>::deserialize(val), Ok(vec![1, 2, 3]));

		let val =
			Composite::Named(vec![("a".into(), Value::u8(1)), ("b".into(), Value::u8(2)), ("c".into(), Value::u8(3))]);
		assert_eq!(<Vec<u8>>::deserialize(val), Ok(vec![1, 2, 3]));

		let val = Composite::Unnamed(vec![Value::str("a".into()), Value::str("b".into()), Value::str("c".into())]);
		assert_eq!(<Vec<String>>::deserialize(val), Ok(vec!["a".into(), "b".into(), "c".into()]));
	}

	#[test]
	fn de_into_map() {
		use std::collections::HashMap;

		let val = ValueDef::Composite(Composite::Named(vec![
			("a".into(), Value::u8(1)),
			("b".into(), Value::u8(2)),
			("c".into(), Value::u8(3)),
		]));
		assert_eq!(
			<HashMap<String, u8>>::deserialize(val),
			Ok(vec![("a".into(), 1), ("b".into(), 2), ("c".into(), 3)].into_iter().collect())
		);

		let val = ValueDef::Composite(Composite::Unnamed(vec![Value::u8(1), Value::u8(2), Value::u8(3)]));
		<HashMap<String, u8>>::deserialize(val).expect_err("no names; can't be map");
	}

	#[test]
	fn de_into_tuple() {
		let val = ValueDef::Composite(Composite::Unnamed(vec![Value::str("hello".into()), Value::bool(true)]));
		assert_eq!(<(String, bool)>::deserialize(val), Ok(("hello".into(), true)));

		// names will just be ignored:
		let val = ValueDef::Composite(Composite::Named(vec![
			("a".into(), Value::str("hello".into())),
			("b".into(), Value::bool(true)),
		]));
		assert_eq!(<(String, bool)>::deserialize(val), Ok(("hello".into(), true)));

		// Enum variants are allowed! The variant name will be ignored:
		let val = ValueDef::Variant(Variant {
			name: "Foo".into(),
			values: Composite::Unnamed(vec![Value::str("hello".into()), Value::bool(true)]),
		});
		assert_eq!(<(String, bool)>::deserialize(val), Ok(("hello".into(), true)));

		// Enum variants with names values are allowed! The variant name will be ignored:
		let val = ValueDef::Variant(Variant {
			name: "Foo".into(),
			values: Composite::Named(vec![("a".into(), Value::str("hello".into())), ("b".into(), Value::bool(true))]),
		});
		assert_eq!(<(String, bool)>::deserialize(val), Ok(("hello".into(), true)));

		// Wrong number of values should fail:
		let val = ValueDef::Composite(Composite::Unnamed(vec![
			Value::str("hello".into()),
			Value::bool(true),
			Value::u8(123),
		]));
		<(String, bool)>::deserialize(val).expect_err("Wrong length, should err");
	}

	#[test]
	fn de_unwrapped_into_tuple() {
		let val = Composite::Unnamed(vec![Value::str("hello".into()), Value::bool(true)]);
		assert_eq!(<(String, bool)>::deserialize(val), Ok(("hello".into(), true)));

		// names will just be ignored:
		let val = Composite::Named(vec![("a".into(), Value::str("hello".into())), ("b".into(), Value::bool(true))]);
		assert_eq!(<(String, bool)>::deserialize(val), Ok(("hello".into(), true)));

		// Wrong number of values should fail:
		let val = Composite::Unnamed(vec![Value::str("hello".into()), Value::bool(true), Value::u8(123)]);
		<(String, bool)>::deserialize(val).expect_err("Wrong length, should err");
	}

	#[test]
	fn de_bitvec() {
		use bitvec::{bitvec, order::Lsb0};

		let val = Value::bit_sequence(bitvec![Lsb0, u8; 0, 1, 1, 0, 1, 0, 1, 0]);
		assert_eq!(BitSequence::deserialize(val), Ok(bitvec![Lsb0, u8; 0, 1, 1, 0, 1, 0, 1, 0]));

		let val = Value::bit_sequence(bitvec![Lsb0, u8; 0, 1, 1, 0, 1, 0, 1, 0, 1, 1, 1, 1, 0, 0, 0, 1, 0]);
		assert_eq!(
			BitSequence::deserialize(val),
			Ok(bitvec![Lsb0, u8; 0, 1, 1, 0, 1, 0, 1, 0, 1, 1, 1, 1, 0, 0, 0, 1, 0])
		);
	}

	#[test]
	fn de_into_tuple_variant() {
		#[derive(Deserialize, Debug, PartialEq)]
		enum MyEnum {
			Foo(String, bool, u8),
		}

		let val = ValueDef::Variant(Variant {
			name: "Foo".into(),
			values: Composite::Unnamed(vec![Value::str("hello".into()), Value::bool(true), Value::u8(123)]),
		});
		assert_eq!(MyEnum::deserialize(val), Ok(MyEnum::Foo("hello".into(), true, 123)));

		// it's fine to name the fields; we'll just ignore the names
		let val = ValueDef::Variant(Variant {
			name: "Foo".into(),
			values: Composite::Named(vec![
				("a".into(), Value::str("hello".into())),
				("b".into(), Value::bool(true)),
				("c".into(), Value::u8(123)),
			]),
		});
		assert_eq!(MyEnum::deserialize(val), Ok(MyEnum::Foo("hello".into(), true, 123)));
	}

	#[test]
	fn de_unwrapped_into_tuple_variant() {
		#[derive(Deserialize, Debug, PartialEq)]
		enum MyEnum {
			Foo(String, bool, u8),
		}

		let val = Variant {
			name: "Foo".into(),
			values: Composite::Unnamed(vec![Value::str("hello".into()), Value::bool(true), Value::u8(123)]),
		};
		assert_eq!(MyEnum::deserialize(val), Ok(MyEnum::Foo("hello".into(), true, 123)));

		// it's fine to name the fields; we'll just ignore the names
		let val = Variant {
			name: "Foo".into(),
			values: Composite::Named(vec![
				("a".into(), Value::str("hello".into())),
				("b".into(), Value::bool(true)),
				("c".into(), Value::u8(123)),
			]),
		};
		assert_eq!(MyEnum::deserialize(val), Ok(MyEnum::Foo("hello".into(), true, 123)));
	}

	#[test]
	fn de_into_struct_variant() {
		#[derive(Deserialize, Debug, PartialEq)]
		enum MyEnum {
			Foo { hi: String, a: bool, b: u8 },
		}

		// If names given, order doesn't matter:
		let val = ValueDef::Variant(Variant {
			name: "Foo".into(),
			values: Composite::Named(vec![
				// Deliberately out of order: names should ensure alignment:
				("b".into(), Value::u8(123)),
				("a".into(), Value::bool(true)),
				("hi".into(), Value::str("hello".into())),
			]),
		});
		assert_eq!(MyEnum::deserialize(val), Ok(MyEnum::Foo { hi: "hello".into(), a: true, b: 123 }));

		// No names needed if order is OK:
		let val = ValueDef::Variant(Variant {
			name: "Foo".into(),
			values: Composite::Unnamed(vec![Value::str("hello".into()), Value::bool(true), Value::u8(123)]),
		});
		assert_eq!(MyEnum::deserialize(val), Ok(MyEnum::Foo { hi: "hello".into(), a: true, b: 123 }));

		// Wrong order won't work if no names:
		let val = ValueDef::Variant(Variant {
			name: "Foo".into(),
			values: Composite::Unnamed(vec![Value::bool(true), Value::u8(123), Value::str("hello".into())]),
		});
		MyEnum::deserialize(val).expect_err("Wrong order shouldn't work");

		// Wrong names won't work:
		let val = ValueDef::Variant(Variant {
			name: "Foo".into(),
			values: Composite::Named(vec![
				("b".into(), Value::u8(123)),
				// Whoops; wrong name:
				("c".into(), Value::bool(true)),
				("hi".into(), Value::str("hello".into())),
			]),
		});
		MyEnum::deserialize(val).expect_err("Wrong names shouldn't work");

		// Too many names is OK; we can ignore fields we don't care about:
		let val = ValueDef::Variant(Variant {
			name: "Foo".into(),
			values: Composite::Named(vec![
				("foo".into(), Value::u8(40)),
				("b".into(), Value::u8(123)),
				("a".into(), Value::bool(true)),
				("bar".into(), Value::bool(false)),
				("hi".into(), Value::str("hello".into())),
			]),
		});
		assert_eq!(MyEnum::deserialize(val), Ok(MyEnum::Foo { hi: "hello".into(), a: true, b: 123 }));
	}

	#[test]
	fn de_into_unit_variants() {
		let val = Value::variant("Foo".into(), Composite::Named(vec![]));
		let unwrapped_val = Variant::<()> { name: "Foo".into(), values: Composite::Named(vec![]) };

		#[derive(Deserialize, Debug, PartialEq)]
		enum MyEnum {
			Foo,
		}
		assert_eq!(MyEnum::deserialize(val.clone()), Ok(MyEnum::Foo));
		assert_eq!(MyEnum::deserialize(unwrapped_val.clone()), Ok(MyEnum::Foo));

		#[derive(Deserialize, Debug, PartialEq)]
		enum MyEnum2 {
			Foo(),
		}
		assert_eq!(MyEnum2::deserialize(val.clone()), Ok(MyEnum2::Foo()));
		assert_eq!(MyEnum2::deserialize(unwrapped_val.clone()), Ok(MyEnum2::Foo()));

		#[derive(Deserialize, Debug, PartialEq)]
		enum MyEnum3 {
			Foo {},
		}
		assert_eq!(MyEnum3::deserialize(val), Ok(MyEnum3::Foo {}));
		assert_eq!(MyEnum3::deserialize(unwrapped_val), Ok(MyEnum3::Foo {}));
	}
}
