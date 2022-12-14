use std::collections::HashMap;

use super::{FirmwareDataInterface, FirmwareDataRaw};

const HEX_LINE_MIN_CHARS: usize = 10;

// Hex File Record Type ---------------------------------------------------------------------------

#[derive(Debug, PartialEq, Clone)]
pub enum RecordType {
    Data,
    EndOfFile,
    ExtendedSegmentAddress,
    StartSegmentAddress,
    ExtendedLinearAddress,
    StartLinearAddress,
}

impl RecordType {
    pub fn from_byte(byte: u8) -> Option<RecordType> {
        match byte {
            0x00 => Some(RecordType::Data),
            0x01 => Some(RecordType::EndOfFile),
            0x02 => Some(RecordType::ExtendedSegmentAddress),
            0x03 => Some(RecordType::StartSegmentAddress),
            0x04 => Some(RecordType::ExtendedLinearAddress),
            0x05 => Some(RecordType::StartLinearAddress),
            _ => None,
        }
    }
}

// Hex File Error Type ----------------------------------------------------------------------------

#[derive(Debug, PartialEq, Clone)]
pub enum ErrorType {
    NoValidData,
    ByteCountParseError,
    OffsetParseError,
    RecordTypeParseError,
    DataParseError,
    ChecksumParseError,
    InvalidEntryLength,
    InvalidByteCount,
    InvalidRecordType,
    InvalidChecksum,
}

// Hex File Entry ----------------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct Entry {
    byte_count: u8,
    offset: u16,
    record_type: RecordType,
    data: Vec<u8>,
    checksum: u16,
}

impl Entry {
    // Parse a single line of hex file
    // Important: Line should not contain ':'!
    pub fn from_hex_line(line: &str) -> Result<Entry, ErrorType> {
        if line.len() < HEX_LINE_MIN_CHARS {
            return Err(ErrorType::InvalidEntryLength);
        }

        // Parse byte count
        let byte_count =
            u8::from_str_radix(&line[0..2], 16).map_err(|_| ErrorType::ByteCountParseError)?;
        let expected_byte_count = byte_count as usize * 2 + HEX_LINE_MIN_CHARS;

        // Check if byte count is correct
        if line.len() != expected_byte_count {
            return Err(ErrorType::InvalidByteCount);
        }

        // Parse offset
        let offset =
            u16::from_str_radix(&line[2..6], 16).map_err(|_| ErrorType::OffsetParseError)?;

        // Parse record type
        let record_type_raw =
            u8::from_str_radix(&line[6..8], 16).map_err(|_| ErrorType::RecordTypeParseError)?;
        let record_type =
            RecordType::from_byte(record_type_raw).ok_or(ErrorType::InvalidRecordType)?;

        // Parse data
        let mut data = Vec::new();
        for i in 0..byte_count {
            let data_raw = u8::from_str_radix(&line[8 + i as usize * 2..10 + i as usize * 2], 16)
                .map_err(|_| ErrorType::DataParseError)?;
            data.push(data_raw);
        }

        // Parse checksum
        let checksum = u16::from_str_radix(
            &line[8 + byte_count as usize * 2..10 + byte_count as usize * 2],
            16,
        )
        .map_err(|_| ErrorType::ChecksumParseError)?;

        // Check checksum
        let mut checksum_calc = 0u16;
        checksum_calc += byte_count as u16;
        checksum_calc += (offset >> 8) as u16;
        checksum_calc += (offset & 0xFF) as u16;
        checksum_calc += record_type_raw as u16;
        for i in 0..byte_count {
            checksum_calc += data[i as usize] as u16;
        }
        checksum_calc = (!checksum_calc + 1) & 0x00FF;

        if checksum_calc != checksum {
            return Err(ErrorType::InvalidChecksum);
        }

        Ok(Entry {
            byte_count,
            offset,
            record_type,
            data,
            checksum,
        })
    }
}

// Hex File Representation ------------------------------------------------------------------------

pub struct HexFile {
    data: FirmwareDataRaw,
}

impl HexFile {
    pub fn from_file(filename: &str) -> Result<HexFile, String> {
        match std::fs::read_to_string(filename) {
            Ok(f) => return Self::from_string(&f.as_str()),
            Err(e) => {
                return Err(format!("Failed to open file '{}': {}", filename, e));
            }
        };
    }

    pub fn from_string(hex_data: &str) -> Result<HexFile, String> {
        Self::parse_hex_file(hex_data)
    }

    pub fn get_data(&self) -> &FirmwareDataRaw {
        &self.data
    }

    fn parse_hex_file(hex_data: &str) -> Result<HexFile, String> {
        let mut entries = Vec::new();

        // Pares every line in hex file
        let mut line_idx = 0;
        for line in hex_data.lines() {
            if line.len() > 0 && line.chars().nth(0).unwrap() == ':' {
                let entry = match Entry::from_hex_line(&line[1..]) {
                    Ok(e) => e,
                    Err(e) => {
                        return Err(format!(
                            "Hex file parse error: {:?} in line {}",
                            e, line_idx
                        ));
                    }
                };
                entries.push(entry);
            }

            line_idx += 1;
        }

        // Convert to map
        let mut firmware_map = FirmwareDataRaw::new();
        let mut address_extended = 0 as u32;
        for entry in &entries {
            match entry.record_type {
                RecordType::ExtendedLinearAddress => {
                    address_extended = (entry.data[0] as u32) << 24 | (entry.data[1] as u32) << 16;
                }
                RecordType::Data => {
                    let address = address_extended | entry.offset as u32;
                    for i in 0..entry.byte_count {
                        firmware_map.insert(address + i as u32, entry.data[i as usize]);
                    }
                }
                RecordType::EndOfFile => {
                    break;
                }
                _ => {}
            }
        }

        if firmware_map.len() == 0 {
            return Err(format!("Hex file does not contain valid data!"));
        } else {
            return Ok(HexFile { data: firmware_map });
        }
    }
}

pub fn parse_hex_file(hex_file: &str) -> Result<HashMap<u32, u8>, ErrorType> {
    let mut entries = Vec::new();

    let mut line_idx = 0;
    for line in hex_file.lines() {
        if line.len() > 0 && line.chars().nth(0).unwrap() == ':' {
            let entry = match Entry::from_hex_line(&line[1..]) {
                Ok(e) => e,
                Err(e) => {
                    eprintln!("Hex file parse error in line {}", line_idx);
                    return Err(e);
                }
            };
            entries.push(entry);
        }

        line_idx += 1;
    }

    // Convert to map
    let mut firmware_map = HashMap::new();
    let mut address_extended = 0 as u32;
    for entry in &entries {
        match entry.record_type {
            RecordType::ExtendedLinearAddress => {
                address_extended = (entry.data[0] as u32) << 24 | (entry.data[1] as u32) << 16;
            }
            RecordType::Data => {
                let address = address_extended | entry.offset as u32;
                for i in 0..entry.byte_count {
                    firmware_map.insert(address + i as u32, entry.data[i as usize]);
                }
            }
            RecordType::EndOfFile => {
                break;
            }
            _ => {}
        }
    }

    if firmware_map.len() == 0 {
        return Err(ErrorType::NoValidData);
    }

    Ok(firmware_map)
}

impl FirmwareDataInterface for HexFile {
    fn get_firmware_data(&self) -> Option<&FirmwareDataRaw> {
        Some(&self.data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_file_read_dos_format() {
        let hex_data = ":020000040800F2\r\n\
             :102000000000012009230008D1220008D522000881\r\n\
             :10201000D9220008DD220008E122000800000000AB\r\n\
             :00000001FF\r\n";

        let expected_addresses: Vec<u32> = (0x8002000..0x8002020).collect();
        let expected_data = vec![
            0x00, 0x00, 0x01, 0x20, 0x09, 0x23, 0x00, 0x08, 0xD1, 0x22, 0x00, 0x08, 0xD5, 0x22,
            0x00, 0x08, 0xD9, 0x22, 0x00, 0x08, 0xDD, 0x22, 0x00, 0x08, 0xE1, 0x22, 0x00, 0x08,
            0x00, 0x00, 0x00, 0x00,
        ];

        let hex_file = HexFile::from_string(hex_data).unwrap();

        let mut idx = 0;
        let mut keys: Vec<u32> = hex_file.get_data().keys().map(|x| *x).collect();
        keys.sort();
        for key in keys {
            assert_eq!(key, expected_addresses[idx]);
            assert_eq!(hex_file.get_data()[&key], expected_data[idx]);
            idx += 1;
        }
    }

    #[test]
    fn hex_file_read_linux_format() {
        let hex_data = ":020000040800F2\n\
             :102000000000012009230008D1220008D522000881\n\
             :10201000D9220008DD220008E122000800000000AB\n\
             :00000001FF\n";

        let expected_addresses: Vec<u32> = (0x8002000..0x8002020).collect();
        let expected_data = vec![
            0x00, 0x00, 0x01, 0x20, 0x09, 0x23, 0x00, 0x08, 0xD1, 0x22, 0x00, 0x08, 0xD5, 0x22,
            0x00, 0x08, 0xD9, 0x22, 0x00, 0x08, 0xDD, 0x22, 0x00, 0x08, 0xE1, 0x22, 0x00, 0x08,
            0x00, 0x00, 0x00, 0x00,
        ];

        let hex_file = HexFile::from_string(hex_data).unwrap();

        let mut idx = 0;
        let mut keys: Vec<u32> = hex_file.get_data().keys().map(|x| *x).collect();
        keys.sort();
        for key in keys {
            assert_eq!(key, expected_addresses[idx]);
            assert_eq!(hex_file.get_data()[&key], expected_data[idx]);
            idx += 1;
        }
    }

    #[test]
    fn entry_from_hex_line_too_short() {
        let line = "00";
        let result = Entry::from_hex_line(line);
        assert_eq!(result.unwrap_err(), ErrorType::InvalidEntryLength);
    }

    #[test]
    fn entry_from_hex_line_byte_count_error() {
        let line = "0X0000040800F2";
        let result = Entry::from_hex_line(line);
        assert_eq!(result.unwrap_err(), ErrorType::ByteCountParseError);
    }

    #[test]
    fn entry_from_hex_line_byte_count_valid() {
        let line = "102000000000012009230008D1220008D522000881";
        let result = Entry::from_hex_line(line);

        assert!(result.is_ok());
        assert_eq!(result.unwrap().byte_count, 0x10);
    }

    #[test]
    fn entry_from_hex_line_offset_error() {
        let line = "102!00000000012009230008D1220008D522000881";
        let result = Entry::from_hex_line(line);

        assert_eq!(result.unwrap_err(), ErrorType::OffsetParseError);
    }

    #[test]
    fn entry_from_hex_line_offset_valid() {
        let line = "102000000000012009230008D1220008D522000881";
        let result = Entry::from_hex_line(line);

        assert!(result.is_ok());
        assert_eq!(result.unwrap().offset, 0x2000);
    }

    #[test]
    fn entry_from_hex_line_record_error() {
        let line = "020000?40800F2";
        let result = Entry::from_hex_line(line);

        assert_eq!(result.unwrap_err(), ErrorType::RecordTypeParseError);
    }

    #[test]
    fn entry_from_hex_line_record_invalid() {
        let line = "020000F10800F2";
        let result = Entry::from_hex_line(line);

        assert_eq!(result.unwrap_err(), ErrorType::InvalidRecordType);
    }

    #[test]
    fn entry_from_hex_line_record_valid() {
        let line = "020000040800F2";
        let result = Entry::from_hex_line(line);

        assert!(result.is_ok());
        assert_eq!(
            result.unwrap().record_type,
            RecordType::ExtendedLinearAddress
        );
    }

    #[test]
    fn entry_from_hex_line_invalid_byte_count() {
        let line = "030000040800F2";
        let result = Entry::from_hex_line(line);

        assert_eq!(result.unwrap_err(), ErrorType::InvalidByteCount);
    }

    #[test]
    fn entry_from_hex_line_data() {
        let line = "102000000000012009230008D1220008D522000881";
        let result = Entry::from_hex_line(line);

        assert!(result.is_ok());
        assert_eq!(
            result.unwrap().data,
            vec![
                0x00, 0x00, 0x01, 0x20, 0x09, 0x23, 0x00, 0x08, 0xD1, 0x22, 0x00, 0x08, 0xD5, 0x22,
                0x00, 0x08
            ]
        );
    }

    #[test]
    fn entry_from_hex_line_checksum_parse_error() {
        let line = "102000000000012009230008D1220008D52200088!";
        let result = Entry::from_hex_line(line);

        assert_eq!(result.unwrap_err(), ErrorType::ChecksumParseError);
    }

    #[test]
    fn entry_from_hex_line_checksum_parse_valid() {
        let line = "102000000000012009230008D1220008D522000881";
        let result = Entry::from_hex_line(line);

        assert!(result.is_ok());
        assert_eq!(result.unwrap().checksum, 0x81);
    }

    #[test]
    fn entry_from_hex_line_checksum_error() {
        let line = "102000000000012009230008D1220008D522000880";
        let result = Entry::from_hex_line(line);

        assert_eq!(result.unwrap_err(), ErrorType::InvalidChecksum);
    }

    #[test]
    fn record_type_from_byte() {
        assert_eq!(RecordType::from_byte(0x00), Some(RecordType::Data));
        assert_eq!(RecordType::from_byte(0x01), Some(RecordType::EndOfFile));
        assert_eq!(
            RecordType::from_byte(0x02),
            Some(RecordType::ExtendedSegmentAddress)
        );
        assert_eq!(
            RecordType::from_byte(0x03),
            Some(RecordType::StartSegmentAddress)
        );
        assert_eq!(
            RecordType::from_byte(0x04),
            Some(RecordType::ExtendedLinearAddress)
        );
        assert_eq!(
            RecordType::from_byte(0x05),
            Some(RecordType::StartLinearAddress)
        );
        assert_eq!(RecordType::from_byte(0x06), None);
    }

    #[test]
    fn hex_file_firmware_data_interface_trait() {
        let hex_data = ":020000040800F2\r\n\
             :102000000000012009230008D1220008D522000881\r\n\
             :10201000D9220008DD220008E122000800000000AB\r\n\
             :00000001FF\r\n";

        let expected_addresses: Vec<u32> = (0x8002000..0x8002020).collect();
        let expected_data = vec![
            0x00, 0x00, 0x01, 0x20, 0x09, 0x23, 0x00, 0x08, 0xD1, 0x22, 0x00, 0x08, 0xD5, 0x22,
            0x00, 0x08, 0xD9, 0x22, 0x00, 0x08, 0xDD, 0x22, 0x00, 0x08, 0xE1, 0x22, 0x00, 0x08,
            0x00, 0x00, 0x00, 0x00,
        ];

        let hex_file = HexFile::from_string(hex_data).unwrap();
        let firmware_interface_trait: Box<dyn FirmwareDataInterface> = Box::new(hex_file);

        let mut idx = 0;
        let mut keys: Vec<u32> = firmware_interface_trait
            .get_firmware_data()
            .unwrap()
            .keys()
            .map(|x| *x)
            .collect();
        keys.sort();
        for key in keys {
            assert_eq!(key, expected_addresses[idx]);
            assert_eq!(
                firmware_interface_trait.get_firmware_data().unwrap()[&key],
                expected_data[idx]
            );
            idx += 1;
        }
    }
}
