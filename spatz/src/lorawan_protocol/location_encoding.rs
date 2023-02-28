//! Latitude, longitude and altitude are encoded into 3 byte signed values.
//!
//! Latitude encoding:
//! LAT: encoded value
//! Latitude: floating point value
//! LAT = (Latitude / (90/2²³) ).round
//! Latitude = LAT * (90/2²³)
//!
//! Longitude encoding:
//! LONG: encoded value
//! Longitude: floating point value
//! LONG = (Longitude / (180 / 2²³)).round
//! Longitude = LONG * (180 / 2²³)
//!
//! Altitude encoding:
//! ALT: encoded value
//! Altitude: floating point value
//! ALT = (Altitude * 100).round
//! Altitude = ALT / 100

use crate::error::LocationEncodingError;
use tracing::trace;

/// Used to encode the latitude into a 3 byte value: 90° divided by 2²³
const LAT_ENCODING_VALUE: f64 = 90_f64 / 2_i32.pow(23) as f64;
/// Used to encode the longitude into a 3 byte value: 180° divided by 2²³
const LONG_ENCODING_VALUE: f64 = 180_f64 / 2_i32.pow(23) as f64;
/// Used to encode the altitude into a 3 byte value
const ALT_ENCODING_VALUE: f64 = 100_f64;

/// Encode a floating point latitude into a 3 byte singed value.
pub fn encode_lat(lat: f64) -> Result<i32, LocationEncodingError> {
    trace!("Encoding latitude from: {lat}");
    if lat.abs() > 90_f64 || lat == -90_f64 {
        Err(LocationEncodingError::LatOutOfRange)
    } else {
        Ok((lat / LAT_ENCODING_VALUE).round() as i32)
    }
}

/// Decode a singed 3 byte encoded latitude into a floating point value.
pub fn decode_lat(lat: i32) -> f64 {
    trace!("Decoding latitude from: {lat}");
    ((LAT_ENCODING_VALUE * f64::from(lat)) * 100000_f64).round() / 100000_f64
}

/// Encode a floating point longitude into a 3 byte singed value.
pub fn encode_long(long: f64) -> Result<i32, LocationEncodingError> {
    trace!("Encoding longitude from: {long}");
    if long.abs() > 180_f64 {
        Err(LocationEncodingError::LongOutOfRange)
    } else {
        Ok((long / LONG_ENCODING_VALUE).round() as i32)
    }
}

/// Decode a singed 3 byte encoded longitude into a floating point value.
pub fn decode_long(long: i32) -> f64 {
    trace!("Decoding longitude from: {long}");
    ((LONG_ENCODING_VALUE * f64::from(long)) * 100000_f64).round() / 100000_f64
}

/// Limit altitude to a max of 41943.00 as 24 bit 2 complement can only hold values between
/// 8388607 and -8388607. Precision two decimal values (e.g. 4022.53).
pub fn encode_alt(alt: f64) -> Result<i32, LocationEncodingError> {
    trace!("Encoding altitude from: {alt}");
    if alt.abs() > 83886_f64 {
        Err(LocationEncodingError::AltOutOfRange)
    } else {
        Ok((alt * ALT_ENCODING_VALUE).round() as i32)
    }
}

/// Decode a singed 3 byte encoded altitude into a floating point value.
pub fn decode_alt(alt: i32) -> f64 {
    trace!("Decoding altitude from: {alt}");
    f64::from(alt) / ALT_ENCODING_VALUE
}

#[allow(clippy::unwrap_used)]
#[cfg(test)]
mod tests {
    use crate::error::LocationEncodingError;
    use crate::lorawan_protocol::location_encoding::{
        decode_alt, decode_lat, decode_long, encode_alt, encode_lat, encode_long,
    };

    #[test]
    fn encode_decode_lat_test() {
        let lat = 23.02;
        let encoded_lat = encode_lat(lat).unwrap();
        let decoded_lat = decode_lat(encoded_lat);
        assert!((lat - decoded_lat).abs() < 0.00001);

        let lat = -58.0124552;
        let encoded_lat = encode_lat(lat).unwrap();
        let decoded_lat = decode_lat(encoded_lat);
        assert!((lat - decoded_lat).abs() < 0.00001);
    }
    #[test]
    fn encode_lat_out_of_range_test() {
        assert_eq!(
            Err(LocationEncodingError::LatOutOfRange),
            encode_lat(91_f64)
        );
        assert_eq!(
            Err(LocationEncodingError::LatOutOfRange),
            encode_lat(-11291_f64)
        );
    }

    #[test]
    fn decode_lat_test() {
        let encoded_lat = 2_i32.pow(23);
        let decoded_lat = decode_lat(encoded_lat);
        assert_eq!(90_f64, decoded_lat);
        let encoded_lat = -(2_i32.pow(23));
        let decoded_lat = decode_lat(encoded_lat);
        assert_eq!(-90_f64, decoded_lat);
    }

    #[test]
    fn encode_decode_long_test() {
        let long = 120.02;
        let encoded_long = encode_long(long).unwrap();
        let decoded_long = decode_long(encoded_long);
        assert!((long - decoded_long).abs() < 0.00001);

        let long = -150.0124552;
        let encoded_long = encode_long(long).unwrap();
        let decoded_long = decode_long(encoded_long);
        assert!((long - decoded_long).abs() < 0.00001);
    }

    #[test]
    fn encode_long_out_of_range_test() {
        assert_eq!(
            Err(LocationEncodingError::LongOutOfRange),
            encode_long(191_f64)
        );
        assert_eq!(
            Err(LocationEncodingError::LongOutOfRange),
            encode_long(-11291_f64)
        );
    }

    #[test]
    fn decode_long_test() {
        let encoded_long = 2_i32.pow(23);
        let decoded_long = decode_long(encoded_long);
        assert_eq!(180_f64, decoded_long);
        let encoded_long = -(2_i32.pow(23));
        let decoded_long = decode_long(encoded_long);
        assert_eq!(-180_f64, decoded_long);
    }

    #[test]
    fn encode_decode_alt_test() {
        let alt = 1200.02;
        let encoded_alt = encode_alt(alt).unwrap();
        let decoded_alt = decode_alt(encoded_alt);
        assert!((alt - decoded_alt).abs() < 0.01);

        let alt = -150.0124552;
        let encoded_alt = encode_alt(alt).unwrap();
        let decoded_alt = decode_alt(encoded_alt);
        assert!((alt - decoded_alt).abs() < 0.01);
    }

    #[test]
    fn encode_alt_out_of_range_test() {
        assert_eq!(
            Err(LocationEncodingError::AltOutOfRange),
            encode_alt(83887_f64)
        );
        assert_eq!(
            Err(LocationEncodingError::AltOutOfRange),
            encode_alt(-11183887_f64)
        );
    }
}
