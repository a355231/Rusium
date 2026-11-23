// Copyright 2026 The Chromium Authors
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

//! `base::Value`, `base::ValueView`, and the serializer interfaces
//! (base/values.{h,cc}).

use std::fmt;

use crate::dict::DictValue;
use crate::json;
use crate::list::ListValue;

/// `Value::BlobStorage`: binary data.
pub type BlobStorage = Vec<u8>;

/// `Value::Type`. The variant order matches the C++ enum, so the derived
/// `Ord` matches the C++ enum order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ValueType {
    None,
    Boolean,
    Integer,
    Double,
    String,
    Binary,
    Dict,
    List,
}

impl ValueType {
    /// `Value::GetTypeName`: the name used in logs and `DebugString`.
    pub fn name(self) -> &'static str {
        match self {
            ValueType::None => "null",
            ValueType::Boolean => "boolean",
            ValueType::Integer => "integer",
            ValueType::Double => "double",
            ValueType::String => "string",
            ValueType::Binary => "binary",
            ValueType::Dict => "dictionary",
            ValueType::List => "list",
        }
    }
}

impl fmt::Display for ValueType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// Storage for `Value`. The variant order matches the C++ `std::variant`
/// index order, which drives `Value`'s ordering.
#[derive(Clone, Debug, Default, PartialEq, PartialOrd)]
enum Data {
    #[default]
    None,
    Bool(bool),
    Int(i32),
    Double(f64),
    String(String),
    Blob(BlobStorage),
    Dict(DictValue),
    List(ListValue),
}

/// Replaces non-finite doubles with `0.0`, as the C++ `DoubleStorage` does.
fn sanitize_double(value: f64) -> f64 {
    if value.is_finite() {
        value
    } else {
        0.0
    }
}

/// A tagged value: none, bool, int, double, string, blob, dictionary, or list.
#[derive(Clone, Debug, Default, PartialEq, PartialOrd)]
pub struct Value {
    data: Data,
}

impl Value {
    /// `explicit Value(Type)`: a value of `value_type` with its default
    /// contents. Doubles are `0.0`, strings are empty, and so on.
    pub fn of_type(value_type: ValueType) -> Value {
        let data = match value_type {
            ValueType::None => Data::None,
            ValueType::Boolean => Data::Bool(false),
            ValueType::Integer => Data::Int(0),
            ValueType::Double => Data::Double(0.0),
            ValueType::String => Data::String(String::new()),
            ValueType::Binary => Data::Blob(Vec::new()),
            ValueType::Dict => Data::Dict(DictValue::new()),
            ValueType::List => Data::List(ListValue::new()),
        };
        Value { data }
    }

    /// `Value(std::u16string_view)`. Invalid UTF-16 becomes U+FFFD, as with
    /// `base::UTF16ToUTF8`.
    pub fn from_utf16(value: &[u16]) -> Value {
        Value::from(utf_string_conversions::utf16_to_utf8_lossy(value))
    }

    /// Returns the type of the stored value.
    pub fn value_type(&self) -> ValueType {
        match self.data {
            Data::None => ValueType::None,
            Data::Bool(_) => ValueType::Boolean,
            Data::Int(_) => ValueType::Integer,
            Data::Double(_) => ValueType::Double,
            Data::String(_) => ValueType::String,
            Data::Blob(_) => ValueType::Binary,
            Data::Dict(_) => ValueType::Dict,
            Data::List(_) => ValueType::List,
        }
    }

    pub fn is_none(&self) -> bool {
        self.value_type() == ValueType::None
    }

    pub fn is_bool(&self) -> bool {
        self.value_type() == ValueType::Boolean
    }

    pub fn is_int(&self) -> bool {
        self.value_type() == ValueType::Integer
    }

    pub fn is_double(&self) -> bool {
        self.value_type() == ValueType::Double
    }

    pub fn is_string(&self) -> bool {
        self.value_type() == ValueType::String
    }

    pub fn is_blob(&self) -> bool {
        self.value_type() == ValueType::Binary
    }

    pub fn is_dict(&self) -> bool {
        self.value_type() == ValueType::Dict
    }

    pub fn is_list(&self) -> bool {
        self.value_type() == ValueType::List
    }

    /// Returns a borrowed view of this value, for matching on its contents.
    pub fn view(&self) -> ValueView<'_> {
        match &self.data {
            Data::None => ValueView::None,
            Data::Bool(b) => ValueView::Bool(*b),
            Data::Int(i) => ValueView::Int(*i),
            Data::Double(d) => ValueView::Double(*d),
            Data::String(s) => ValueView::String(s),
            Data::Blob(b) => ValueView::Blob(b),
            Data::Dict(d) => ValueView::Dict(d),
            Data::List(l) => ValueView::List(l),
        }
    }

    // GetIf*(): `None` (or `Option::None`) when the type is wrong.

    /// `GetIfBool()`.
    pub fn get_if_bool(&self) -> Option<bool> {
        match self.data {
            Data::Bool(b) => Some(b),
            _ => None,
        }
    }

    /// `GetIfInt()`.
    pub fn get_if_int(&self) -> Option<i32> {
        match self.data {
            Data::Int(i) => Some(i),
            _ => None,
        }
    }

    /// `GetIfDouble()`. Returns a value for both `Double` and `Int`, converting
    /// the int to `f64`.
    pub fn get_if_double(&self) -> Option<f64> {
        match self.data {
            Data::Double(d) => Some(d),
            Data::Int(i) => Some(f64::from(i)),
            _ => None,
        }
    }

    /// `GetIfString()`.
    pub fn get_if_string(&self) -> Option<&str> {
        match &self.data {
            Data::String(s) => Some(s),
            _ => None,
        }
    }

    /// `GetIfString()`, mutable.
    pub fn get_if_string_mut(&mut self) -> Option<&mut String> {
        match &mut self.data {
            Data::String(s) => Some(s),
            _ => None,
        }
    }

    /// `GetIfBlob()`.
    pub fn get_if_blob(&self) -> Option<&[u8]> {
        match &self.data {
            Data::Blob(b) => Some(b),
            _ => None,
        }
    }

    /// `GetIfBlob()`, mutable.
    pub fn get_if_blob_mut(&mut self) -> Option<&mut BlobStorage> {
        match &mut self.data {
            Data::Blob(b) => Some(b),
            _ => None,
        }
    }

    /// `GetIfDict()`.
    pub fn get_if_dict(&self) -> Option<&DictValue> {
        match &self.data {
            Data::Dict(d) => Some(d),
            _ => None,
        }
    }

    /// `GetIfDict()`, mutable.
    pub fn get_if_dict_mut(&mut self) -> Option<&mut DictValue> {
        match &mut self.data {
            Data::Dict(d) => Some(d),
            _ => None,
        }
    }

    /// `GetIfList()`.
    pub fn get_if_list(&self) -> Option<&ListValue> {
        match &self.data {
            Data::List(l) => Some(l),
            _ => None,
        }
    }

    /// `GetIfList()`, mutable.
    pub fn get_if_list_mut(&mut self) -> Option<&mut ListValue> {
        match &mut self.data {
            Data::List(l) => Some(l),
            _ => None,
        }
    }

    // Get*(): panic on a type mismatch, like the C++ CHECK-on-type.

    fn type_mismatch(&self, wanted: ValueType) -> ! {
        panic!(
            "base::Value type mismatch: wanted {}, have {}",
            wanted,
            self.value_type()
        )
    }

    /// `GetBool()`. Panics if the value is not a bool.
    pub fn get_bool(&self) -> bool {
        self.get_if_bool()
            .unwrap_or_else(|| self.type_mismatch(ValueType::Boolean))
    }

    /// `GetInt()`. Panics if the value is not an int.
    pub fn get_int(&self) -> i32 {
        self.get_if_int()
            .unwrap_or_else(|| self.type_mismatch(ValueType::Integer))
    }

    /// `GetDouble()`. Returns the value for `Double` and `Int` (converted to
    /// `f64`). Panics for any other type.
    pub fn get_double(&self) -> f64 {
        self.get_if_double()
            .unwrap_or_else(|| self.type_mismatch(ValueType::Double))
    }

    /// `GetString()`. Panics if the value is not a string.
    pub fn get_string(&self) -> &str {
        self.get_if_string()
            .unwrap_or_else(|| self.type_mismatch(ValueType::String))
    }

    /// `GetString()`, mutable. Panics if the value is not a string.
    pub fn get_string_mut(&mut self) -> &mut String {
        if !self.is_string() {
            self.type_mismatch(ValueType::String);
        }
        self.get_if_string_mut()
            .unwrap_or_else(|| self.type_mismatch(ValueType::String))
    }

    /// `GetBlob()`. Panics if the value is not binary.
    pub fn get_blob(&self) -> &[u8] {
        self.get_if_blob()
            .unwrap_or_else(|| self.type_mismatch(ValueType::Binary))
    }

    /// `GetBlob()`, mutable. Panics if the value is not binary.
    pub fn get_blob_mut(&mut self) -> &mut BlobStorage {
        if !self.is_blob() {
            self.type_mismatch(ValueType::Binary);
        }
        self.get_if_blob_mut()
            .unwrap_or_else(|| self.type_mismatch(ValueType::Binary))
    }

    /// `GetDict()`. Panics if the value is not a dictionary.
    pub fn get_dict(&self) -> &DictValue {
        self.get_if_dict()
            .unwrap_or_else(|| self.type_mismatch(ValueType::Dict))
    }

    /// `GetDict()`, mutable. Panics if the value is not a dictionary.
    pub fn get_dict_mut(&mut self) -> &mut DictValue {
        if !self.is_dict() {
            self.type_mismatch(ValueType::Dict);
        }
        self.get_if_dict_mut()
            .unwrap_or_else(|| self.type_mismatch(ValueType::Dict))
    }

    /// `GetList()`. Panics if the value is not a list.
    pub fn get_list(&self) -> &ListValue {
        self.get_if_list()
            .unwrap_or_else(|| self.type_mismatch(ValueType::List))
    }

    /// `GetList()`, mutable. Panics if the value is not a list.
    pub fn get_list_mut(&mut self) -> &mut ListValue {
        if !self.is_list() {
            self.type_mismatch(ValueType::List);
        }
        self.get_if_list_mut()
            .unwrap_or_else(|| self.type_mismatch(ValueType::List))
    }

    // Take*(): consume the value and return the contents. Panic on a type
    // mismatch. After a take, the value is moved, and the variable can be
    // assigned again.

    /// `TakeString()`.
    pub fn take_string(self) -> String {
        match self.data {
            Data::String(s) => s,
            _ => self.type_mismatch(ValueType::String),
        }
    }

    /// `TakeBlob()`.
    pub fn take_blob(self) -> BlobStorage {
        match self.data {
            Data::Blob(b) => b,
            _ => self.type_mismatch(ValueType::Binary),
        }
    }

    /// `TakeDict()`.
    pub fn take_dict(self) -> DictValue {
        match self.data {
            Data::Dict(d) => d,
            _ => self.type_mismatch(ValueType::Dict),
        }
    }

    /// `TakeList()`.
    pub fn take_list(self) -> ListValue {
        match self.data {
            Data::List(l) => l,
            _ => self.type_mismatch(ValueType::List),
        }
    }

    /// `DebugString()`: pretty-printed JSON with a trailing newline, from the
    /// `JSONWriter` pretty-print path.
    pub fn debug_string(&self) -> String {
        json::debug_string(self.view())
    }
}

impl From<bool> for Value {
    fn from(value: bool) -> Value {
        Value {
            data: Data::Bool(value),
        }
    }
}

impl From<i32> for Value {
    fn from(value: i32) -> Value {
        Value {
            data: Data::Int(value),
        }
    }
}

impl From<f64> for Value {
    /// Non-finite values are stored as `0.0`.
    fn from(value: f64) -> Value {
        Value {
            data: Data::Double(sanitize_double(value)),
        }
    }
}

impl From<&str> for Value {
    fn from(value: &str) -> Value {
        Value {
            data: Data::String(value.to_owned()),
        }
    }
}

impl From<String> for Value {
    fn from(value: String) -> Value {
        Value {
            data: Data::String(value),
        }
    }
}

impl From<BlobStorage> for Value {
    fn from(value: BlobStorage) -> Value {
        Value {
            data: Data::Blob(value),
        }
    }
}

impl From<&[u8]> for Value {
    fn from(value: &[u8]) -> Value {
        Value {
            data: Data::Blob(value.to_vec()),
        }
    }
}

impl From<DictValue> for Value {
    fn from(value: DictValue) -> Value {
        Value {
            data: Data::Dict(value),
        }
    }
}

impl From<ListValue> for Value {
    fn from(value: ListValue) -> Value {
        Value {
            data: Data::List(value),
        }
    }
}

// Equality with the primitive types. Each one follows the C++ `operator==`
// overload for that type. `Value == f64` requires a `Double` and does not
// match an `Integer`, as the C++ `is_double() && GetDouble() == rhs` does.

impl PartialEq<bool> for Value {
    fn eq(&self, other: &bool) -> bool {
        matches!(self.data, Data::Bool(b) if b == *other)
    }
}

impl PartialEq<i32> for Value {
    fn eq(&self, other: &i32) -> bool {
        matches!(self.data, Data::Int(i) if i == *other)
    }
}

impl PartialEq<f64> for Value {
    fn eq(&self, other: &f64) -> bool {
        matches!(self.data, Data::Double(d) if d == *other)
    }
}

impl PartialEq<&str> for Value {
    fn eq(&self, other: &&str) -> bool {
        matches!(&self.data, Data::String(s) if s == other)
    }
}

impl PartialEq<DictValue> for Value {
    fn eq(&self, other: &DictValue) -> bool {
        matches!(&self.data, Data::Dict(d) if d == other)
    }
}

impl PartialEq<ListValue> for Value {
    fn eq(&self, other: &ListValue) -> bool {
        matches!(&self.data, Data::List(l) if l == other)
    }
}

impl PartialEq<Value> for bool {
    fn eq(&self, other: &Value) -> bool {
        other == self
    }
}

impl PartialEq<Value> for i32 {
    fn eq(&self, other: &Value) -> bool {
        other == self
    }
}

impl PartialEq<Value> for f64 {
    fn eq(&self, other: &Value) -> bool {
        other == self
    }
}

impl PartialEq<Value> for &str {
    fn eq(&self, other: &Value) -> bool {
        other == self
    }
}

impl PartialEq<Value> for DictValue {
    fn eq(&self, other: &Value) -> bool {
        other == self
    }
}

impl PartialEq<Value> for ListValue {
    fn eq(&self, other: &Value) -> bool {
        other == self
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.debug_string())
    }
}

/// `ValueView`: a borrowed, copyable view of a value. Converting a value into
/// a view does not copy its contents. Use `to_value()` to make an owned copy.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ValueView<'a> {
    None,
    Bool(bool),
    Int(i32),
    Double(f64),
    String(&'a str),
    Blob(&'a [u8]),
    Dict(&'a DictValue),
    List(&'a ListValue),
}

impl ValueView<'_> {
    /// `ValueView::ToValue()`: a deep copy of the viewed value.
    pub fn to_value(&self) -> Value {
        match *self {
            ValueView::None => Value::default(),
            ValueView::Bool(b) => Value::from(b),
            ValueView::Int(i) => Value::from(i),
            ValueView::Double(d) => Value::from(d),
            ValueView::String(s) => Value::from(s),
            ValueView::Blob(b) => Value::from(b),
            ValueView::Dict(d) => Value::from(d.clone()),
            ValueView::List(l) => Value::from(l.clone()),
        }
    }
}

impl<'a> From<&'a Value> for ValueView<'a> {
    fn from(value: &'a Value) -> ValueView<'a> {
        value.view()
    }
}

impl From<bool> for ValueView<'_> {
    fn from(value: bool) -> Self {
        ValueView::Bool(value)
    }
}

impl From<i32> for ValueView<'_> {
    fn from(value: i32) -> Self {
        ValueView::Int(value)
    }
}

impl From<f64> for ValueView<'_> {
    fn from(value: f64) -> Self {
        ValueView::Double(sanitize_double(value))
    }
}

impl<'a> From<&'a str> for ValueView<'a> {
    fn from(value: &'a str) -> Self {
        ValueView::String(value)
    }
}

impl<'a> From<&'a String> for ValueView<'a> {
    fn from(value: &'a String) -> Self {
        ValueView::String(value)
    }
}

impl<'a> From<&'a [u8]> for ValueView<'a> {
    fn from(value: &'a [u8]) -> Self {
        ValueView::Blob(value)
    }
}

impl<'a> From<&'a BlobStorage> for ValueView<'a> {
    fn from(value: &'a BlobStorage) -> Self {
        ValueView::Blob(value)
    }
}

impl<'a> From<&'a DictValue> for ValueView<'a> {
    fn from(value: &'a DictValue) -> Self {
        ValueView::Dict(value)
    }
}

impl<'a> From<&'a ListValue> for ValueView<'a> {
    fn from(value: &'a ListValue) -> Self {
        ValueView::List(value)
    }
}

/// Interface for classes that serialize `Value` trees.
pub trait ValueSerializer {
    /// Serializes `root`. Returns `true` on success.
    fn serialize(&mut self, root: ValueView<'_>) -> bool;
}

/// The error from `ValueDeserializer::deserialize`. `code` is the integer
/// error code from the C++ `ValueDeserializer::ErrorCode` scheme, and
/// `message` is the formatted error message.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeserializeError {
    pub code: i32,
    pub message: String,
}

impl DeserializeError {
    /// `kErrorCodeInvalidFormat`: generic "the data is not in the right
    /// format" error.
    pub const ERROR_CODE_INVALID_FORMAT: i32 = 1;
    /// `kErrorCodeFirstMetadataError`: codes at or above this are metadata
    /// errors (the file could not be read, the network is down, and so on).
    pub const ERROR_CODE_FIRST_METADATA_ERROR: i32 = 1000;
}

/// `ValueDeserializer::ErrorCodeIsDataError`: `true` for codes in
/// `[kErrorCodeInvalidFormat, kErrorCodeFirstMetadataError)`.
pub fn error_code_is_data_error(code: i32) -> bool {
    (DeserializeError::ERROR_CODE_INVALID_FORMAT..DeserializeError::ERROR_CODE_FIRST_METADATA_ERROR)
        .contains(&code)
}

/// Interface for classes that deserialize `Value` trees. Errors are returned
/// as `Err`, in place of the C++ `error_code` and `error_message` out-params.
pub trait ValueDeserializer {
    fn deserialize(&mut self) -> Result<Value, DeserializeError>;
}
