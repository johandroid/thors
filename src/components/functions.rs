/// Client-side BOLT #11 Lightning invoice decoder.
///
/// Decodes the human-readable part (amount) and tagged fields (description,
/// expiry) from a bech32-encoded invoice without requiring an LND round-trip.

#[derive(Debug, Clone)]
pub(crate) struct DecodedInvoice {
    pub(crate) amount_msats: Option<u64>,
    pub(crate) description: Option<String>,
    pub(crate) expiry_seconds: u64,
}

#[derive(Debug, Clone, thiserror::Error)]
pub(crate) enum DecodeError {
    #[error("empty invoice")]
    Empty,
    #[error("invalid bech32 encoding")]
    InvalidBech32,
    #[error("invalid invoice encoding (expected bech32, not bech32m)")]
    InvalidVariant,
    #[error("invoice data too short")]
    DataTooShort,
    #[error("invalid invoice prefix (expected 'ln')")]
    InvalidPrefix,
    #[error("invalid amount in invoice")]
    InvalidAmount,
    #[error("amount exceeds maximum representable value")]
    AmountOverflow,
    #[error("sub-millisatoshi precision not supported")]
    SubMillisatoshi,
    #[error("tagged field extends beyond invoice data")]
    TagLengthOverflow,
    #[error("tagged field missing length bytes")]
    TagLengthMissing,
    #[error("invalid 5-to-8 bit padding")]
    InvalidPadding,
    #[error("description contains invalid UTF-8")]
    InvalidDescription,
}

/// Bech32 character set used by BOLT #11 to identify tagged field types.
const BECH32_CHARSET: &[u8; 32] = b"qpzry9x8gf2tvdw0s3jn54khce6mua7l";

/// Minimum data length: 7 (timestamp) + 104 (signature + recovery flag).
const MIN_DATA_LEN: usize = 7 + 104;

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Decode a BOLT #11 payment request into its core fields.
pub(crate) fn decode_payment_request_local(input: &str) -> Result<DecodedInvoice, DecodeError> {
    let cleaned = sanitize_payment_request(input);
    if cleaned.is_empty() {
        return Err(DecodeError::Empty);
    }

    let invoice = cleaned.to_lowercase();
    let (hrp, data, variant) = bech32::decode(&invoice).map_err(|_| DecodeError::InvalidBech32)?;

    if variant != bech32::Variant::Bech32 {
        return Err(DecodeError::InvalidVariant);
    }

    if data.len() < MIN_DATA_LEN {
        return Err(DecodeError::DataTooShort);
    }

    let data_no_sig = &data[..data.len() - 104];
    let (description, expiry_seconds) = decode_tagged_fields(data_no_sig)?;
    let amount_msats = parse_amount_msats(&hrp)?;

    Ok(DecodedInvoice {
        amount_msats,
        description,
        expiry_seconds,
    })
}

/// Format millisatoshi amount for display.
pub(crate) fn format_amount(amount_msats: Option<u64>) -> String {
    match amount_msats {
        None => "Any amount".to_string(),
        Some(msats) if msats % 1000 == 0 => format!("{} sats", msats / 1000),
        Some(msats) => format!("{} msats", msats),
    }
}

/// Format seconds into a short human-readable duration (e.g. "1h 30m").
pub(crate) fn format_expiry(seconds: u64) -> String {
    if seconds == 0 {
        return "0s".to_string();
    }

    let mut remaining = seconds;
    let days = remaining / 86_400;
    remaining %= 86_400;
    let hours = remaining / 3_600;
    remaining %= 3_600;
    let minutes = remaining / 60;
    let secs = remaining % 60;

    let mut parts = Vec::new();
    if days > 0 {
        parts.push(format!("{days}d"));
    }
    if hours > 0 {
        parts.push(format!("{hours}h"));
    }
    if minutes > 0 {
        parts.push(format!("{minutes}m"));
    }
    if secs > 0 || parts.is_empty() {
        parts.push(format!("{secs}s"));
    }

    parts.join(" ")
}

// ---------------------------------------------------------------------------
// Internals
// ---------------------------------------------------------------------------

/// Strip optional `lightning:` URI prefix and whitespace.
fn sanitize_payment_request(input: &str) -> String {
    let trimmed = input.trim();
    trimmed
        .strip_prefix("lightning:")
        .or_else(|| trimmed.strip_prefix("LIGHTNING:"))
        .unwrap_or(trimmed)
        .trim()
        .to_string()
}

/// Parse the amount encoded in the human-readable part of a BOLT #11 invoice.
///
/// The HRP has the form `ln<network>[<amount><multiplier>]`.
/// Returns `None` for invoices with no amount specified.
fn parse_amount_msats(hrp: &str) -> Result<Option<u64>, DecodeError> {
    if !hrp.starts_with("ln") {
        return Err(DecodeError::InvalidPrefix);
    }

    // Skip the "ln" prefix and network identifier to reach the amount portion.
    let rest = &hrp[2..];
    let amount_start = rest.find(|c: char| c.is_ascii_digit());
    let amount_part = match amount_start {
        Some(idx) => &rest[idx..],
        None => return Ok(None),
    };

    if amount_part.is_empty() {
        return Ok(None);
    }

    let last = amount_part.chars().last().unwrap();
    let (number_str, multiplier) = if matches!(last, 'm' | 'u' | 'n' | 'p') {
        (&amount_part[..amount_part.len() - 1], Some(last))
    } else {
        (amount_part, None)
    };

    if number_str.is_empty() {
        return Err(DecodeError::InvalidAmount);
    }

    let value: u64 = number_str.parse().map_err(|_| DecodeError::InvalidAmount)?;

    let msats = match multiplier {
        None => value
            .checked_mul(100_000_000_000)
            .ok_or(DecodeError::AmountOverflow)?,
        Some('m') => value
            .checked_mul(100_000_000)
            .ok_or(DecodeError::AmountOverflow)?,
        Some('u') => value
            .checked_mul(100_000)
            .ok_or(DecodeError::AmountOverflow)?,
        Some('n') => value.checked_mul(100).ok_or(DecodeError::AmountOverflow)?,
        Some('p') => {
            if value % 10 != 0 {
                return Err(DecodeError::SubMillisatoshi);
            }
            value / 10
        }
        _ => return Err(DecodeError::InvalidAmount),
    };

    Ok(Some(msats))
}

/// Convert a slice of 5-bit values to a byte vector (8-bit).
fn five_bit_to_bytes(data: &[bech32::u5]) -> Result<Vec<u8>, DecodeError> {
    let mut acc: u32 = 0;
    let mut bits: u32 = 0;
    let mut out = Vec::new();

    for value in data {
        acc = (acc << 5) | (value.to_u8() as u32);
        bits += 5;

        while bits >= 8 {
            bits -= 8;
            out.push(((acc >> bits) & 0xff) as u8);
        }
    }

    if bits > 0 {
        let mask = (1u32 << bits) - 1;
        if (acc & mask) != 0 {
            return Err(DecodeError::InvalidPadding);
        }
    }

    Ok(out)
}

/// Decode the tagged fields from a BOLT #11 invoice's 5-bit data.
///
/// Extracts the description (`d` tag) and expiry (`x` tag).
/// The first 7 words are the timestamp; everything after (minus the
/// trailing 104-word signature) consists of tagged fields.
fn decode_tagged_fields(data: &[bech32::u5]) -> Result<(Option<String>, u64), DecodeError> {
    if data.len() < 7 {
        return Err(DecodeError::DataTooShort);
    }

    // Skip the 7-word timestamp.
    let mut index = 7;

    let mut description = None;
    let mut expiry_seconds = 3600u64;

    while index < data.len() {
        let tag_value = data[index].to_u8() as usize;
        index += 1;

        if index + 1 >= data.len() {
            return Err(DecodeError::TagLengthMissing);
        }

        let data_len = ((data[index].to_u8() as usize) << 5) | data[index + 1].to_u8() as usize;
        index += 2;

        if index + data_len > data.len() {
            return Err(DecodeError::TagLengthOverflow);
        }

        let tag_data = &data[index..index + data_len];
        index += data_len;

        let tag_char = BECH32_CHARSET.get(tag_value).copied().unwrap_or(b'?') as char;

        match tag_char {
            'd' => {
                let bytes = five_bit_to_bytes(tag_data)?;
                description =
                    Some(String::from_utf8(bytes).map_err(|_| DecodeError::InvalidDescription)?);
            }
            'x' => {
                let mut value = 0u64;
                for item in tag_data {
                    value = (value << 5) | item.to_u8() as u64;
                }
                expiry_seconds = value;
            }
            _ => {}
        }
    }

    Ok((description, expiry_seconds))
}
