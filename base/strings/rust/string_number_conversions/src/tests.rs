// Copyright 2026 The Chromium Authors
// Use of this source code is governed by a BSD-style license that can be
// found in the LICENSE file.

//! Ports of `base/strings/string_number_conversions_unittest.cc` (one Rust test
//! per C++ TEST) plus extra differential and property tests.

use super::*;

/// The UTF-16 form of `bytes`, decoded the way `UTF8ToUTF16` decodes them
/// (invalid sequences become U+FFFD).
fn wide(bytes: &[u8]) -> Vec<u16> {
    String::from_utf8_lossy(bytes).encode_utf16().collect()
}

/// `(value, ok)` for a `Parsed` result, matching the `(output, success)`
/// order of the C++ test tables.
fn split_result<T>(parsed: Parsed<T>) -> (T, bool) {
    match parsed {
        Ok(v) => (v, true),
        Err(v) => (v, false),
    }
}

/// gtest `EXPECT_DOUBLE_EQ`: values within 4 ULPs compare equal. Infinity is
/// one ULP past DBL_MAX, which is why the C++ test expects HUGE_VAL for
/// values that round to DBL_MAX.
fn assert_double_eq(actual: f64, expected: f64, context: &str) {
    fn ordered(x: f64) -> i128 {
        let bits = x.to_bits();
        if bits >> 63 == 1 {
            -i128::from(bits & !(1u64 << 63))
        } else {
            i128::from(bits)
        }
    }
    assert!(!actual.is_nan() && !expected.is_nan(), "{context}");
    let distance = (ordered(actual) - ordered(expected)).abs();
    assert!(distance <= 4, "{context}: expected {expected:?}, got {actual:?}");
}

/// Checks `$f` on both the narrow and wide form of `$input`.
macro_rules! check_both {
    ($f:path, $input:expr, $expected:expr, $success:expr) => {{
        let bytes: &[u8] = $input;
        let narrow = split_result($f(bytes));
        assert_eq!(
            narrow,
            ($expected, $success),
            "narrow input {:?}",
            String::from_utf8_lossy(bytes)
        );
        let wide_units = wide(bytes);
        let wide_result = split_result($f(&wide_units[..]));
        assert_eq!(
            wide_result,
            ($expected, $success),
            "wide input {:?}",
            String::from_utf8_lossy(bytes)
        );
    }};
}

// ---------------------------------------------------------------------------
// Ported C++ TEST cases
// ---------------------------------------------------------------------------

#[test]
fn ported_number_to_string() {
    // (value, signed expected, unsigned expected) for int and int64 inputs.
    let int_tests: [(i32, &str, &str); 4] = [
        (0, "0", "0"),
        (-1, "-1", "4294967295"),
        (i32::MAX, "2147483647", "2147483647"),
        (i32::MIN, "-2147483648", "2147483648"),
    ];
    let int64_tests: [(i64, &str, &str); 4] = [
        (0, "0", "0"),
        (-1, "-1", "18446744073709551615"),
        (i64::MAX, "9223372036854775807", "9223372036854775807"),
        (i64::MIN, "-9223372036854775808", "9223372036854775808"),
    ];

    for (num, sexpected, uexpected) in int_tests {
        assert_eq!(number_to_string(num), sexpected);
        assert_eq!(number_to_string16(num), wide(sexpected.as_bytes()));
        let unsigned = num as u32;
        assert_eq!(number_to_string(unsigned), uexpected);
        assert_eq!(number_to_string16(unsigned), wide(uexpected.as_bytes()));
    }
    for (num, sexpected, uexpected) in int64_tests {
        assert_eq!(number_to_string(num), sexpected);
        assert_eq!(number_to_string16(num), wide(sexpected.as_bytes()));
        let unsigned = num as u64;
        assert_eq!(number_to_string(unsigned), uexpected);
        assert_eq!(number_to_string16(unsigned), wide(uexpected.as_bytes()));
    }
}

#[test]
fn ported_uint64_to_string() {
    let cases: [(u64, &str); 4] =
        [(0, "0"), (42, "42"), (i32::MAX as u64, "2147483647"), (u64::MAX, "18446744073709551615")];
    for (input, output) in cases {
        assert_eq!(number_to_string(input), output);
    }
}

#[test]
fn ported_size_t_to_string() {
    let size_t_max = usize::MAX;
    let size_t_max_string = size_t_max.to_string();

    let cases: [(usize, &str); 6] = [
        (0, "0"),
        (9, "9"),
        (42, "42"),
        (i32::MAX as usize, "2147483647"),
        (2147483648, "2147483648"),
        (99999999999, "99999999999"),
    ];
    for (input, output) in cases {
        assert_eq!(number_to_string(input), output);
    }
    assert_eq!(number_to_string(size_t_max), size_t_max_string);
}

#[test]
fn ported_string_to_int() {
    let cases: &[(&[u8], i32, bool)] = &[
        (b"0", 0, true),
        (b"42", 42, true),
        (b"42\x99", 42, false),
        (b"\x9942\x99", 0, false),
        (b"-2147483648", i32::MIN, true),
        (b"2147483647", i32::MAX, true),
        (b"", 0, false),
        (b" 42", 42, false),
        (b"42 ", 42, false),
        (b"\t\n\x0B\x0C\r 42", 42, false),
        (b"blah42", 0, false),
        (b"42blah", 42, false),
        (b"blah42blah", 0, false),
        (b"-273.15", -273, false),
        (b"+98.6", 98, false),
        (b"--123", 0, false),
        (b"++123", 0, false),
        (b"-+123", 0, false),
        (b"+-123", 0, false),
        (b"-", 0, false),
        (b"-2147483649", i32::MIN, false),
        (b"-99999999999", i32::MIN, false),
        (b"2147483648", i32::MAX, false),
        (b"99999999999", i32::MAX, false),
    ];
    for &(input, output, success) in cases {
        check_both!(string_to_int, input, output, success);
    }

    // Embedded NUL: the NUL and the data after it are junk.
    let input_string: &[u8] = b"6\x006";
    assert_eq!(split_result(string_to_int(input_string)), (6, false));
    assert_eq!(split_result(string_to_int(&wide(input_string))), (6, false));

    // Fullwidth hyphen-minus (U+FF0D) is not a minus sign.
    let negative_wide_input = [0xFF4D, u16::from(b'4'), u16::from(b'2')];
    assert_eq!(split_result(string_to_int(&negative_wide_input)), (0, false));
}

#[test]
fn ported_string_to_uint() {
    let cases: &[(&[u8], u32, bool)] = &[
        (b"0", 0, true),
        (b"42", 42, true),
        (b"42\x99", 42, false),
        (b"\x9942\x99", 0, false),
        (b"-2147483648", 0, false),
        (b"2147483647", i32::MAX as u32, true),
        (b"", 0, false),
        (b" 42", 42, false),
        (b"42 ", 42, false),
        (b"\t\n\x0B\x0C\r 42", 42, false),
        (b"blah42", 0, false),
        (b"42blah", 42, false),
        (b"blah42blah", 0, false),
        (b"-273.15", 0, false),
        (b"+98.6", 98, false),
        (b"--123", 0, false),
        (b"++123", 0, false),
        (b"-+123", 0, false),
        (b"+-123", 0, false),
        (b"-", 0, false),
        (b"-2147483649", 0, false),
        (b"-99999999999", 0, false),
        (b"4294967295", u32::MAX, true),
        (b"4294967296", u32::MAX, false),
        (b"99999999999", u32::MAX, false),
    ];
    for &(input, output, success) in cases {
        check_both!(string_to_uint, input, output, success);
    }

    let input_string: &[u8] = b"6\x006";
    assert_eq!(split_result(string_to_uint(input_string)), (6, false));
    assert_eq!(split_result(string_to_uint(&wide(input_string))), (6, false));

    let negative_wide_input = [0xFF4D, u16::from(b'4'), u16::from(b'2')];
    assert_eq!(split_result(string_to_uint(&negative_wide_input)), (0, false));
}

#[test]
fn ported_string_to_int64() {
    let cases: &[(&[u8], i64, bool)] = &[
        (b"0", 0, true),
        (b"42", 42, true),
        (b"-2147483648", i32::MIN as i64, true),
        (b"2147483647", i32::MAX as i64, true),
        (b"-2147483649", -2147483649, true),
        (b"-99999999999", -99999999999, true),
        (b"2147483648", 2147483648, true),
        (b"99999999999", 99999999999, true),
        (b"9223372036854775807", i64::MAX, true),
        (b"-9223372036854775808", i64::MIN, true),
        (b"09", 9, true),
        (b"-09", -9, true),
        (b"", 0, false),
        (b" 42", 42, false),
        (b"42 ", 42, false),
        (b"0x42", 0, false),
        (b"\t\n\x0B\x0C\r 42", 42, false),
        (b"blah42", 0, false),
        (b"42blah", 42, false),
        (b"blah42blah", 0, false),
        (b"-273.15", -273, false),
        (b"+98.6", 98, false),
        (b"--123", 0, false),
        (b"++123", 0, false),
        (b"-+123", 0, false),
        (b"+-123", 0, false),
        (b"-", 0, false),
        (b"-9223372036854775809", i64::MIN, false),
        (b"-99999999999999999999", i64::MIN, false),
        (b"9223372036854775808", i64::MAX, false),
        (b"99999999999999999999", i64::MAX, false),
    ];
    for &(input, output, success) in cases {
        check_both!(string_to_int64, input, output, success);
    }

    let input_string: &[u8] = b"6\x006";
    assert_eq!(split_result(string_to_int64(input_string)), (6, false));
    assert_eq!(split_result(string_to_int64(&wide(input_string))), (6, false));
}

#[test]
fn ported_string_to_uint64() {
    let cases: &[(&[u8], u64, bool)] = &[
        (b"0", 0, true),
        (b"42", 42, true),
        (b"-2147483648", 0, false),
        (b"2147483647", i32::MAX as u64, true),
        (b"-2147483649", 0, false),
        (b"-99999999999", 0, false),
        (b"2147483648", 2147483648, true),
        (b"99999999999", 99999999999, true),
        (b"9223372036854775807", i64::MAX as u64, true),
        (b"-9223372036854775808", 0, false),
        (b"09", 9, true),
        (b"-09", 0, false),
        (b"", 0, false),
        (b" 42", 42, false),
        (b"42 ", 42, false),
        (b"0x42", 0, false),
        (b"\t\n\x0B\x0C\r 42", 42, false),
        (b"blah42", 0, false),
        (b"42blah", 42, false),
        (b"blah42blah", 0, false),
        (b"-273.15", 0, false),
        (b"+98.6", 98, false),
        (b"--123", 0, false),
        (b"++123", 0, false),
        (b"-+123", 0, false),
        (b"+-123", 0, false),
        (b"-", 0, false),
        (b"-9223372036854775809", 0, false),
        (b"-99999999999999999999", 0, false),
        (b"9223372036854775808", 9223372036854775808, true),
        (b"99999999999999999999", u64::MAX, false),
        (b"18446744073709551615", u64::MAX, true),
        (b"18446744073709551616", u64::MAX, false),
    ];
    for &(input, output, success) in cases {
        check_both!(string_to_uint64, input, output, success);
    }

    let input_string: &[u8] = b"6\x006";
    assert_eq!(split_result(string_to_uint64(input_string)), (6, false));
    assert_eq!(split_result(string_to_uint64(&wide(input_string))), (6, false));
}

#[test]
fn ported_string_to_size_t() {
    let size_t_max = usize::MAX;
    let size_t_max_string = size_t_max.to_string();

    let cases: Vec<(Vec<u8>, usize, bool)> = vec![
        (b"0".to_vec(), 0, true),
        (b"42".to_vec(), 42, true),
        (b"-2147483648".to_vec(), 0, false),
        (b"2147483647".to_vec(), i32::MAX as usize, true),
        (b"-2147483649".to_vec(), 0, false),
        (b"-99999999999".to_vec(), 0, false),
        (b"2147483648".to_vec(), 2147483648, true),
        (b"99999999999".to_vec(), 99999999999, true),
        (b"-9223372036854775808".to_vec(), 0, false),
        (b"09".to_vec(), 9, true),
        (b"-09".to_vec(), 0, false),
        (b"".to_vec(), 0, false),
        (b" 42".to_vec(), 42, false),
        (b"42 ".to_vec(), 42, false),
        (b"0x42".to_vec(), 0, false),
        (b"\t\n\x0B\x0C\r 42".to_vec(), 42, false),
        (b"blah42".to_vec(), 0, false),
        (b"42blah".to_vec(), 42, false),
        (b"blah42blah".to_vec(), 0, false),
        (b"-273.15".to_vec(), 0, false),
        (b"+98.6".to_vec(), 98, false),
        (b"--123".to_vec(), 0, false),
        (b"++123".to_vec(), 0, false),
        (b"-+123".to_vec(), 0, false),
        (b"+-123".to_vec(), 0, false),
        (b"-".to_vec(), 0, false),
        (b"-9223372036854775809".to_vec(), 0, false),
        (b"-99999999999999999999".to_vec(), 0, false),
        (b"999999999999999999999999".to_vec(), size_t_max, false),
        (size_t_max_string.into_bytes(), size_t_max, true),
    ];
    for (input, output, success) in &cases {
        check_both!(string_to_size_t, input, *output, *success);
    }

    let input_string: &[u8] = b"6\x006";
    assert_eq!(split_result(string_to_size_t(input_string)), (6, false));
    assert_eq!(split_result(string_to_size_t(&wide(input_string))), (6, false));
}

#[test]
fn ported_hex_string_to_int() {
    let cases: &[(&[u8], i64, bool)] = &[
        (b"0", 0, true),
        (b"42", 66, true),
        (b"-42", -66, true),
        (b"+42", 66, true),
        (b"7fffffff", i32::MAX as i64, true),
        (b"-80000000", i32::MIN as i64, true),
        (b"80000000", i32::MAX as i64, false), // Overflow test.
        (b"-80000001", i32::MIN as i64, false), // Underflow test.
        (b"0x42", 66, true),
        (b"-0x42", -66, true),
        (b"+0x42", 66, true),
        (b"0x7fffffff", i32::MAX as i64, true),
        (b"-0x80000000", i32::MIN as i64, true),
        (b"-80000000", i32::MIN as i64, true),
        (b"80000000", i32::MAX as i64, false),
        (b"-80000001", i32::MIN as i64, false),
        (b"0x0f", 15, true),
        (b"0f", 15, true),
        (b" 45", 0x45, false),
        (b"\t\n\x0B\x0C\r 0x45", 0x45, false),
        (b" 45", 0x45, false),
        (b"45 ", 0x45, false),
        (b"45:", 0x45, false),
        (b"efgh", 0xef, false),
        (b"0xefgh", 0xef, false),
        (b"hgfe", 0, false),
        (b"-", 0, false),
        (b"", 0, false),
        (b"0x", 0, false),
    ];
    for &(input, output, success) in cases {
        let output = output as i32;
        let (value, ok) = split_result(hex_string_to_int(input));
        assert_eq!((ok, value), (success, output), "{:?}", String::from_utf8_lossy(input));
    }

    // Embedded NUL.
    let input_string: &[u8] = b"0xc0ffee\x009";
    assert_eq!(split_result(hex_string_to_int(input_string)), (0xc0ffee, false));
}

#[test]
fn ported_hex_string_to_uint() {
    let cases: &[(&[u8], u32, bool)] = &[
        (b"0", 0, true),
        (b"42", 0x42, true),
        (b"-42", 0, false),
        (b"+42", 0x42, true),
        (b"7fffffff", i32::MAX as u32, true),
        (b"-80000000", 0, false),
        (b"ffffffff", 0xffffffff, true),
        (b"DeadBeef", 0xdeadbeef, true),
        (b"0x42", 0x42, true),
        (b"-0x42", 0, false),
        (b"+0x42", 0x42, true),
        (b"0x7fffffff", i32::MAX as u32, true),
        (b"-0x80000000", 0, false),
        (b"0xffffffff", u32::MAX, true),
        (b"0XDeadBeef", 0xdeadbeef, true),
        (b"0x7fffffffffffffff", u32::MAX, false), // Overflow test.
        (b"-0x8000000000000000", 0, false),
        (b"0x8000000000000000", u32::MAX, false), // Overflow test.
        (b"-0x8000000000000001", 0, false),
        (b"0xFFFFFFFFFFFFFFFF", u32::MAX, false), // Overflow test.
        (b"FFFFFFFFFFFFFFFF", u32::MAX, false),   // Overflow test.
        (b"0x0000000000000000", 0, true),
        (b"0000000000000000", 0, true),
        (b"1FFFFFFFFFFFFFFFF", u32::MAX, false), // Overflow test.
        (b"0x0f", 0x0f, true),
        (b"0f", 0x0f, true),
        (b" 45", 0x45, false),
        (b"\t\n\x0B\x0C\r 0x45", 0x45, false),
        (b" 45", 0x45, false),
        (b"45 ", 0x45, false),
        (b"45:", 0x45, false),
        (b"efgh", 0xef, false),
        (b"0xefgh", 0xef, false),
        (b"hgfe", 0, false),
        (b"-", 0, false),
        (b"", 0, false),
        (b"0x", 0, false),
    ];
    for &(input, output, success) in cases {
        let (value, ok) = split_result(hex_string_to_uint(input));
        assert_eq!((ok, value), (success, output), "{:?}", String::from_utf8_lossy(input));
    }

    let input_string: &[u8] = b"0xc0ffee\x009";
    assert_eq!(split_result(hex_string_to_uint(input_string)), (0xc0ffee, false));
}

#[test]
fn ported_hex_string_to_int64() {
    let cases: &[(&[u8], i64, bool)] = &[
        (b"0", 0, true),
        (b"42", 66, true),
        (b"-42", -66, true),
        (b"+42", 66, true),
        (b"40acd88557b", 4444444448123, true),
        (b"7fffffff", i32::MAX as i64, true),
        (b"-80000000", i32::MIN as i64, true),
        (b"ffffffff", 0xffffffff, true),
        (b"DeadBeef", 0xdeadbeef, true),
        (b"0x42", 66, true),
        (b"-0x42", -66, true),
        (b"+0x42", 66, true),
        (b"0x40acd88557b", 4444444448123, true),
        (b"0x7fffffff", i32::MAX as i64, true),
        (b"-0x80000000", i32::MIN as i64, true),
        (b"0xffffffff", 0xffffffff, true),
        (b"0XDeadBeef", 0xdeadbeef, true),
        (b"0x7fffffffffffffff", i64::MAX, true),
        (b"-0x8000000000000000", i64::MIN, true),
        (b"0x8000000000000000", i64::MAX, false), // Overflow test.
        (b"-0x8000000000000001", i64::MIN, false), // Underflow test.
        (b"0x0f", 15, true),
        (b"0f", 15, true),
        (b" 45", 0x45, false),
        (b"\t\n\x0B\x0C\r 0x45", 0x45, false),
        (b" 45", 0x45, false),
        (b"45 ", 0x45, false),
        (b"45:", 0x45, false),
        (b"efgh", 0xef, false),
        (b"0xefgh", 0xef, false),
        (b"hgfe", 0, false),
        (b"-", 0, false),
        (b"", 0, false),
        (b"0x", 0, false),
    ];
    for &(input, output, success) in cases {
        let (value, ok) = split_result(hex_string_to_int64(input));
        assert_eq!((ok, value), (success, output), "{:?}", String::from_utf8_lossy(input));
    }

    let input_string: &[u8] = b"0xc0ffee\x009";
    assert_eq!(split_result(hex_string_to_int64(input_string)), (0xc0ffee, false));
}

#[test]
fn ported_hex_string_to_uint64() {
    let cases: &[(&[u8], u64, bool)] = &[
        (b"0", 0, true),
        (b"42", 66, true),
        (b"-42", 0, false),
        (b"+42", 66, true),
        (b"40acd88557b", 4444444448123, true),
        (b"7fffffff", i32::MAX as u64, true),
        (b"-80000000", 0, false),
        (b"ffffffff", 0xffffffff, true),
        (b"DeadBeef", 0xdeadbeef, true),
        (b"0x42", 66, true),
        (b"-0x42", 0, false),
        (b"+0x42", 66, true),
        (b"0x40acd88557b", 4444444448123, true),
        (b"0x7fffffff", i32::MAX as u64, true),
        (b"-0x80000000", 0, false),
        (b"0xffffffff", 0xffffffff, true),
        (b"0XDeadBeef", 0xdeadbeef, true),
        (b"0x7fffffffffffffff", i64::MAX as u64, true),
        (b"-0x8000000000000000", 0, false),
        (b"0x8000000000000000", 0x8000000000000000, true),
        (b"-0x8000000000000001", 0, false),
        (b"0xFFFFFFFFFFFFFFFF", u64::MAX, true),
        (b"FFFFFFFFFFFFFFFF", u64::MAX, true),
        (b"0x0000000000000000", 0, true),
        (b"0000000000000000", 0, true),
        (b"1FFFFFFFFFFFFFFFF", u64::MAX, false), // Overflow test.
        (b"0x0f", 15, true),
        (b"0f", 15, true),
        (b" 45", 0x45, false),
        (b"\t\n\x0B\x0C\r 0x45", 0x45, false),
        (b" 45", 0x45, false),
        (b"45 ", 0x45, false),
        (b"45:", 0x45, false),
        (b"efgh", 0xef, false),
        (b"0xefgh", 0xef, false),
        (b"hgfe", 0, false),
        (b"-", 0, false),
        (b"", 0, false),
        (b"0x", 0, false),
    ];
    for &(input, output, success) in cases {
        let (value, ok) = split_result(hex_string_to_uint64(input));
        assert_eq!((ok, value), (success, output), "{:?}", String::from_utf8_lossy(input));
    }

    let input_string: &[u8] = b"0xc0ffee\x009";
    assert_eq!(split_result(hex_string_to_uint64(input_string)), (0xc0ffee, false));
}

#[test]
fn ported_hex_string_to_bytes_string_span() {
    // (input, expected decoded bytes, success)
    let cases: &[(&[u8], &[u8], bool)] = &[
        (b"0", b"", false), // odd number of characters fails
        (b"00", b"\x00", true),
        (b"42", b"\x42", true),
        (b"-42", b"", false), // any non-hex value fails
        (b"+42", b"", false),
        (b"7fffffff", b"\x7f\xff\xff\xff", true),
        (b"80000000", b"\x80\x00\x00\x00", true),
        (b"deadbeef", b"\xde\xad\xbe\xef", true),
        (b"DeadBeef", b"\xde\xad\xbe\xef", true),
        (b"0x42", b"", false), // leading 0x fails (x is not hex)
        (b"0f", b"\x0f", true),
        (b"45  ", b"\x45", false),
        (b"efgh", b"\xef", false),
        (b"", b"", false),
        (b"0123456789ABCDEF", b"\x01\x23\x45\x67\x89\xAB\xCD\xEF", true),
        (b"0123456789ABCDEF012345", b"\x01\x23\x45\x67\x89\xAB\xCD\xEF\x01\x23\x45", true),
    ];

    for (test_i, &(input, expected, success)) in cases.iter().enumerate() {
        let context = format!("{test_i}: {}", String::from_utf8_lossy(input));

        // hex_string_to_bytes: on failure, `partial` holds what was decoded.
        match hex_string_to_bytes(input) {
            Ok(bytes) => {
                assert!(success, "{context}");
                assert_eq!(bytes, expected, "{context}");
            }
            Err(err) => {
                assert!(!success, "{context}");
                assert_eq!(err.partial, expected, "{context}");
            }
        }

        // hex_string_to_string: same result as bytes, as raw bytes.
        match hex_string_to_string(input) {
            Ok(bytes) => {
                assert!(success, "{context}");
                assert_eq!(bytes, expected, "{context}");
            }
            Err(err) => {
                assert!(!success, "{context}");
                assert_eq!(err.partial, expected, "{context}");
            }
        }

        // hex_string_to_span with a correctly sized output. On failure, the
        // decoded prefix is written and the rest stays zero.
        {
            let mut output = vec![0u8; input.len() / 2];
            assert_eq!(hex_string_to_span(input, &mut output).is_ok(), success, "{context}");
            for (i, &byte) in expected.iter().enumerate() {
                assert_eq!(output[i], byte, "{context}");
            }
            for &byte in &output[expected.len()..] {
                assert_eq!(byte, 0, "{context}");
            }
        }

        // One byte too small: nothing is written and the call fails.
        {
            let mut output =
                if input.len() > 1 { vec![0u8; input.len() / 2 - 1] } else { Vec::new() };
            assert!(hex_string_to_span(input, &mut output).is_err(), "{context}");
        }

        // One byte too large: fails.
        {
            let mut output = vec![0u8; input.len() / 2 + 1];
            assert!(hex_string_to_span(input, &mut output).is_err(), "{context}");
        }
    }
}

/// gtest `EXPECT_DOUBLE_EQ` on a `Parsed<f64>`.
fn assert_parsed_double(parsed: Parsed<f64>, expected: f64, success: bool, context: &str) {
    let (value, ok) = split_result(parsed);
    assert_eq!(ok, success, "{context}");
    assert_double_eq(value, expected, context);
}

// The float literals below are copied verbatim from the C++ tables, so their
// extra digits and the 3.14 value are intentional.
#[test]
#[allow(clippy::excessive_precision, clippy::approx_constant)]
fn ported_string_to_double() {
    let cases: &[(&str, f64, bool)] = &[
        // Different forms of zero.
        ("0", 0.0, true),
        ("+0", 0.0, true),
        ("-0", 0.0, true),
        ("0.0", 0.0, true),
        ("000000000000000000000000000000.0", 0.0, true),
        ("0.000000000000000000000000000", 0.0, true),
        // The answer.
        ("42", 42.0, true),
        ("-42", -42.0, true),
        // Variants of an ordinary number.
        ("123.45", 123.45, true),
        ("-123.45", -123.45, true),
        ("+123.45", 123.45, true),
        // Different forms of representation.
        ("2.99792458e8", 299792458.0, true),
        ("149597870.691E+3", 149597870691.0, true),
        ("6.", 6.0, true),
        // Around the largest/smallest value a double can represent.
        ("9e307", 9e307, true),
        ("1.7976e308", 1.7976e308, true),
        ("1.7977e308", f64::INFINITY, false),
        ("1.797693134862315807e+308", f64::INFINITY, true),
        ("1.797693134862315808e+308", f64::INFINITY, false),
        ("9e308", f64::INFINITY, false),
        ("9e309", f64::INFINITY, false),
        ("9e999", f64::INFINITY, false),
        ("9e1999", f64::INFINITY, false),
        ("9e19999", f64::INFINITY, false),
        ("9e99999999999999999999", f64::INFINITY, false),
        ("-9e307", -9e307, true),
        ("-1.7976e308", -1.7976e308, true),
        ("-1.7977e308", f64::NEG_INFINITY, false),
        ("-1.797693134862315807e+308", f64::NEG_INFINITY, true),
        ("-1.797693134862315808e+308", f64::NEG_INFINITY, false),
        ("-9e308", f64::NEG_INFINITY, false),
        ("-9e309", f64::NEG_INFINITY, false),
        ("-9e999", f64::NEG_INFINITY, false),
        ("-9e1999", f64::NEG_INFINITY, false),
        ("-9e19999", f64::NEG_INFINITY, false),
        ("-9e99999999999999999999", f64::NEG_INFINITY, false),
        // More exponents.
        ("1e-2", 0.01, true),
        ("42 ", 42.0, false),
        (" 1e-2", 0.01, false),
        ("1e-2 ", 0.01, false),
        ("-1E-7", -0.0000001, true),
        ("01e02", 100.0, true),
        ("2.3e15", 2.3e15, true),
        ("100e-309", 100e-309, true),
        // Invalid cases.
        ("\x09\x0A\x0B\x0C\x0D -123.45e2", -12345.0, false),
        ("+123 e4", 123.0, false),
        ("123e ", 123.0, false),
        ("123e", 123.0, false),
        ("10.5px", 10.5, false),
        ("11.5e2em", 1150.0, false),
        (" 2.99", 2.99, false),
        ("1e3.4", 1000.0, false),
        ("nothing", 0.0, false),
        ("-", 0.0, false),
        ("+", 0.0, false),
        ("", 0.0, false),
        // crbug.org/588726
        ("-0.0010000000000000000000000000000000000000001e-256", -1.0000000000000001e-259, true),
    ];

    for (i, &(input, output, success)) in cases.iter().enumerate() {
        let context = format!("case {i} {input:?}");
        assert_parsed_double(string_to_double(input.as_bytes()), output, success, &context);
        assert_parsed_double(string_to_double(&wide(input.as_bytes())), output, success, &context);
    }

    // Embedded NUL: "3.14" followed by junk.
    let input_string: &[u8] = b"3.14\x00159";
    assert_parsed_double(string_to_double(input_string), 3.14, false, "embedded NUL");
}

#[test]
fn ported_double_to_string() {
    let cases: &[(f64, &str)] = &[
        (0.0, "0"),
        (0.5, "0.5"),
        (1.25, "1.25"),
        (1.33518e12, "1.33518e+12"),
        (1.33489e12, "1.33489e+12"),
        (1.33505e12, "1.33505e+12"),
        (1.33545e9, "1335450000"),
        (1.33503e9, "1335030000"),
    ];
    for &(input, expected) in cases {
        assert_eq!(number_to_string(input), expected);
        assert_eq!(number_to_string16(input), wide(expected.as_bytes()));
    }

    // The following two values were seen in crashes in the wild.
    let input = f64::from_le_bytes([0, 0, 0, 0, 0xee, 0x6d, 0x73, 0x42]);
    assert_eq!(number_to_string(input), "1.335179083776e+12");
    let input = f64::from_le_bytes([0, 0, 0, 0xa0, 0xda, 0x6c, 0x73, 0x42]);
    assert_eq!(number_to_string(input), "1.33489033216e+12");
}

#[test]
fn ported_double_to_string_fixed_precision() {
    let cases: &[(f64, usize, &str)] = &[
        (0.0, 0, "0"),
        (0.0, 3, "0.000"),
        (0.5, 3, "0.500"),
        (1.25, 3, "1.250"),
        (1.33518, 3, "1.335"),
        (1.33578, 3, "1.336"),
    ];
    for &(input, digits, expected) in cases {
        assert_eq!(number_to_string_with_fixed_precision(input, digits), expected);
        assert_eq!(
            number_to_string16_with_fixed_precision(input, digits),
            wide(expected.as_bytes())
        );
    }
}

#[test]
fn ported_append_hex_encoded_byte() {
    let mut hex = String::new();
    append_hex_encoded_byte(0, &mut hex, true);
    append_hex_encoded_byte(0, &mut hex, false);
    append_hex_encoded_byte(1, &mut hex, true);
    append_hex_encoded_byte(1, &mut hex, false);
    append_hex_encoded_byte(0xf, &mut hex, true);
    append_hex_encoded_byte(0xf, &mut hex, false);
    append_hex_encoded_byte(0x8a, &mut hex, true);
    append_hex_encoded_byte(0x8a, &mut hex, false);
    append_hex_encoded_byte(0xe0, &mut hex, true);
    append_hex_encoded_byte(0xe0, &mut hex, false);
    append_hex_encoded_byte(0xff, &mut hex, true);
    append_hex_encoded_byte(0xff, &mut hex, false);
    assert_eq!(hex, "000001010F0f8A8aE0e0FFff");
}

#[test]
fn ported_hex_encode() {
    assert_eq!(hex_encode(&[]), "");
    assert_eq!(hex_encode(b""), "");
    assert_eq!(hex_encode_lower(&[]), "");
    assert_eq!(hex_encode_lower(b""), "");

    let bytes: [u8; 7] = [0x01, 0xff, 0x02, 0xfe, 0x03, 0x80, 0x81];
    assert_eq!(hex_encode(&bytes), "01FF02FE038081");
    assert_eq!(hex_encode_lower(&bytes), "01ff02fe038081");

    let string: &[u8] = b"\x01\xff";
    assert_eq!(hex_encode(string), "01FF");
    assert_eq!(hex_encode_lower(string), "01ff");
}

/// Known-bad strtod conversions that motivated the use of dmg_fp.
#[test]
fn ported_strtod_failures() {
    let cases: &[(&str, u64)] = &[
        // exploringbinary.com: incorrectly-rounded conversions in Visual C++
        ("9214843084008499", 0x43405e6cec57761a),
        ("0.500000000000000166533453693773481063544750213623046875", 0x3fe0000000000002),
        ("30078505129381147446200", 0x44997a3c7271b021),
        ("1777820000000000000001", 0x4458180d5bad2e3e),
        (
            "0.500000000000000166547006220929549868969843373633921146392822265625",
            0x3fe0000000000002,
        ),
        ("0.50000000000000016656055874808561867439493653364479541778564453125", 0x3fe0000000000002),
        ("0.3932922657273", 0x3fd92bb352c4623a),
        // exploringbinary.com: incorrectly-rounded conversions in GCC and glibc
        ("0.500000000000000166533453693773481063544750213623046875", 0x3fe0000000000002),
        ("3.518437208883201171875e13", 0x42c0000000000002),
        ("62.5364939768271845828", 0x404f44abd5aa7ca4),
        ("8.10109172351e-10", 0x3e0bd5cbaef0fd0c),
        ("1.50000000000000011102230246251565404236316680908203125", 0x3ff8000000000000),
        ("9007199254740991.4999999999999999999999999999999995", 0x433fffffffffffff),
        // exploringbinary.com: incorrect decimal-to-floating-point in SQLite
        ("1e-23", 0x3b282db34012b251),
        ("8.533e+68", 0x4e3fa69165a8eea2),
        ("4.1006e-184", 0x19dbe0d1c7ea60c9),
        ("9.998e+307", 0x7fe1cc0a350ca87b),
        ("9.9538452227e-280", 0x0602117ae45cde43),
        ("6.47660115e-260", 0x0a1fdd9e333badad),
        ("7.4e+47", 0x49e033d7eca0adef),
        ("5.92e+48", 0x4a1033d7eca0adef),
        ("7.35e+66", 0x4dd172b70eababa9),
        ("8.32116e+55", 0x4b8b2628393e02cd),
    ];

    for &(input, expected) in cases {
        let (value, ok) = split_result(string_to_double(input.as_bytes()));
        assert!(ok, "input {input:?} should parse");
        assert_eq!(value.to_bits(), expected, "input {input:?}");
    }
}

// ---------------------------------------------------------------------------
// Extra tests: edge cases, differential and property checks
// ---------------------------------------------------------------------------

/// xorshift64, so the tests need no `rand` crate.
struct XorShift(u64);

impl XorShift {
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }

    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
}

/// Independent reference for the integer grammar. It scans a prefix of valid
/// digits. An invalid character ends the prefix and leaves a partial value.
/// Overflow saturates. Whitespace before the sign invalidates the value.
fn reference_int(input: &[u8], base: u32, signed: bool, min: i128, max: i128) -> (i128, bool) {
    let is_space = |b: u8| matches!(b, b'\t' | b'\n' | 0x0B | 0x0C | b'\r' | b' ');
    let leading = input.iter().take_while(|&&b| is_space(b)).count();
    let mut rest = &input[leading..];
    let mut negative = false;
    match rest.first() {
        Some(b'-') => {
            if !signed {
                return (0, false);
            }
            negative = true;
            rest = &rest[1..];
        }
        Some(b'+') => rest = &rest[1..],
        _ => {}
    }
    if base == 16 && rest.len() > 2 && rest[0] == b'0' && (rest[1] == b'x' || rest[1] == b'X') {
        rest = &rest[2..];
    }
    let mut value: i128 = 0;
    let mut valid_digits = 0;
    for &b in rest {
        let Some(d) = (b as char).to_digit(base) else {
            break;
        };
        value = value * i128::from(base) + i128::from(d);
        valid_digits += 1;
        let signed_value = if negative { -value } else { value };
        if signed_value < min || signed_value > max {
            return (if negative { min } else { max }, false);
        }
    }
    let signed_value = if negative { -value } else { value };
    let ok = leading == 0 && valid_digits > 0 && valid_digits == rest.len();
    (signed_value, ok)
}

#[test]
fn integer_parsers_match_reference_on_random_inputs() {
    const ALPHABET: &[u8] = b"0123456789abcdefABCDEFxX+- \t\n\x0Bg";
    let mut rng = XorShift(0x9e37_79b9_7f4a_7c15);
    for _ in 0..20_000 {
        let len = rng.below(21) as usize;
        let input: Vec<u8> =
            (0..len).map(|_| ALPHABET[rng.below(ALPHABET.len() as u64) as usize]).collect();
        let units = wide(&input);

        let (v, ok) = split_result(string_to_int(&input));
        let (rv, rok) = reference_int(&input, 10, true, i128::from(i32::MIN), i128::from(i32::MAX));
        assert_eq!((v as i128, ok), (rv, rok), "int {:?}", String::from_utf8_lossy(&input));
        assert_eq!(split_result(string_to_int(&units)), (v, ok));

        let (v, ok) = split_result(string_to_uint(&input));
        let (rv, rok) = reference_int(&input, 10, false, 0, i128::from(u32::MAX));
        assert_eq!((v as i128, ok), (rv, rok), "uint {:?}", String::from_utf8_lossy(&input));

        let (v, ok) = split_result(string_to_int64(&input));
        let (rv, rok) = reference_int(&input, 10, true, i128::from(i64::MIN), i128::from(i64::MAX));
        assert_eq!((v as i128, ok), (rv, rok), "int64 {:?}", String::from_utf8_lossy(&input));

        let (v, ok) = split_result(string_to_uint64(&input));
        let (rv, rok) = reference_int(&input, 10, false, 0, i128::from(u64::MAX));
        assert_eq!((v as i128, ok), (rv, rok), "uint64 {:?}", String::from_utf8_lossy(&input));

        let (v, ok) = split_result(hex_string_to_int(&input));
        let (rv, rok) = reference_int(&input, 16, true, i128::from(i32::MIN), i128::from(i32::MAX));
        assert_eq!((v as i128, ok), (rv, rok), "hex int {:?}", String::from_utf8_lossy(&input));

        let (v, ok) = split_result(hex_string_to_uint64(&input));
        let (rv, rok) = reference_int(&input, 16, false, 0, i128::from(u64::MAX));
        assert_eq!((v as i128, ok), (rv, rok), "hex uint64 {:?}", String::from_utf8_lossy(&input));
    }
}

#[test]
fn integer_saturation_at_boundaries() {
    assert_eq!(split_result(string_to_int(b"2147483647")), (i32::MAX, true));
    assert_eq!(split_result(string_to_int(b"2147483648")), (i32::MAX, false));
    assert_eq!(split_result(string_to_int(b"-2147483648")), (i32::MIN, true));
    assert_eq!(split_result(string_to_int(b"-2147483649")), (i32::MIN, false));
    assert_eq!(split_result(string_to_uint64(b"18446744073709551616")), (u64::MAX, false));
    // Overflow in the middle of a longer string still saturates, even when
    // more digits follow.
    assert_eq!(split_result(string_to_int(b"99999999999junk")), (i32::MAX, false));
    assert_eq!(split_result(string_to_int64(b"-99999999999999999999x")), (i64::MIN, false));
    // Huge input never panics.
    let long = "9".repeat(500);
    assert_eq!(split_result(string_to_uint64(long.as_bytes())), (u64::MAX, false));
}

#[test]
fn whitespace_rules() {
    // Leading whitespace keeps the value but makes it invalid.
    assert_eq!(split_result(string_to_int(b"\t7")), (7, false));
    // Whitespace between sign and digits is junk.
    assert_eq!(split_result(string_to_int(b"- 7")), (0, false));
    // Narrow bytes >= 0x80 are never whitespace.
    assert_eq!(split_result(string_to_double(b"\xA05")), (0.0, false));
    // Wide input: U+00A0 is rejected by IsWhitespace, but as leading whitespace
    // double_conversion skips it. The value is still returned with ok=false.
    let nbsp_units = [0x00A0u16, u16::from(b'5')];
    assert_eq!(split_result(string_to_double(&nbsp_units)), (5.0, false));
    // U+FEFF is skipped by double_conversion but is not in base::IsWhitespace, so
    // the parse succeeds. This matches C++.
    let bom_units = [0xFEFFu16, u16::from(b'5')];
    assert_eq!(split_result(string_to_double(&bom_units)), (5.0, true));
    // U+0085 is in base::IsWhitespace, but double_conversion does not skip it.
    let nel_units = [0x0085u16, u16::from(b'5')];
    assert_eq!(split_result(string_to_double(&nel_units)), (0.0, false));
}

#[test]
fn double_exponent_edge_cases() {
    assert_eq!(split_result(string_to_double(b"1e400")), (f64::INFINITY, false));
    assert_eq!(split_result(string_to_double(b"-1e400")), (f64::NEG_INFINITY, false));
    assert_eq!(split_result(string_to_double(b"1e-400")), (0.0, true));
    assert_eq!(split_result(string_to_double(b"0e5")), (0.0, true));
    assert_eq!(split_result(string_to_double(b"-0")).0.to_bits(), (-0.0f64).to_bits());
    assert_eq!(split_result(string_to_double(b"1e0000000000000000000000000001")), (10.0, true));
    assert_eq!(
        split_result(string_to_double(b"1e99999999999999999999999")),
        (f64::INFINITY, false)
    );
    assert_eq!(split_result(string_to_double(b".5")), (0.5, true));
    assert_eq!(split_result(string_to_double(b".")), (0.0, false));
    assert_eq!(split_result(string_to_double(b"-.")), (0.0, false));
    assert_eq!(split_result(string_to_double(b"e5")), (0.0, false));
    assert_eq!(split_result(string_to_double(b"inf")), (0.0, false));
    assert_eq!(split_result(string_to_double(b"0x10")), (0.0, false));
}

#[test]
fn double_matches_rust_parser_on_random_decimals() {
    let mut rng = XorShift(0x2545_f491_4f6c_dd1d);
    for _ in 0..5_000 {
        let mut text = String::new();
        match rng.below(3) {
            0 => text.push('-'),
            1 => text.push('+'),
            _ => {}
        }
        let int_len = 1 + rng.below(30) as usize;
        for _ in 0..int_len {
            text.push(char::from(b'0' + rng.below(10) as u8));
        }
        if rng.below(2) == 0 {
            text.push('.');
            for _ in 0..(1 + rng.below(30)) {
                text.push(char::from(b'0' + rng.below(10) as u8));
            }
        }
        if rng.below(2) == 0 {
            text.push('e');
            match rng.below(3) {
                0 => text.push('-'),
                1 => text.push('+'),
                _ => {}
            }
            let exp_len = 1 + rng.below(3) as usize;
            for _ in 0..exp_len {
                text.push(char::from(b'0' + rng.below(10) as u8));
            }
        }
        let expected: f64 = text.parse().expect("generated text is a valid float");
        let got = string_to_double(text.as_bytes());
        if expected.is_finite() {
            assert_eq!(got, Ok(expected), "input {text}");
        } else {
            assert_eq!(got, Err(expected), "input {text}");
        }
    }
}

#[test]
fn shortest_double_round_trips() {
    let mut rng = XorShift(0x0123_4567_89ab_cdef);
    for _ in 0..20_000 {
        let value = f64::from_bits(rng.next());
        if !value.is_finite() {
            assert_eq!(number_to_string(value), "");
            continue;
        }
        let text = number_to_string(value);
        let back = string_to_double(text.as_bytes()).expect("NumberToString output parses");
        assert_eq!(back.to_bits(), value.to_bits(), "text {text}");
    }
}

#[test]
fn shortest_double_layout() {
    assert_eq!(number_to_string(-0.0f64), "-0");
    assert_eq!(number_to_string(1.0f64), "1");
    assert_eq!(number_to_string(100.0f64), "100");
    assert_eq!(number_to_string(1e11f64), "100000000000");
    assert_eq!(number_to_string(1e12f64), "1e+12");
    assert_eq!(number_to_string(999_999_999_999.0f64), "999999999999");
    assert_eq!(number_to_string(1e-6f64), "0.000001");
    assert_eq!(number_to_string(1e-7f64), "1e-7");
    assert_eq!(number_to_string(-1.5e-9f64), "-1.5e-9");
    assert_eq!(number_to_string(5e-324f64), "5e-324");
    assert_eq!(number_to_string(f64::MAX), "1.7976931348623157e+308");
    assert_eq!(number_to_string(f64::INFINITY), "");
    assert_eq!(number_to_string(f64::NAN), "");
}

#[test]
fn fixed_precision_rounds_half_up_on_exact_value() {
    // Exact binary ties round up, unlike Rust's round-half-even.
    assert_eq!(number_to_string_with_fixed_precision(0.5, 0), "1");
    assert_eq!(number_to_string_with_fixed_precision(2.5, 0), "3");
    assert_eq!(number_to_string_with_fixed_precision(0.125, 2), "0.13");
    assert_eq!(number_to_string_with_fixed_precision(0.9, 0), "1");
    assert_eq!(number_to_string_with_fixed_precision(9.999, 2), "10.00");
    assert_eq!(number_to_string_with_fixed_precision(-0.0, 2), "-0.00");
    assert_eq!(number_to_string_with_fixed_precision(-0.001, 2), "-0.00");
    assert_eq!(number_to_string_with_fixed_precision(-1.5, 0), "-2");
}

#[test]
fn fixed_precision_rejects_like_to_fixed() {
    assert_eq!(number_to_string_with_fixed_precision(f64::NAN, 2), "");
    assert_eq!(number_to_string_with_fixed_precision(f64::INFINITY, 2), "");
    assert_eq!(number_to_string_with_fixed_precision(1e60, 2), "");
    assert_eq!(number_to_string_with_fixed_precision(-1e60, 2), "");
    assert_eq!(number_to_string_with_fixed_precision(1.0, 101), "");
    // Just below the limits still formats.
    assert_eq!(number_to_string_with_fixed_precision(9.99e59, 0).len(), 60);
    assert_eq!(number_to_string_with_fixed_precision(1.0, 100).len(), 102);
}

#[test]
fn fixed_precision_matches_rust_away_from_ties() {
    let mut rng = XorShift(0xdead_beef_cafe_f00d);
    for _ in 0..5_000 {
        let value = f64::from_bits(rng.next());
        if !value.is_finite() || value.abs() >= 1e60 {
            continue;
        }
        let digits = rng.below(12) as usize;
        // Skip exact ties, where Rust rounds half to even and C++ rounds up.
        let exact = format!("{:.1074}", value.abs());
        let frac = exact.split_once('.').expect("has point").1.as_bytes();
        let is_tie = frac[digits] == b'5' && frac[digits + 1..].iter().all(|&b| b == b'0');
        if is_tie {
            continue;
        }
        assert_eq!(
            number_to_string_with_fixed_precision(value, digits),
            format!("{:.*}", digits, value),
            "value {value:?} digits {digits}"
        );
    }
}

#[test]
fn hex_round_trip_and_rejections() {
    let mut rng = XorShift(0x5151_5151_5151_5151);
    for _ in 0..2_000 {
        let len = rng.below(40) as usize;
        let bytes: Vec<u8> = (0..len).map(|_| rng.next() as u8).collect();
        let upper = hex_encode(&bytes);
        let lower = hex_encode_lower(&bytes);
        assert_eq!(upper.to_ascii_lowercase(), lower);
        if bytes.is_empty() {
            assert!(hex_string_to_bytes(b"").is_err());
            continue;
        }
        assert_eq!(hex_string_to_bytes(upper.as_bytes()).unwrap(), bytes);
        assert_eq!(hex_string_to_bytes(lower.as_bytes()).unwrap(), bytes);
        let mut span = vec![0u8; bytes.len()];
        hex_string_to_span(upper.as_bytes(), &mut span).unwrap();
        assert_eq!(span, bytes);
    }
}

#[test]
fn hex_decode_error_carries_partial_bytes() {
    let err = hex_string_to_bytes(b"0102zz").unwrap_err();
    assert_eq!(err.partial, vec![0x01, 0x02]);
    assert!(hex_string_to_bytes(b"abc").unwrap_err().partial.is_empty());
    let mut out = [0xAAu8; 3];
    assert!(hex_string_to_span(b"0102zz", &mut out).is_err());
    assert_eq!(out, [0x01, 0x02, 0xAA]);
    assert_eq!(err.to_string(), "invalid hex input");
}

#[test]
fn utf16_inputs_parse_like_narrow() {
    let text = "-12345";
    assert_eq!(
        split_result(string_to_int(&wide(text.as_bytes()))),
        split_result(string_to_int(text.as_bytes()))
    );
    assert_eq!(split_result(hex_string_to_uint(b"ff")), (255, true));
    assert_eq!(string_to_uint64(&[u16::from(b'1'), 0xFF0D]), Err(1));
}
