// Copyright 2026 The Chromium Authors
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

//! Rust port of `base/strings/string_number_conversions.{h,cc}` and
//! `string_number_conversions_internal.h`.
//!
//! Covers number-to-string conversion (`NumberToString`, `NumberToString16`,
//! `NumberToStringWithFixedPrecision`), string-to-number parsing
//! (`StringToInt` and friends, `StringToDouble`, the `HexStringTo*` family),
//! and hex encoding (`HexEncode`, `HexEncodeLower`, `AppendHexEncodedByte`,
//! `HexStringToBytes`, `HexStringToString`, `HexStringToSpan`).
//!
//! Inputs: the C++ `std::string_view` overloads take bytes, and the
//! `std::u16string_view` overloads take UTF-16 code units. The Rust functions
//! are generic over [`CodeUnit`], so `&[u8]` (narrow) and `&[u16]` (wide)
//! both work. Callers with a `&str` pass `.as_bytes()`.
//!
//! Return values: the C++ functions return `bool` and always write `*output`.
//! The Rust parsers return `Result<T, T>`: `Ok(v)` is a "perfect" conversion
//! (C++ `true`), and `Err(v)` is a failed conversion (C++ `false`) where `v` is
//! the value C++ would have written, such as the partial value, the saturated
//! value, or 0. Hex decoders return `Result<_, HexDecodeError>`. Its `partial`
//! field holds the bytes C++ would have left in `*output`.
//!
//! Differences from the C++ version:
//! - No out-parameters, `errno`, or `DCHECK(output->empty())`. The Rust
//!   `HexStringToBytes`-style functions allocate their own `Vec`.
//! - `HexStringToString` returns `Vec<u8>`, not `String`, because decoded
//!   bytes are arbitrary and need not be UTF-8.
//! - The `long`, `long long`, `unsigned long` and `size_t` overloads map to
//!   `i64`, `u64` and `usize`. `NumberToString` is one generic function over
//!   [`Number`].
//! - `NumberToStringWithFixedPrecision` takes `digits: usize`, not `int`.
//!   Negative digits have no meaning and C++ does not support them either.
//!   C++ rejects more than 100 digits by returning an empty string, and the
//!   port does the same.
//! - The C++ `NumberToStringWithFixedPrecision` formats into a 32-byte stack
//!   buffer, which overflows for long outputs (undefined behavior). The port
//!   returns the full string and never truncates.
//! - Floating-point parsing reproduces double_conversion's grammar exactly:
//!   the same leading-whitespace set, the same rules for signs, `.`, exponents
//!   and junk, the same 772-significant-digit truncation with a sticky digit,
//!   and the same clamping of huge exponents. The digits are then converted
//!   with Rust's correctly rounded `f64` parser, not double_conversion's
//!   bignum code. Both are correctly rounded, so results agree.
//! - Shortest-form output of `NumberToString(f64)` uses Rust's shortest
//!   round-trip digits (`{:e}`), which match double_conversion's SHORTEST mode.
//!   The layout rules are the same: fixed notation when the decimal exponent
//!   is in `[-6, 12)`, otherwise exponential notation with a signed exponent
//!   such as `1e+12`.
//! - Infinity and NaN produce an empty string in `NumberToString`, as in C++,
//!   because the C++ converter has no infinity or NaN symbols configured.
//! - `NumberToStringWithFixedPrecision` rounds half up on the exact binary
//!   value, the same as double_conversion's FIXED mode.
//! - Narrow whitespace follows the C++ rules. Bytes at or above 0x80 are never
//!   whitespace (C++ `char` values are negative there). For UTF-16,
//!   `StringToDouble` uses two different whitespace sets, exactly as C++ does:
//!   double_conversion's set skips leading whitespace, and `base::IsWhitespace`
//!   decides whether the input is rejected. This makes U+180E and U+FEFF
//!   accepted as leading whitespace while U+0085 is not. The integer parsers
//!   only accept ASCII whitespace.
//! - Not ported: `string_number_conversions_win.{h,cc}` (Windows-only helpers),
//!   `abseil_string_number_conversions.{h,cc}` (a separate Abseil-backed unit),
//!   `string_number_conversions_fuzzer.cc` (a LibFuzzer harness, not a unit
//!   test), and `EstimateMemoryUsage` (not applicable).

#![forbid(unsafe_code)]

use std::fmt;

/// One code unit of a string to parse: `u8` for narrow input, `u16` for
/// UTF-16 input.
pub trait CodeUnit: Copy {
    /// The numeric value of this code unit.
    fn value(self) -> u32;

    /// `base::IsAsciiWhitespace`: TAB, LF, VT, FF, CR and SPACE.
    fn is_ascii_whitespace(self) -> bool;

    /// Whitespace that double_conversion skips before a number.
    fn is_leading_space_for_double(self) -> bool;

    /// `base::IsWhitespace`, which decides whether `StringToDouble` rejects
    /// input that starts with whitespace.
    fn is_base_whitespace(self) -> bool;
}

/// ASCII whitespace per `WHITESPACE_ASCII` in `whitespace_constants.h`.
const ASCII_WHITESPACE: [u32; 6] = [0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x20];

fn is_ascii_space(c: u32) -> bool {
    ASCII_WHITESPACE.contains(&c)
}

impl CodeUnit for u8 {
    fn value(self) -> u32 {
        u32::from(self)
    }

    fn is_ascii_whitespace(self) -> bool {
        is_ascii_space(self.into())
    }

    // C++ `char` is negative for bytes >= 0x80, so none of them match the
    // ASCII-only whitespace table used by double_conversion.
    fn is_leading_space_for_double(self) -> bool {
        is_ascii_space(self.into())
    }

    fn is_base_whitespace(self) -> bool {
        is_ascii_space(self.into())
    }
}

impl CodeUnit for u16 {
    fn value(self) -> u32 {
        u32::from(self)
    }

    fn is_ascii_whitespace(self) -> bool {
        is_ascii_space(self.into())
    }

    // double_conversion's kWhitespaceTable16.
    fn is_leading_space_for_double(self) -> bool {
        let c = self.value();
        is_ascii_space(c)
            || matches!(
                c,
                160 | 5760 | 6158 | 8192..=8202 | 8232 | 8233 | 8239 | 8287 | 12288 | 65279
            )
    }

    // base::kWhitespaceUTF16: ASCII whitespace plus the Unicode list.
    fn is_base_whitespace(self) -> bool {
        let c = self.value();
        is_ascii_space(c)
            || matches!(
                c,
                0x85 | 0xA0
                    | 0x1680
                    | 0x2000..=0x200A
                    | 0x2028
                    | 0x2029
                    | 0x202F
                    | 0x205F
                    | 0x3000
            )
    }
}

fn is_char<C: CodeUnit>(c: C, ch: u8) -> bool {
    c.value() == u32::from(ch)
}

fn is_dec<C: CodeUnit>(c: C) -> bool {
    (u32::from(b'0')..=u32::from(b'9')).contains(&c.value())
}

/// Result of a best-effort conversion. `Ok` is a perfect conversion (C++
/// `true`). `Err` is a failed conversion (C++ `false`), and the payload is the
/// value C++ would have written to `*output`.
pub type Parsed<T> = Result<T, T>;

fn split<T>(parsed: Parsed<T>) -> (T, bool) {
    match parsed {
        Ok(v) => (v, true),
        Err(v) => (v, false),
    }
}

// ---------------------------------------------------------------------------
// Integer parsing (StringToIntImpl / HexStringToIntImpl)
// ---------------------------------------------------------------------------

trait Integer: Copy {
    const SIGNED: bool;
    fn zero() -> Self;
    /// Magnitude of `MAX`.
    fn max_magnitude() -> u128;
    /// Magnitude of `MIN` (zero for unsigned types).
    fn min_magnitude() -> u128;
    /// Builds `±magnitude`. The magnitude is always within range.
    fn from_magnitude(negative: bool, magnitude: u128) -> Self;
    /// `MAX` for a positive overflow, `MIN` for a negative one.
    fn saturate(negative: bool) -> Self;
}

macro_rules! impl_integer {
    ($($t:ty),*) => {$(
        impl Integer for $t {
            const SIGNED: bool = <$t>::MIN != 0;
            fn zero() -> Self {
                0
            }
            fn max_magnitude() -> u128 {
                <$t>::MAX as u128
            }
            fn min_magnitude() -> u128 {
                (<$t>::MIN as i128).unsigned_abs()
            }
            fn from_magnitude(negative: bool, magnitude: u128) -> Self {
                if negative {
                    (-(magnitude as i128)) as $t
                } else {
                    magnitude as $t
                }
            }
            fn saturate(negative: bool) -> Self {
                if negative {
                    <$t>::MIN
                } else {
                    <$t>::MAX
                }
            }
        }
    )*};
}

impl_integer!(i32, u32, i64, u64, usize);

/// `CharToDigit<BASE>`: digits, then `a`-`f` and `A`-`F` for base 16.
fn digit_value(c: u32, base: u32) -> Option<u32> {
    let d = match c {
        0x30..=0x39 => c - 0x30,
        0x61..=0x66 => c - 0x61 + 10,
        0x41..=0x46 => c - 0x41 + 10,
        _ => return None,
    };
    (d < base).then_some(d)
}

/// `StringToNumberParser::Base::Invoke`. Digits start at `begin`. The sign has
/// already been consumed.
fn parse_digits<T: Integer, C: CodeUnit>(
    input: &[C],
    begin: usize,
    base: u32,
    negative: bool,
) -> Parsed<T> {
    let end = input.len();
    if begin == end {
        return Err(T::zero());
    }
    let mut begin = begin;
    // An optional "0x"/"0X" is skipped only when more than two code units
    // remain, so a bare "0x" is rejected.
    if base == 16
        && end - begin > 2
        && is_char(input[begin], b'0')
        && (is_char(input[begin + 1], b'x') || is_char(input[begin + 1], b'X'))
    {
        begin += 2;
    }

    // Accumulate the magnitude. The C++ bounds check before `value *= base`
    // is equivalent to `magnitude * base + digit > bound`, because
    // `bound` is `MAX` or `|MIN|`, and every digit is smaller than `base`.
    let bound = if negative {
        T::min_magnitude()
    } else {
        T::max_magnitude()
    };
    let mut magnitude: u128 = 0;
    for &unit in &input[begin..] {
        let Some(digit) = digit_value(unit.value(), base) else {
            return Err(T::from_magnitude(negative, magnitude));
        };
        let next = magnitude * u128::from(base) + u128::from(digit);
        if next > bound {
            return Err(T::saturate(negative));
        }
        magnitude = next;
    }
    Ok(T::from_magnitude(negative, magnitude))
}

/// `StringToNumber`: skips leading ASCII whitespace, then an optional sign.
/// Whitespace before the sign makes the result invalid, but the value is still
/// parsed.
fn parse_integer<T: Integer, C: CodeUnit>(input: &[C], base: u32) -> Parsed<T> {
    let end = input.len();
    let mut begin = 0;
    let mut has_leading_whitespace = false;
    while begin < end && input[begin].is_ascii_whitespace() {
        has_leading_whitespace = true;
        begin += 1;
    }

    let result = if begin < end && is_char(input[begin], b'-') {
        if !T::SIGNED {
            return Err(T::zero());
        }
        parse_digits::<T, C>(input, begin + 1, base, true)
    } else {
        if begin < end && is_char(input[begin], b'+') {
            begin += 1;
        }
        parse_digits::<T, C>(input, begin, base, false)
    };

    match result {
        Ok(v) if !has_leading_whitespace => Ok(v),
        Ok(v) | Err(v) => Err(v),
    }
}

/// Decimal `StringToInt`. `Ok` on a perfect conversion, otherwise `Err` with
/// the partial or saturated value (see [`Parsed`]).
pub fn string_to_int<C: CodeUnit>(input: &[C]) -> Parsed<i32> {
    parse_integer(input, 10)
}

/// Decimal `StringToUint`. A leading `-` is rejected with a value of 0.
pub fn string_to_uint<C: CodeUnit>(input: &[C]) -> Parsed<u32> {
    parse_integer(input, 10)
}

/// Decimal `StringToInt64`.
pub fn string_to_int64<C: CodeUnit>(input: &[C]) -> Parsed<i64> {
    parse_integer(input, 10)
}

/// Decimal `StringToUint64`.
pub fn string_to_uint64<C: CodeUnit>(input: &[C]) -> Parsed<u64> {
    parse_integer(input, 10)
}

/// Decimal `StringToSizeT`.
pub fn string_to_size_t<C: CodeUnit>(input: &[C]) -> Parsed<usize> {
    parse_integer(input, 10)
}

/// `HexStringToInt`. Accepts an optional sign and an optional `0x` prefix. The
/// value must fit in `i32`.
pub fn hex_string_to_int(input: &[u8]) -> Parsed<i32> {
    parse_integer(input, 16)
}

/// `HexStringToUInt`. The value must fit in `u32`. A leading `-` is rejected.
pub fn hex_string_to_uint(input: &[u8]) -> Parsed<u32> {
    parse_integer(input, 16)
}

/// `HexStringToInt64`.
pub fn hex_string_to_int64(input: &[u8]) -> Parsed<i64> {
    parse_integer(input, 16)
}

/// `HexStringToUInt64`.
pub fn hex_string_to_uint64(input: &[u8]) -> Parsed<u64> {
    parse_integer(input, 16)
}

// ---------------------------------------------------------------------------
// Floating-point parsing (StringToDoubleImpl, double_conversion grammar)
// ---------------------------------------------------------------------------

/// double_conversion's `kMaxSignificantDigits`. Digits beyond this are
/// dropped. A sticky '1' is appended if any dropped digit was non-zero.
const MAX_SIGNIFICANT_DIGITS: usize = 772;

/// Exponent saturation used by double_conversion (`INT_MAX / 2`).
const MAX_EXPONENT_DIGITS: i64 = (i32::MAX / 2) as i64;

/// Exponents outside this range always give 0 or infinity for at most
/// `MAX_SIGNIFICANT_DIGITS + 1` digits, so clamping them is value-preserving.
const EXPONENT_CLAMP: i64 = 1 << 20;

fn signed_zero(negative: bool) -> f64 {
    if negative {
        -0.0
    } else {
        0.0
    }
}

/// Significant digits and the decimal exponent accumulated while scanning.
#[derive(Default)]
struct Mantissa {
    digits: Vec<u8>,
    exponent: i64,
    insignificant_digits: i64,
    nonzero_digit_dropped: bool,
}

impl Mantissa {
    /// `parsing_done`: folds in dropped digits, trims trailing zeros, and
    /// converts the digit string.
    fn finish(mut self, negative: bool) -> f64 {
        self.exponent += self.insignificant_digits;
        if self.nonzero_digit_dropped {
            self.digits.push(b'1');
            self.exponent -= 1;
        }
        while self.digits.last() == Some(&b'0') {
            self.digits.pop();
            self.exponent += 1;
        }
        let magnitude = if self.digits.is_empty() {
            0.0
        } else {
            let exponent = self.exponent.clamp(-EXPONENT_CLAMP, EXPONENT_CLAMP);
            let digits = std::str::from_utf8(&self.digits).expect("digits are ASCII");
            format!("{digits}e{exponent}")
                .parse::<f64>()
                .expect("digit string and exponent form a valid float literal")
        };
        if negative {
            -magnitude
        } else {
            magnitude
        }
    }
}

/// Parses an exponent starting at the `e`/`E` at `e_pos`. Returns the new
/// position. If the exponent has no digits, the position is left at `e_pos`,
/// so the `e` counts as trailing junk.
fn parse_exponent<C: CodeUnit>(input: &[C], e_pos: usize, exponent: &mut i64) -> usize {
    let end = input.len();
    let mut cur = e_pos + 1;
    if cur == end {
        return e_pos;
    }
    let mut negative = false;
    if is_char(input[cur], b'+') || is_char(input[cur], b'-') {
        negative = is_char(input[cur], b'-');
        cur += 1;
        if cur == end {
            return e_pos;
        }
    }
    if !is_dec(input[cur]) {
        return e_pos;
    }
    let mut num: i64 = 0;
    loop {
        let digit = i64::from(input[cur].value() as u8 - b'0');
        let limit = MAX_EXPONENT_DIGITS / 10;
        if num >= limit && !(num == limit && digit <= MAX_EXPONENT_DIGITS % 10) {
            num = MAX_EXPONENT_DIGITS;
        } else {
            num = num * 10 + digit;
        }
        cur += 1;
        if !(cur < end && is_dec(input[cur])) {
            break;
        }
    }
    *exponent += if negative { -num } else { num };
    cur
}

/// Scans a prefix of `input` the way double_conversion's `StringToIeee` does
/// with `ALLOW_LEADING_SPACES | ALLOW_TRAILING_JUNK` and no hex, octal,
/// infinity or NaN support. Returns the value and the number of code units
/// consumed. Junk returns `(0.0, 0)`.
fn parse_double_prefix<C: CodeUnit>(input: &[C]) -> (f64, usize) {
    const JUNK: (f64, usize) = (0.0, 0);
    let end = input.len();
    if end == 0 {
        return (0.0, 0);
    }

    let mut cur = 0;
    while cur < end && input[cur].is_leading_space_for_double() {
        cur += 1;
    }
    if cur == end {
        return (0.0, end);
    }

    let mut negative = false;
    if is_char(input[cur], b'+') || is_char(input[cur], b'-') {
        negative = is_char(input[cur], b'-');
        cur += 1;
        // No whitespace is allowed between the sign and the number.
        let mut next = cur;
        while next < end && input[next].is_leading_space_for_double() {
            next += 1;
        }
        if next == end || next != cur {
            return JUNK;
        }
        cur = next;
    }

    let mut m = Mantissa::default();
    let mut leading_zero = false;
    if is_char(input[cur], b'0') {
        cur += 1;
        if cur == end {
            return (signed_zero(negative), end);
        }
        leading_zero = true;
        while is_char(input[cur], b'0') {
            cur += 1;
            if cur == end {
                return (signed_zero(negative), end);
            }
        }
    }

    // Integer part. `finished` mirrors the `goto parsing_done` taken when the
    // end of the input is reached.
    let mut finished = false;
    while cur < end && is_dec(input[cur]) {
        if m.digits.len() < MAX_SIGNIFICANT_DIGITS {
            m.digits.push(input[cur].value() as u8);
        } else {
            m.insignificant_digits += 1;
            m.nonzero_digit_dropped |= !is_char(input[cur], b'0');
        }
        cur += 1;
        if cur == end {
            finished = true;
            break;
        }
    }

    if !finished && cur < end && is_char(input[cur], b'.') {
        cur += 1;
        if cur == end {
            if m.digits.is_empty() && !leading_zero {
                return JUNK;
            }
            finished = true;
        } else {
            if m.digits.is_empty() {
                // Zeros right after the point move into the exponent.
                while is_char(input[cur], b'0') {
                    cur += 1;
                    if cur == end {
                        return (signed_zero(negative), end);
                    }
                    m.exponent -= 1;
                }
            }
            while cur < end && is_dec(input[cur]) {
                if m.digits.len() < MAX_SIGNIFICANT_DIGITS {
                    m.digits.push(input[cur].value() as u8);
                    m.exponent -= 1;
                } else {
                    m.nonzero_digit_dropped |= !is_char(input[cur], b'0');
                }
                cur += 1;
                if cur == end {
                    finished = true;
                    break;
                }
            }
        }
    }

    if !finished {
        if !leading_zero && m.exponent == 0 && m.digits.is_empty() {
            return JUNK;
        }
        if cur < end && (is_char(input[cur], b'e') || is_char(input[cur], b'E')) {
            cur = parse_exponent(input, cur, &mut m.exponent);
        }
    }

    (m.finish(negative), cur)
}

/// `StringToDouble` for narrow and wide input.
///
/// The value must be in decimal form. Hex, `inf` and `nan` are rejected.
/// Returns `Ok` only if the whole input was consumed, the input does not start
/// with whitespace, and the value is finite. Otherwise returns `Err` with the
/// value C++ would have written. Overflow gives `±inf`, and underflow gives
/// 0 with `Ok`, as in C++.
pub fn string_to_double<C: CodeUnit>(input: &[C]) -> Parsed<f64> {
    let (value, processed) = parse_double_prefix(input);
    let ok = !input.is_empty()
        && processed == input.len()
        && !input[0].is_base_whitespace()
        && value.is_finite();
    if ok {
        Ok(value)
    } else {
        Err(value)
    }
}

// ---------------------------------------------------------------------------
// Number -> string (NumberToString, NumberToString16, fixed precision)
// ---------------------------------------------------------------------------

/// Types that `NumberToString` accepts.
pub trait Number: Copy {
    /// The decimal representation, as `NumberToString` produces it.
    fn to_decimal_string(self) -> String;
}

macro_rules! impl_number_int {
    ($($t:ty),*) => {$(
        impl Number for $t {
            fn to_decimal_string(self) -> String {
                self.to_string()
            }
        }
    )*};
}

impl_number_int!(i32, u32, i64, u64, usize);

impl Number for f64 {
    fn to_decimal_string(self) -> String {
        shortest_double_string(self)
    }
}

/// `NumberToString`: decimal ASCII.
pub fn number_to_string<T: Number>(value: T) -> String {
    value.to_decimal_string()
}

/// `NumberToString16`: the same text as UTF-16.
pub fn number_to_string16<T: Number>(value: T) -> Vec<u16> {
    value.to_decimal_string().encode_utf16().collect()
}

/// Shortest round-trip digits of a finite, non-negative `f64`, as ASCII digits
/// and the scientific exponent. For example, `1.25` gives `("125", 0)`.
fn shortest_digits(value: f64) -> (Vec<u8>, i32) {
    let text = format!("{value:e}");
    let (mantissa, exponent) = text.split_once('e').expect("LowerExp always has 'e'");
    let digits = mantissa.bytes().filter(|&b| b != b'.').collect();
    let exponent = exponent.parse().expect("LowerExp exponent is an integer");
    (digits, exponent)
}

/// double_conversion's ToShortest layout, with
/// `decimal_in_shortest_low = -6` and `decimal_in_shortest_high = 12`.
/// Non-finite values give an empty string.
fn shortest_double_string(value: f64) -> String {
    if !value.is_finite() {
        return String::new();
    }
    let (digits, exponent) = shortest_digits(value.abs());
    let len = digits.len() as i32;
    let mut out = String::new();
    if value.is_sign_negative() {
        out.push('-');
    }
    let push_digits = |out: &mut String, bytes: &[u8]| {
        out.push_str(std::str::from_utf8(bytes).expect("digits are ASCII"));
    };
    if (-6..12).contains(&exponent) {
        let point = exponent + 1;
        if point <= 0 {
            out.push_str("0.");
            out.extend(std::iter::repeat_n('0', (-point) as usize));
            push_digits(&mut out, &digits);
        } else if point >= len {
            push_digits(&mut out, &digits);
            out.extend(std::iter::repeat_n('0', (point - len) as usize));
        } else {
            push_digits(&mut out, &digits[..point as usize]);
            out.push('.');
            push_digits(&mut out, &digits[point as usize..]);
        }
    } else {
        push_digits(&mut out, &digits[..1]);
        if digits.len() > 1 {
            out.push('.');
            push_digits(&mut out, &digits[1..]);
        }
        out.push('e');
        out.push(if exponent < 0 { '-' } else { '+' });
        out.push_str(&exponent.unsigned_abs().to_string());
    }
    out
}

/// Adds one unit in the last place to ASCII digits, with carry.
fn increment_decimal(digits: &mut Vec<u8>) {
    for d in digits.iter_mut().rev() {
        if *d == b'9' {
            *d = b'0';
        } else {
            *d += 1;
            return;
        }
    }
    digits.insert(0, b'1');
}

/// `NumberToStringWithFixedPrecision`, or `None` where C++ `ToFixed` returns
/// false: non-finite values, `|value| >= 1e60`, and more than 100 digits.
fn fixed_precision_string(value: f64, digits: usize) -> Option<String> {
    const FIRST_NON_FIXED: f64 = 1e60;
    const MAX_FIXED_DIGITS_AFTER_POINT: usize = 100;
    if !value.is_finite()
        || digits > MAX_FIXED_DIGITS_AFTER_POINT
        || value >= FIRST_NON_FIXED
        || value <= -FIRST_NON_FIXED
    {
        return None;
    }
    // Every finite f64 has at most 1074 fractional decimal digits. Printing
    // that many gives the exact value, so the rounding below is exact.
    let exact = format!("{:.1074}", value.abs());
    let (int_part, frac_part) = exact.split_once('.').expect("precision > 0 prints '.'");
    let round_up = frac_part.as_bytes()[digits] >= b'5';
    let mut all: Vec<u8> = int_part
        .bytes()
        .chain(frac_part.bytes().take(digits))
        .collect();
    if round_up {
        increment_decimal(&mut all);
    }
    let int_len = all.len() - digits;
    let mut out = String::new();
    if value.is_sign_negative() {
        out.push('-');
    }
    out.push_str(std::str::from_utf8(&all[..int_len]).expect("digits are ASCII"));
    if digits > 0 {
        out.push('.');
        out.push_str(std::str::from_utf8(&all[int_len..]).expect("digits are ASCII"));
    }
    Some(out)
}

/// `NumberToStringWithFixedPrecision`: exactly `digits` digits after the
/// decimal point, rounded half up. Returns an empty string where C++ does.
pub fn number_to_string_with_fixed_precision(value: f64, digits: usize) -> String {
    fixed_precision_string(value, digits).unwrap_or_default()
}

/// `NumberToString16WithFixedPrecision`.
pub fn number_to_string16_with_fixed_precision(value: f64, digits: usize) -> Vec<u16> {
    number_to_string_with_fixed_precision(value, digits)
        .encode_utf16()
        .collect()
}

// ---------------------------------------------------------------------------
// Hex encoding and decoding
// ---------------------------------------------------------------------------

/// Appends the two hex digits of `byte` to `output`. Uppercase by default in C++.
pub fn append_hex_encoded_byte(byte: u8, output: &mut String, uppercase: bool) {
    const UPPER: &[u8; 16] = b"0123456789ABCDEF";
    const LOWER: &[u8; 16] = b"0123456789abcdef";
    let chars = if uppercase { UPPER } else { LOWER };
    output.push(char::from(chars[usize::from(byte >> 4)]));
    output.push(char::from(chars[usize::from(byte & 0xf)]));
}

/// `HexEncode`: two uppercase hex digits per byte.
pub fn hex_encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for &byte in bytes {
        append_hex_encoded_byte(byte, &mut out, true);
    }
    out
}

/// `HexEncodeLower`: two lowercase hex digits per byte.
pub fn hex_encode_lower(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for &byte in bytes {
        append_hex_encoded_byte(byte, &mut out, false);
    }
    out
}

/// Error from the hex decoders. C++ returns `false` and does not say why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HexDecodeError {
    /// Bytes decoded before the error. C++ leaves these in `*output`. It is
    /// always empty for [`hex_string_to_span`], whose output slice holds the
    /// partial bytes instead.
    pub partial: Vec<u8>,
}

impl fmt::Display for HexDecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid hex input")
    }
}

impl std::error::Error for HexDecodeError {}

fn hex_value(c: u8) -> Option<u8> {
    match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'a'..=b'f' => Some(c - b'a' + 10),
        b'A'..=b'F' => Some(c - b'A' + 10),
        _ => None,
    }
}

/// `HexStringToByteContainer`. Empty and odd-length input decode nothing.
/// Otherwise each complete pair is decoded in order, and `emit` gets the pair
/// index and byte. Decoding stops at the first invalid pair. Returns whether
/// the whole input decoded.
fn decode_pairs(input: &[u8], mut emit: impl FnMut(usize, u8)) -> bool {
    if input.is_empty() || input.len() % 2 != 0 {
        return false;
    }
    for (index, pair) in input.chunks_exact(2).enumerate() {
        let (Some(high), Some(low)) = (hex_value(pair[0]), hex_value(pair[1])) else {
            return false;
        };
        emit(index, (high << 4) | low);
    }
    true
}

/// `HexStringToBytes`. Input must be non-empty with even length and contain
/// only hex digits, with no `0x` or sign. On failure, `partial` holds the bytes
/// decoded before the error.
pub fn hex_string_to_bytes(input: &[u8]) -> Result<Vec<u8>, HexDecodeError> {
    let mut out = Vec::with_capacity(input.len() / 2);
    if decode_pairs(input, |_, byte| out.push(byte)) {
        Ok(out)
    } else {
        Err(HexDecodeError { partial: out })
    }
}

/// `HexStringToString`. Same as [`hex_string_to_bytes`], but the result is
/// raw bytes, which need not be UTF-8.
pub fn hex_string_to_string(input: &[u8]) -> Result<Vec<u8>, HexDecodeError> {
    hex_string_to_bytes(input)
}

/// `HexStringToSpan`. The output must be exactly `input.len() / 2` bytes, or
/// nothing is written. On an invalid digit, the bytes decoded so far are
/// written and the rest of `output` is left unchanged.
pub fn hex_string_to_span(input: &[u8], output: &mut [u8]) -> Result<(), HexDecodeError> {
    if input.len() / 2 != output.len() {
        return Err(HexDecodeError {
            partial: Vec::new(),
        });
    }
    if decode_pairs(input, |index, byte| output[index] = byte) {
        Ok(())
    } else {
        Err(HexDecodeError {
            partial: Vec::new(),
        })
    }
}

#[cfg(test)]
mod tests;
