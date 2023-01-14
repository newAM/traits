//! Elliptic curve public keys.

use crate::{
    point::NonIdentity, AffinePoint, Curve, Error, NonZeroScalar, ProjectiveArithmetic,
    ProjectivePoint, Result,
};
use core::fmt::Debug;
use group::{Curve as _, Group};

#[cfg(feature = "jwk")]
use crate::{JwkEcKey, JwkParameters};

#[cfg(all(feature = "sec1", feature = "pkcs8"))]
use crate::{
    pkcs8::{self, AssociatedOid, DecodePublicKey},
    ALGORITHM_OID,
};

#[cfg(feature = "pem")]
use core::str::FromStr;

#[cfg(feature = "sec1")]
use {
    crate::{
        sec1::{EncodedPoint, FromEncodedPoint, ModulusSize, ToEncodedPoint},
        FieldSize, PointCompression,
    },
    core::cmp::Ordering,
    subtle::CtOption,
};

#[cfg(feature = "serde")]
use serdect::serde::{de, ser, Deserialize, Serialize};

#[cfg(all(feature = "alloc", feature = "pkcs8"))]
use pkcs8::EncodePublicKey;

#[cfg(any(feature = "jwk", feature = "pem"))]
use alloc::string::{String, ToString};

#[cfg(feature = "alloc")]
use alloc::boxed::Box;

/// Elliptic curve public keys.
///
/// This is a wrapper type for [`AffinePoint`] which ensures an inner
/// non-identity point and provides a common place to handle encoding/decoding.
///
/// # Parsing "SPKI" Keys
///
/// X.509 `SubjectPublicKeyInfo` (SPKI) is a commonly used format for encoding
/// public keys, notably public keys corresponding to PKCS#8 private keys.
/// (especially ones generated by OpenSSL).
///
/// Keys in SPKI format are either binary (ASN.1 BER/DER), or PEM encoded
/// (ASCII) and begin with the following:
///
/// ```text
/// -----BEGIN PUBLIC KEY-----
/// ```
///
/// To decode an elliptic curve public key from SPKI, enable the `pkcs8`
/// feature of this crate (or the `pkcs8` feature of a specific RustCrypto
/// elliptic curve crate) and use the
/// [`elliptic_curve::pkcs8::DecodePublicKey`][`pkcs8::DecodePublicKey`]
/// trait to parse it.
///
/// When the `pem` feature of this crate (or a specific RustCrypto elliptic
/// curve crate) is enabled, a [`FromStr`] impl is also available.
///
/// # `serde` support
///
/// When the optional `serde` feature of this create is enabled, [`Serialize`]
/// and [`Deserialize`] impls are provided for this type.
///
/// The serialization is binary-oriented and supports ASN.1 DER
/// Subject Public Key Info (SPKI) as the encoding format.
///
/// For a more text-friendly encoding of public keys, use [`JwkEcKey`] instead.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PublicKey<C>
where
    C: Curve + ProjectiveArithmetic,
{
    point: AffinePoint<C>,
}

impl<C> PublicKey<C>
where
    C: Curve + ProjectiveArithmetic,
{
    /// Convert an [`AffinePoint`] into a [`PublicKey`]
    pub fn from_affine(point: AffinePoint<C>) -> Result<Self> {
        if ProjectivePoint::<C>::from(point).is_identity().into() {
            Err(Error)
        } else {
            Ok(Self { point })
        }
    }

    /// Compute a [`PublicKey`] from a secret [`NonZeroScalar`] value
    /// (i.e. a secret key represented as a raw scalar value)
    pub fn from_secret_scalar(scalar: &NonZeroScalar<C>) -> Self {
        // `NonZeroScalar` ensures the resulting point is not the identity
        Self {
            point: (C::ProjectivePoint::generator() * scalar.as_ref()).to_affine(),
        }
    }

    /// Decode [`PublicKey`] (compressed or uncompressed) from the
    /// `Elliptic-Curve-Point-to-Octet-String` encoding described in
    /// SEC 1: Elliptic Curve Cryptography (Version 2.0) section
    /// 2.3.3 (page 10).
    ///
    /// <http://www.secg.org/sec1-v2.pdf>
    #[cfg(feature = "sec1")]
    pub fn from_sec1_bytes(bytes: &[u8]) -> Result<Self>
    where
        C: Curve,
        FieldSize<C>: ModulusSize,
        AffinePoint<C>: FromEncodedPoint<C> + ToEncodedPoint<C>,
    {
        let point = EncodedPoint::<C>::from_bytes(bytes).map_err(|_| Error)?;
        Option::from(Self::from_encoded_point(&point)).ok_or(Error)
    }

    /// Convert this [`PublicKey`] into the
    /// `Elliptic-Curve-Point-to-Octet-String` encoding described in
    /// SEC 1: Elliptic Curve Cryptography (Version 2.0) section 2.3.3
    /// (page 10).
    ///
    /// <http://www.secg.org/sec1-v2.pdf>
    #[cfg(feature = "alloc")]
    pub fn to_sec1_bytes(&self) -> Box<[u8]>
    where
        C: Curve + ProjectiveArithmetic + PointCompression,
        AffinePoint<C>: FromEncodedPoint<C> + ToEncodedPoint<C>,
        FieldSize<C>: ModulusSize,
    {
        let point = EncodedPoint::<C>::from(self);
        point.to_bytes()
    }

    /// Borrow the inner [`AffinePoint`] from this [`PublicKey`].
    ///
    /// In ECC, public keys are elliptic curve points.
    pub fn as_affine(&self) -> &AffinePoint<C> {
        &self.point
    }

    /// Convert this [`PublicKey`] to a [`ProjectivePoint`] for the given curve
    pub fn to_projective(&self) -> ProjectivePoint<C> {
        self.point.into()
    }

    /// Parse a [`JwkEcKey`] JSON Web Key (JWK) into a [`PublicKey`].
    #[cfg(feature = "jwk")]
    pub fn from_jwk(jwk: &JwkEcKey) -> Result<Self>
    where
        C: Curve + JwkParameters,
        AffinePoint<C>: FromEncodedPoint<C> + ToEncodedPoint<C>,
        FieldSize<C>: ModulusSize,
    {
        jwk.to_public_key::<C>()
    }

    /// Parse a string containing a JSON Web Key (JWK) into a [`PublicKey`].
    #[cfg(feature = "jwk")]
    pub fn from_jwk_str(jwk: &str) -> Result<Self>
    where
        C: Curve + JwkParameters,
        AffinePoint<C>: FromEncodedPoint<C> + ToEncodedPoint<C>,
        FieldSize<C>: ModulusSize,
    {
        jwk.parse::<JwkEcKey>().and_then(|jwk| Self::from_jwk(&jwk))
    }

    /// Serialize this public key as [`JwkEcKey`] JSON Web Key (JWK).
    #[cfg(feature = "jwk")]
    pub fn to_jwk(&self) -> JwkEcKey
    where
        C: Curve + JwkParameters,
        AffinePoint<C>: FromEncodedPoint<C> + ToEncodedPoint<C>,
        FieldSize<C>: ModulusSize,
    {
        self.into()
    }

    /// Serialize this public key as JSON Web Key (JWK) string.
    #[cfg(feature = "jwk")]
    pub fn to_jwk_string(&self) -> String
    where
        C: Curve + JwkParameters,
        AffinePoint<C>: FromEncodedPoint<C> + ToEncodedPoint<C>,
        FieldSize<C>: ModulusSize,
    {
        self.to_jwk().to_string()
    }
}

impl<C> AsRef<AffinePoint<C>> for PublicKey<C>
where
    C: Curve + ProjectiveArithmetic,
{
    fn as_ref(&self) -> &AffinePoint<C> {
        self.as_affine()
    }
}

impl<C> Copy for PublicKey<C> where C: Curve + ProjectiveArithmetic {}

#[cfg(feature = "sec1")]
impl<C> FromEncodedPoint<C> for PublicKey<C>
where
    C: Curve + ProjectiveArithmetic,
    AffinePoint<C>: FromEncodedPoint<C> + ToEncodedPoint<C>,
    FieldSize<C>: ModulusSize,
{
    /// Initialize [`PublicKey`] from an [`EncodedPoint`]
    fn from_encoded_point(encoded_point: &EncodedPoint<C>) -> CtOption<Self> {
        AffinePoint::<C>::from_encoded_point(encoded_point).and_then(|point| {
            let is_identity = ProjectivePoint::<C>::from(point).is_identity();
            CtOption::new(PublicKey { point }, !is_identity)
        })
    }
}

#[cfg(feature = "sec1")]
impl<C> ToEncodedPoint<C> for PublicKey<C>
where
    C: Curve + ProjectiveArithmetic,
    AffinePoint<C>: FromEncodedPoint<C> + ToEncodedPoint<C>,
    FieldSize<C>: ModulusSize,
{
    /// Serialize this [`PublicKey`] as a SEC1 [`EncodedPoint`], optionally applying
    /// point compression
    fn to_encoded_point(&self, compress: bool) -> EncodedPoint<C> {
        self.point.to_encoded_point(compress)
    }
}

#[cfg(feature = "sec1")]
impl<C> From<PublicKey<C>> for EncodedPoint<C>
where
    C: Curve + ProjectiveArithmetic + PointCompression,
    AffinePoint<C>: FromEncodedPoint<C> + ToEncodedPoint<C>,
    FieldSize<C>: ModulusSize,
{
    fn from(public_key: PublicKey<C>) -> EncodedPoint<C> {
        EncodedPoint::<C>::from(&public_key)
    }
}

#[cfg(feature = "sec1")]
impl<C> From<&PublicKey<C>> for EncodedPoint<C>
where
    C: Curve + ProjectiveArithmetic + PointCompression,
    AffinePoint<C>: FromEncodedPoint<C> + ToEncodedPoint<C>,
    FieldSize<C>: ModulusSize,
{
    fn from(public_key: &PublicKey<C>) -> EncodedPoint<C> {
        public_key.to_encoded_point(C::COMPRESS_POINTS)
    }
}

impl<C, P> From<NonIdentity<P>> for PublicKey<C>
where
    C: Curve + ProjectiveArithmetic,
    P: Copy + Into<AffinePoint<C>>,
{
    fn from(value: NonIdentity<P>) -> Self {
        PublicKey::from(&value)
    }
}

impl<C, P> From<&NonIdentity<P>> for PublicKey<C>
where
    C: Curve + ProjectiveArithmetic,
    P: Copy + Into<AffinePoint<C>>,
{
    fn from(value: &NonIdentity<P>) -> Self {
        Self {
            point: value.to_point().into(),
        }
    }
}

#[cfg(feature = "sec1")]
impl<C> PartialOrd for PublicKey<C>
where
    C: Curve + ProjectiveArithmetic,
    AffinePoint<C>: FromEncodedPoint<C> + ToEncodedPoint<C>,
    FieldSize<C>: ModulusSize,
{
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[cfg(feature = "sec1")]
impl<C> Ord for PublicKey<C>
where
    C: Curve + ProjectiveArithmetic,
    AffinePoint<C>: FromEncodedPoint<C> + ToEncodedPoint<C>,
    FieldSize<C>: ModulusSize,
{
    fn cmp(&self, other: &Self) -> Ordering {
        // TODO(tarcieri): more efficient implementation?
        // This is implemented this way to reduce bounds for `AffinePoint<C>`
        self.to_encoded_point(false)
            .cmp(&other.to_encoded_point(false))
    }
}

#[cfg(all(feature = "pkcs8", feature = "sec1"))]
impl<C> TryFrom<pkcs8::SubjectPublicKeyInfo<'_>> for PublicKey<C>
where
    C: Curve + AssociatedOid + ProjectiveArithmetic,
    AffinePoint<C>: FromEncodedPoint<C> + ToEncodedPoint<C>,
    FieldSize<C>: ModulusSize,
{
    type Error = pkcs8::spki::Error;

    fn try_from(spki: pkcs8::SubjectPublicKeyInfo<'_>) -> pkcs8::spki::Result<Self> {
        spki.algorithm.assert_oids(ALGORITHM_OID, C::OID)?;
        Self::from_sec1_bytes(spki.subject_public_key)
            .map_err(|_| der::Tag::BitString.value_error().into())
    }
}

#[cfg(all(feature = "pkcs8", feature = "sec1"))]
impl<C> DecodePublicKey for PublicKey<C>
where
    C: Curve + AssociatedOid + ProjectiveArithmetic,
    AffinePoint<C>: FromEncodedPoint<C> + ToEncodedPoint<C>,
    FieldSize<C>: ModulusSize,
{
}

#[cfg(all(feature = "alloc", feature = "pkcs8"))]
impl<C> EncodePublicKey for PublicKey<C>
where
    C: Curve + AssociatedOid + ProjectiveArithmetic,
    AffinePoint<C>: FromEncodedPoint<C> + ToEncodedPoint<C>,
    FieldSize<C>: ModulusSize,
{
    fn to_public_key_der(&self) -> pkcs8::spki::Result<der::Document> {
        let algorithm = pkcs8::AlgorithmIdentifier {
            oid: ALGORITHM_OID,
            parameters: Some((&C::OID).into()),
        };

        let public_key_bytes = self.to_encoded_point(false);

        pkcs8::SubjectPublicKeyInfo {
            algorithm,
            subject_public_key: public_key_bytes.as_ref(),
        }
        .try_into()
    }
}

#[cfg(feature = "pem")]
impl<C> FromStr for PublicKey<C>
where
    C: Curve + AssociatedOid + ProjectiveArithmetic,
    AffinePoint<C>: FromEncodedPoint<C> + ToEncodedPoint<C>,
    FieldSize<C>: ModulusSize,
{
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        Self::from_public_key_pem(s).map_err(|_| Error)
    }
}

#[cfg(feature = "pem")]
impl<C> ToString for PublicKey<C>
where
    C: Curve + AssociatedOid + ProjectiveArithmetic,
    AffinePoint<C>: FromEncodedPoint<C> + ToEncodedPoint<C>,
    FieldSize<C>: ModulusSize,
{
    fn to_string(&self) -> String {
        self.to_public_key_pem(Default::default())
            .expect("PEM encoding error")
    }
}

#[cfg(feature = "serde")]
impl<C> Serialize for PublicKey<C>
where
    C: Curve + AssociatedOid + ProjectiveArithmetic,
    AffinePoint<C>: FromEncodedPoint<C> + ToEncodedPoint<C>,
    FieldSize<C>: ModulusSize,
{
    fn serialize<S>(&self, serializer: S) -> core::result::Result<S::Ok, S::Error>
    where
        S: ser::Serializer,
    {
        let der = self.to_public_key_der().map_err(ser::Error::custom)?;
        serdect::slice::serialize_hex_upper_or_bin(&der, serializer)
    }
}

#[cfg(feature = "serde")]
impl<'de, C> Deserialize<'de> for PublicKey<C>
where
    C: Curve + AssociatedOid + ProjectiveArithmetic,
    AffinePoint<C>: FromEncodedPoint<C> + ToEncodedPoint<C>,
    FieldSize<C>: ModulusSize,
{
    fn deserialize<D>(deserializer: D) -> core::result::Result<Self, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        let der_bytes = serdect::slice::deserialize_hex_or_bin_vec(deserializer)?;
        Self::from_public_key_der(&der_bytes).map_err(de::Error::custom)
    }
}

#[cfg(all(feature = "dev", test))]
mod tests {
    use crate::{dev::MockCurve, sec1::FromEncodedPoint};

    type EncodedPoint = crate::sec1::EncodedPoint<MockCurve>;
    type PublicKey = super::PublicKey<MockCurve>;

    #[test]
    fn from_encoded_point_rejects_identity() {
        let identity = EncodedPoint::identity();
        assert!(bool::from(
            PublicKey::from_encoded_point(&identity).is_none()
        ));
    }
}
