//! Cartridge size limits shared by the uploader and Supafaust frontend.

pub const MINIMUM_BYTES: usize = 32 * 1024;
const MAXIMUM_PAYLOAD_BYTES: usize = 8 * 1024 * 1024;
const COPIER_HEADER_BYTES: usize = 512;
pub const MAXIMUM_BYTES: usize = MAXIMUM_PAYLOAD_BYTES + COPIER_HEADER_BYTES;

pub const fn valid_size(bytes: usize) -> bool {
    // Supafaust treats a 512-byte remainder in a 32 KiB bank as a header.
    let payload = if bytes % MINIMUM_BYTES == COPIER_HEADER_BYTES {
        bytes - COPIER_HEADER_BYTES
    } else {
        bytes
    };
    payload >= MINIMUM_BYTES && payload <= MAXIMUM_PAYLOAD_BYTES
}
