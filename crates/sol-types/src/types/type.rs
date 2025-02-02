use crate::{token::TokenSeq, Result, TokenType, Word};
use alloc::{borrow::Cow, string::String, vec::Vec};

/// An encodable is any type that may be encoded via a given `SolType`.
pub trait Encodable<T: ?Sized + SolType> {
    /// Convert the value to tokens.
    fn to_tokens(&self) -> T::TokenType<'_>;
}

/// A Solidity Type, for ABI encoding and decoding
///
/// This trait is implemented by types that contain ABI encoding and decoding
/// info for Solidity types. Types may be combined to express arbitrarily
/// complex Solidity types.
///
/// ```
/// use alloy_sol_types::{SolType, sol_data::*};
///
/// type DynUint256Array = Array<Uint<256>>;
/// assert_eq!(&DynUint256Array::sol_type_name(), "uint256[]");
///
/// type Erc20FunctionArgs = (Address, Uint<256>);
/// assert_eq!(&Erc20FunctionArgs::sol_type_name(), "(address,uint256)");
///
/// type LargeComplexType = (FixedArray<Array<Bool>, 2>, (FixedBytes<13>, String));
/// assert_eq!(&LargeComplexType::sol_type_name(), "(bool[][2],(bytes13,string))");
/// ```
///
/// These types are zero cost representations of Solidity types. They do not
/// exist at runtime. They ONLY contain information about the type, they do not
/// carry any data.
///
/// ### Implementer's Guide
///
/// We do not recommend implementing this trait directly. Instead, we recommend
/// using the [`crate::sol`] proc macro to parse a Solidity structdef into a
/// native Rust struct.
///
/// ```
/// alloy_sol_types::sol! {
///     struct MyStruct {
///         bool a;
///         bytes2 b;
///     }
/// }
///
/// // This is the native rust representation of a Solidity type!
/// // How cool is that!
/// const MY_STRUCT: MyStruct = MyStruct { a: true, b: [0x01, 0x02] };
/// ```
pub trait SolType {
    /// The corresponding Rust type.
    type RustType: Encodable<Self> + 'static;

    /// The corresponding ABI token type.
    ///
    /// See implementers of [`TokenType`].
    type TokenType<'a>: TokenType<'a>;

    /// The encoded size of the type, if known at compile time
    const ENCODED_SIZE: Option<usize> = Some(32);

    /// Whether the encoded size is dynamic.
    const DYNAMIC: bool = Self::ENCODED_SIZE.is_none();

    /// The name of the type in Solidity.
    fn sol_type_name() -> Cow<'static, str>;

    /// Calculate the encoded size of the data, counting both head and tail
    /// words. For a single-word type this will always be 32.
    #[inline]
    fn encoded_size(_rust: &Self::RustType) -> usize {
        Self::ENCODED_SIZE.unwrap()
    }

    /// Check a token to see if it can be detokenized with this type.
    fn type_check(token: &Self::TokenType<'_>) -> Result<()>;

    #[doc(hidden)]
    fn type_check_fail(data: &[u8]) -> crate::Error {
        crate::Error::type_check_fail(data, Self::sol_type_name())
    }

    /// Detokenize.
    fn detokenize(token: Self::TokenType<'_>) -> Self::RustType;

    /// Tokenize.
    fn tokenize<E: Encodable<Self>>(rust: &E) -> Self::TokenType<'_> {
        rust.to_tokens()
    }

    /// The encoded struct type (as EIP-712), if any. None for non-structs.
    #[inline]
    fn eip712_encode_type() -> Option<Cow<'static, str>> {
        None
    }

    /// Encode this data according to EIP-712 `encodeData` rules, and hash it
    /// if necessary.
    ///
    /// Implementer's note: All single-word types are encoded as their word.
    /// All multi-word types are encoded as the hash the concatenated data
    /// words for each element
    ///
    /// <https://eips.ethereum.org/EIPS/eip-712#definition-of-encodedata>
    fn eip712_data_word(rust: &Self::RustType) -> Word;

    /// Non-standard Packed Mode ABI encoding.
    ///
    /// See [`encode_packed`][SolType::encode_packed] for more details.
    fn encode_packed_to(rust: &Self::RustType, out: &mut Vec<u8>);

    /// Non-standard Packed Mode ABI encoding.
    ///
    /// This is different from normal ABI encoding:
    /// - types shorter than 32 bytes are concatenated directly, without padding
    ///   or sign extension;
    /// - dynamic types are encoded in-place and without the length;
    /// - array elements are padded, but still encoded in-place.
    ///
    /// More information can be found in the [Solidity docs](https://docs.soliditylang.org/en/latest/abi-spec.html#non-standard-packed-mode).
    fn encode_packed(rust: &Self::RustType) -> Vec<u8> {
        let mut out = Vec::new();
        Self::encode_packed_to(rust, &mut out);
        out
    }

    /* BOILERPLATE BELOW */

    /// Encode a single ABI token by wrapping it in a 1-length sequence.
    fn encode_single(rust: &Self::RustType) -> Vec<u8> {
        crate::encode_single(&rust.to_tokens())
    }

    /// Encode an ABI sequence.
    fn encode<'a>(rust: &'a Self::RustType) -> Vec<u8>
    where
        Self::TokenType<'a>: TokenSeq<'a>,
    {
        crate::encode(&rust.to_tokens())
    }

    /// Encode an ABI sequence suitable for function parameters.
    fn encode_params<'a>(rust: &'a Self::RustType) -> Vec<u8>
    where
        Self::TokenType<'a>: TokenSeq<'a>,
    {
        crate::encode_params(&rust.to_tokens())
    }

    /// Hex output of [`encode`][SolType::encode].
    fn hex_encode<'a>(rust: &'a Self::RustType) -> String
    where
        Self::TokenType<'a>: TokenSeq<'a>,
    {
        hex::encode_prefixed(Self::encode(rust))
    }

    /// Hex output of [`encode_single`][SolType::encode_single].
    fn hex_encode_single(rust: &Self::RustType) -> String {
        hex::encode_prefixed(Self::encode_single(rust))
    }

    /// Hex output of [`encode_params`][SolType::encode_params].
    fn hex_encode_params<'a>(rust: &'a Self::RustType) -> String
    where
        Self::TokenType<'a>: TokenSeq<'a>,
    {
        hex::encode_prefixed(Self::encode_params(rust))
    }

    /// Decode a Rust type from an ABI blob.
    #[inline]
    fn decode<'de>(data: &'de [u8], validate: bool) -> Result<Self::RustType>
    where
        Self::TokenType<'de>: TokenSeq<'de>,
    {
        let decoded = crate::decode::<Self::TokenType<'_>>(data, validate)?;
        if validate {
            Self::type_check(&decoded)?;
        }
        Ok(Self::detokenize(decoded))
    }

    /// Decode a Rust type from an ABI blob.
    #[inline]
    fn decode_params(data: &[u8], validate: bool) -> Result<Self::RustType>
    where
        for<'de> Self::TokenType<'de>: TokenSeq<'de>,
    {
        let decoded = crate::decode_params::<Self::TokenType<'_>>(data, validate)?;
        if validate {
            Self::type_check(&decoded)?;
        }
        Ok(Self::detokenize(decoded))
    }

    /// Decode a Rust type from an ABI blob.
    #[inline]
    fn decode_single(data: &[u8], validate: bool) -> Result<Self::RustType> {
        let decoded = crate::decode_single::<Self::TokenType<'_>>(data, validate)?;
        if validate {
            Self::type_check(&decoded)?;
        }
        Ok(Self::detokenize(decoded))
    }

    /// Decode a Rust type from a hex-encoded ABI blob.
    #[inline]
    fn hex_decode(data: &str, validate: bool) -> Result<Self::RustType>
    where
        for<'de> Self::TokenType<'de>: TokenSeq<'de>,
    {
        hex::decode(data)
            .map_err(Into::into)
            .and_then(|buf| Self::decode(&buf, validate))
    }

    /// Decode a Rust type from a hex-encoded ABI blob.
    #[inline]
    fn hex_decode_single(data: &str, validate: bool) -> Result<Self::RustType> {
        hex::decode(data)
            .map_err(Into::into)
            .and_then(|buf| Self::decode_single(&buf, validate))
    }

    /// Decode a Rust type from a hex-encoded ABI blob.
    #[inline]
    fn hex_decode_params(data: &str, validate: bool) -> Result<Self::RustType>
    where
        for<'de> Self::TokenType<'de>: TokenSeq<'de>,
    {
        hex::decode(data)
            .map_err(Into::into)
            .and_then(|buf| Self::decode_params(&buf, validate))
    }
}
