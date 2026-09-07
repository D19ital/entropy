use super::hid_parse::parse_unlock_status_response;
use super::hid_protocol::*;
use super::HidDevice;
use anyhow::{bail, Context, Result};

const MAX_DEFINITION_SIZE: u32 = 2_000_000;
const BLUETOOTH_DEFINITION_TRANSFER_ATTEMPTS: usize = 3;
const XZ_MAGIC: &[u8] = b"\xFD7zXZ\x00";

fn checked_definition_size(size: u32) -> Result<usize> {
    if size == 0 || size > MAX_DEFINITION_SIZE {
        bail!("Invalid definition size: {size}");
    }
    Ok(size as usize)
}

fn definition_transfer_attempts(bluetooth: bool) -> usize {
    if bluetooth {
        BLUETOOTH_DEFINITION_TRANSFER_ATTEMPTS
    } else {
        1
    }
}

fn decode_vial_definition(payload: &[u8]) -> Result<serde_json::Value> {
    let mut decompressed = Vec::new();
    if payload.starts_with(XZ_MAGIC) {
        lzma_rs::xz_decompress(&mut &payload[..], &mut decompressed)
            .context("Failed to decompress XZ Vial definition")?;
    } else {
        lzma_rs::lzma_decompress(&mut &payload[..], &mut decompressed)
            .context("Failed to decompress legacy LZMA Vial definition")?;
    }

    let json_str =
        std::str::from_utf8(&decompressed).context("Vial definition is not valid UTF-8")?;
    serde_json::from_str(json_str).context("Failed to parse vial JSON")
}

fn transfer_and_decode_vial_definition(
    attempts: usize,
    mut transfer: impl FnMut() -> Result<Vec<u8>>,
) -> Result<serde_json::Value> {
    debug_assert!(attempts > 0);
    for attempt in 1..=attempts {
        // A transport error already has its own per-request retry policy. Only
        // a fully transferred but corrupt definition triggers a full restart.
        let payload = transfer()?;
        match decode_vial_definition(&payload) {
            Ok(definition) => return Ok(definition),
            Err(error) if attempt < attempts => {
                log::warn!(
                    "Vial definition decode failed on attempt {attempt}/{attempts}: {error:#}; retrying complete transfer"
                );
            }
            Err(error) => return Err(error),
        }
    }
    unreachable!("definition transfer attempt count is nonzero")
}

#[cfg(not(target_arch = "wasm32"))]
impl HidDevice {
    pub fn get_protocol_version(&self) -> Result<u16> {
        let resp = self
            .usb_send(&[CMD_VIA_GET_PROTOCOL_VERSION])
            .context("failed to read VIA protocol version")?;
        // resp[1..3] = big-endian u16
        Ok(u16::from_be_bytes([resp[1], resp[2]]))
    }

    /// Returns (vial_protocol: u32, keyboard_id: u64)
    pub fn get_keyboard_id(&self) -> Result<(u32, u64)> {
        let resp = self.usb_send(&[CMD_VIA_VIAL_PREFIX, CMD_VIAL_GET_KEYBOARD_ID])?;
        let vial_proto = u32::from_le_bytes([resp[0], resp[1], resp[2], resp[3]]);
        let kb_id = u64::from_le_bytes([
            resp[4], resp[5], resp[6], resp[7], resp[8], resp[9], resp[10], resp[11],
        ]);
        Ok((vial_proto, kb_id))
    }

    pub fn get_definition_size(&self) -> Result<u32> {
        let resp = self
            .usb_send(&[CMD_VIA_VIAL_PREFIX, CMD_VIAL_GET_SIZE])
            .context("failed to read Vial definition size")?;
        // response: size as little-endian u32 starting at byte 0
        Ok(u32::from_le_bytes([resp[0], resp[1], resp[2], resp[3]]))
    }

    pub fn get_layout_json(&self) -> Result<serde_json::Value> {
        let sz = self.get_definition_size()?;
        self.get_layout_json_with_size(sz)
    }

    pub fn get_layout_json_with_size(&self, sz: u32) -> Result<serde_json::Value> {
        let sz = checked_definition_size(sz)?;
        log::info!("Vial definition compressed size: {sz} bytes");

        let attempts = definition_transfer_attempts(self.is_bluetooth_transport());
        transfer_and_decode_vial_definition(attempts, || {
            let mut payload = Vec::with_capacity(sz);
            let mut block: u32 = 0;
            let mut remaining = sz;

            while remaining > 0 {
                let mut cmd = [0u8; MSG_LEN];
                cmd[0] = CMD_VIA_VIAL_PREFIX;
                cmd[1] = CMD_VIAL_GET_DEFINITION;
                cmd[2..6].copy_from_slice(&block.to_le_bytes());
                let resp = self
                    .usb_send(&cmd)
                    .with_context(|| format!("failed to read Vial definition block {block}"))?;

                let chunk = remaining.min(MSG_LEN);
                payload.extend_from_slice(&resp[..chunk]);
                remaining -= chunk;
                block += 1;
            }

            Ok(payload)
        })
    }

    /// Check if keyboard is unlocked
    /// Returns (unlocked, unlock_keys: Vec<(row,col)>)
    pub fn get_unlock_status(&self) -> Result<(bool, Vec<(u8, u8)>)> {
        let resp = self
            .usb_send(&[CMD_VIA_VIAL_PREFIX, CMD_VIAL_GET_UNLOCK_STATUS])
            .context("failed to read Vial unlock status")?;
        // resp[0] = unlocked (1=yes), resp[1] = unlock_in_progress
        // resp[2..] = pairs of (row, col), rest filled with 0xFF
        Ok(parse_unlock_status_response(&resp))
    }

    /// Start unlock sequence — returns keys to hold (row, col pairs)
    pub fn unlock_start(&self) -> Result<()> {
        self.usb_send(&[CMD_VIA_VIAL_PREFIX, CMD_VIAL_UNLOCK_START])
            .context("failed to start Vial unlock sequence")?;
        Ok(())
    }

    /// Poll unlock status — returns (unlocked, in_progress)
    /// Returns (unlocked, in_progress, counter)
    pub fn unlock_poll(&self) -> Result<(bool, bool, u8)> {
        let resp = self
            .usb_send(&[CMD_VIA_VIAL_PREFIX, CMD_VIAL_UNLOCK_POLL])
            .context("failed to poll Vial unlock status")?;
        // resp[0] = unlocked, resp[1] = in_progress, resp[2] = counter
        Ok((resp[0] == 1, resp[1] == 1, resp[2]))
    }

    /// Lock the keyboard
    pub fn lock(&self) -> Result<()> {
        self.usb_send(&[CMD_VIA_VIAL_PREFIX, CMD_VIAL_LOCK])
            .context("failed to lock Vial device")?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::anyhow;
    use std::cell::Cell;

    fn compressed_definition(xz: bool) -> Vec<u8> {
        let json = br#"{"name":"fixture"}"#;
        let mut compressed = Vec::new();
        if xz {
            lzma_rs::xz_compress(&mut &json[..], &mut compressed).unwrap();
        } else {
            lzma_rs::lzma_compress(&mut &json[..], &mut compressed).unwrap();
        }
        compressed
    }

    #[test]
    fn xz_magic_preserves_the_xz_decode_error() {
        let error = decode_vial_definition(b"\xFD7zXZ\x00corrupt").unwrap_err();
        let message = format!("{error:#}");

        assert!(message.contains("Failed to decompress XZ Vial definition"));
        assert!(!message.contains("legacy LZMA"));
        assert!(!message.contains("LZMA header invalid properties"));
    }

    #[test]
    fn legacy_lzma_definition_remains_supported() {
        let definition = decode_vial_definition(&compressed_definition(false)).unwrap();

        assert_eq!(definition["name"], "fixture");
    }

    #[test]
    fn bluetooth_retries_a_complete_transfer_after_decode_corruption() {
        let transfers = Cell::new(0);
        let valid = compressed_definition(true);

        let definition =
            transfer_and_decode_vial_definition(definition_transfer_attempts(true), || {
                let attempt = transfers.get();
                transfers.set(attempt + 1);
                Ok(if attempt == 0 {
                    b"\xFD7zXZ\x00corrupt".to_vec()
                } else {
                    valid.clone()
                })
            })
            .unwrap();

        assert_eq!(definition["name"], "fixture");
        assert_eq!(transfers.get(), 2);
    }

    #[test]
    fn usb_does_not_retry_a_corrupt_definition() {
        let transfers = Cell::new(0);

        let error =
            transfer_and_decode_vial_definition(definition_transfer_attempts(false), || {
                transfers.set(transfers.get() + 1);
                Ok(b"\xFD7zXZ\x00corrupt".to_vec())
            })
            .unwrap_err();

        assert!(format!("{error:#}").contains("Failed to decompress XZ Vial definition"));
        assert_eq!(transfers.get(), 1);
    }

    #[test]
    fn bluetooth_does_not_restart_after_a_transfer_error() {
        let transfers = Cell::new(0);

        let error = transfer_and_decode_vial_definition(definition_transfer_attempts(true), || {
            transfers.set(transfers.get() + 1);
            Err(anyhow!("definition block timed out"))
        })
        .unwrap_err();

        assert!(format!("{error:#}").contains("definition block timed out"));
        assert_eq!(transfers.get(), 1);
    }

    #[test]
    fn invalid_definition_sizes_are_rejected_before_transfer_policy() {
        assert!(checked_definition_size(0).is_err());
        assert!(checked_definition_size(MAX_DEFINITION_SIZE + 1).is_err());
        assert_eq!(checked_definition_size(1).unwrap(), 1);
        assert_eq!(
            definition_transfer_attempts(true),
            BLUETOOTH_DEFINITION_TRANSFER_ATTEMPTS
        );
        assert_eq!(definition_transfer_attempts(false), 1);
    }
}
