use std::{cell::Ref, fmt, rc::Rc};

///
/// Flash Description Error
///
#[derive(Debug, PartialEq, Clone)]
pub enum FlashDescError {
    /// Flash section address is not aligned to flash pages
    FlashSectionAddressInvalid,

    /// Flash section size is not multiple of page size
    FlashSectionSizeInvalid,

    /// Flash section does not fit into flash
    FlashSectionSizeTooBig,

    /// Flash area is already used by another section
    FlashAreaAlreadyUsed,

    /// Flash name is already used
    FlashNameAlreadyUsed,
}

/// Implementation of the Display trait for the FlashDescError enumeration
impl fmt::Display for FlashDescError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            FlashDescError::FlashSectionAddressInvalid => {
                write!(f, "FlashSectionAddressInvalid: Flash section address must start aligned to pages!")
            }

            FlashDescError::FlashSectionSizeInvalid => {
                write!(
                    f,
                    "FlashSectionSizeInvalid: Flash section size must be multiple of page size!"
                )
            }

            FlashDescError::FlashSectionSizeTooBig => {
                write!(
                    f,
                    "FlashSectionSizeTooBig: Flash section does not fit into flash!"
                )
            }

            FlashDescError::FlashAreaAlreadyUsed => {
                write!(
                    f,
                    "FlashAreaAlreadyUsed: Flash area is already used by another section!"
                )
            }

            FlashDescError::FlashNameAlreadyUsed => {
                write!(f, "FlashNameAlreadyUsed: Flash name is already used!")
            }
        }
    }
}

// Flash ------------------------------------------------------------------------------------------

///
/// Description of the structure of the flash
///
pub struct FlashDesc {
    address: u32,
    size: u32,
    page_size: u32,
    section_lst: Vec<FlashSection>,
}

impl FlashDesc {
    ///
    /// Creates a new flash description.
    ///
    pub fn new(address: u32, size: u32, page_size: u32) -> FlashDesc {
        FlashDesc {
            address: address,
            size: size,
            page_size: page_size,
            section_lst: Vec::new(),
        }
    }

    pub fn add_section(
        &mut self,
        name: &str,
        address: u32,
        size: u32,
    ) -> Result<(), FlashDescError> {
        // Check if section name is already used
        for section in &self.section_lst {
            if section.get_name() == name {
                return Err(FlashDescError::FlashNameAlreadyUsed);
            }
        }

        // section must start aligned to pages
        if address % self.page_size != 0 {
            return Err(FlashDescError::FlashSectionAddressInvalid);
        }

        // section size must be multiple of page szize
        if size % self.page_size != 0 {
            return Err(FlashDescError::FlashSectionSizeInvalid);
        }

        // check if section is to big for flash
        if address + size > self.address + self.size {
            return Err(FlashDescError::FlashSectionSizeTooBig);
        }

        // check if section overlaops with other sections
        for section in &self.section_lst {
            if section.get_address() <= address
                && section.get_address() + section.get_size() > address
            {
                return Err(FlashDescError::FlashAreaAlreadyUsed);
            }

            if section.get_address() < address + size
                && section.get_address() + section.get_size() >= address + size
            {
                return Err(FlashDescError::FlashAreaAlreadyUsed);
            }
        }

        // calculate page offset
        let page_id = (address - self.address) / self.page_size;

        // add flash section
        self.section_lst
            .push(FlashSection::new(name, address, size, page_id));

        Ok(())
    }

    // Getters ------------------------------------------------------------------------------------

    ///
    /// Returns the start address of the flash memory.
    ///
    pub fn get_address(&self) -> u32 {
        self.address
    }

    ///
    /// Returns the size of the flash memory.
    ///
    pub fn get_size(&self) -> u32 {
        self.size
    }

    ///
    /// Returns the page size of the flash memory.
    ///
    pub fn get_page_size(&self) -> u32 {
        self.page_size
    }

    ///
    /// Returns the number of pages in the flash memory
    ///
    pub fn get_num_pages(&self) -> u32 {
        self.size / self.page_size
    }

    ///
    /// Returns the number of sections in the flash memory.
    ///
    pub fn get_num_section(&self) -> usize {
        self.section_lst.len()
    }

    ///
    /// Get section name list
    ///
    pub fn get_section_name_list(&self) -> Vec<String> {
        let mut name_lst = Vec::new();

        for section in &self.section_lst {
            name_lst.push(section.get_name().clone());
        }

        name_lst
    }

    ///
    /// Returns the start address of the section with the given name.
    ///
    pub fn get_section_address(&self, name: &str) -> Option<u32> {
        match self._get_section(name) {
            Some(section) => Some(section.get_address()),
            None => None,
        }
    }

    ///
    /// Returns the size of the section with the given name.
    ///
    pub fn get_section_size(&self, name: &str) -> Option<u32> {
        match self._get_section(name) {
            Some(section) => Some(section.get_size()),
            None => None,
        }
    }

    ///
    /// Returns the page id of the section with the given name.
    ///
    pub fn get_section_page_id(&self, name: &str) -> Option<u32> {
        match self._get_section(name) {
            Some(section) => Some(section.get_flash_page_id()),
            None => None,
        }
    }

    ///
    /// Returns the number of pages of the section with the given name.
    ///
    pub fn get_section_num_pages(&self, name: &str) -> Option<u32> {
        match self._get_section(name) {
            Some(section) => Some(section.get_size() / self.page_size),
            None => None,
        }
    }

    // Private functions ---------------------------------------------------------------------------

    fn _get_section(&self, name: &str) -> Option<&FlashSection> {
        for section in &self.section_lst {
            if section.get_name() == name {
                return Some(section);
            }
        }

        None
    }
}

/// Flash section ---------------------------------------------------------------------------------

///
/// Representation of a flash section
///
pub struct FlashSection {
    /// Name of the section (unique)
    name: String,

    /// Absolute address of section
    address: u32,

    /// Size of section in bytes
    size: u32,

    /// Page id in flash
    flash_page_id: u32,
}

impl FlashSection {
    ///
    /// Create a new flash section
    ///
    pub fn new(name: &str, address: u32, size: u32, flash_page_id: u32) -> FlashSection {
        FlashSection {
            name: name.to_string(),
            address: address,
            size: size,
            flash_page_id: flash_page_id,
        }
    }

    // Getters ------------------------------------------------------------------------------------

    ///
    /// Returns the name of the flash section.
    ///    
    pub fn get_name(&self) -> &String {
        &self.name
    }

    ///
    /// Returns the start address of the flash section.
    ///
    pub fn get_address(&self) -> u32 {
        self.address
    }

    ///
    /// Returns the size of the flash section.
    ///
    pub fn get_size(&self) -> u32 {
        self.size
    }

    ///
    /// Returns the page offset of the flash section.
    ///
    pub fn get_flash_page_id(&self) -> u32 {
        self.flash_page_id
    }
}

// Tests ------------------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flash_desc_new() {
        let flash_desc = FlashDesc::new(0x08000000, 0x10000, 0x400);

        assert_eq!(flash_desc.get_address(), 0x08000000);
        assert_eq!(flash_desc.get_size(), 0x10000);
        assert_eq!(flash_desc.get_page_size(), 0x400);
        assert_eq!(flash_desc.get_num_pages(), 0x40);
    }

    #[test]
    fn flash_desc_add_section() {
        let mut flash_desc = FlashDesc::new(0x08000000, 0x10000, 0x400);

        let name = String::from("test");
        let address = 0x08000000;
        let size = 0x1000;

        flash_desc.add_section(&name, address, size).unwrap();

        assert_eq!(flash_desc.section_lst.len(), 1);
        assert_eq!(flash_desc.section_lst[0].get_name(), &name);
        assert_eq!(flash_desc.section_lst[0].get_address(), address);
        assert_eq!(flash_desc.section_lst[0].get_size(), size);
        assert_eq!(flash_desc.section_lst[0].get_flash_page_id(), 0);
    }

    #[test]
    fn flash_desc_add_section_name_error() {
        let mut flash_desc = FlashDesc::new(0x08000000, 0x10000, 0x400);

        let name = String::from("test");
        let address = 0x08000000;
        let size = 0x1000;

        flash_desc.add_section(&name, address, size).unwrap();

        let result = flash_desc.add_section(&name, address, size);

        assert_eq!(result, Err(FlashDescError::FlashNameAlreadyUsed));
    }

    #[test]
    fn flash_desc_add_section_invalid_address() {
        let mut flash_desc = FlashDesc::new(0x08000000, 0x10000, 0x400);

        let name = String::from("test");
        let address = 0x08000001;
        let size = 0x1000;

        let result = flash_desc.add_section(&name, address, size);

        assert_eq!(result.is_err(), true);
        assert_eq!(
            result.unwrap_err(),
            FlashDescError::FlashSectionAddressInvalid
        );
    }

    #[test]
    fn flash_desc_add_section_invalid_size() {
        let mut flash_desc = FlashDesc::new(0x08000000, 0x10000, 0x400);

        let name = String::from("test");
        let address = 0x08000000;
        let size = 0x1001;

        let result = flash_desc.add_section(&name, address, size);

        assert_eq!(result.is_err(), true);
        assert_eq!(result.unwrap_err(), FlashDescError::FlashSectionSizeInvalid);
    }

    #[test]
    fn flash_desc_add_section_too_big() {
        let mut flash_desc = FlashDesc::new(0x08000000, 0x10000, 0x400);

        let name = String::from("test");
        let address = 0x08000000;
        let size = 0x20000;

        let result = flash_desc.add_section(&name, address, size);

        assert_eq!(result.is_err(), true);
        assert_eq!(result.unwrap_err(), FlashDescError::FlashSectionSizeTooBig);
    }

    #[test]
    fn flash_desc_add_section_overlaps() {
        let mut flash_desc = FlashDesc::new(0x08000000, 0x10000, 0x400);

        let name = String::from("test");
        let address = 0x08000000;
        let size = 0x1000;

        flash_desc.add_section(&name, address, size).unwrap();

        let name = String::from("test2");
        let address = 0x08000400;
        let size = 0x1000;

        let result = flash_desc.add_section(&name, address, size);

        assert_eq!(result.is_err(), true);
        assert_eq!(result.unwrap_err(), FlashDescError::FlashAreaAlreadyUsed);
    }

    #[test]
    fn flash_desc_get_section_by_name() {
        let mut flash_desc = FlashDesc::new(0x08000000, 0x10000, 0x400);

        let name = String::from("test");
        let address = 0x08000000;
        let size = 0x1000;

        flash_desc.add_section(&name, address, size).unwrap();

        let result = flash_desc._get_section(&name);

        assert_eq!(result.is_some(), true);
        assert_eq!(result.unwrap().get_name(), &name);
        assert_eq!(result.unwrap().get_address(), address);
        assert_eq!(result.unwrap().get_size(), size);
    }

    #[test]
    fn flash_desc_get_section_by_name_none() {
        let mut flash_desc = FlashDesc::new(0x08000000, 0x10000, 0x400);

        let name = String::from("test");
        let address = 0x08000000;
        let size = 0x1000;

        flash_desc.add_section(&name, address, size).unwrap();

        let name = String::from("test2");

        let result = flash_desc._get_section(&name);

        assert_eq!(result.is_none(), true);
    }

    #[test]
    fn flash_desc_get_section_address_by_name() {
        let mut flash_desc = FlashDesc::new(0x08000000, 0x10000, 0x400);

        let name = String::from("test");
        let address = 0x08000000;
        let size = 0x1000;

        flash_desc.add_section(&name, address, size).unwrap();

        let result = flash_desc.get_section_address(&name);

        assert_eq!(result.is_some(), true);
        assert_eq!(result.unwrap(), address);
    }

    #[test]
    fn flash_desc_get_section_size_by_name() {
        let mut flash_desc = FlashDesc::new(0x08000000, 0x10000, 0x400);

        let name = String::from("test");
        let address = 0x08000000;
        let size = 0x1000;

        flash_desc.add_section(&name, address, size).unwrap();

        let result = flash_desc.get_section_size(&name);

        assert_eq!(result.is_some(), true);
        assert_eq!(result.unwrap(), size);
    }

    #[test]
    fn flash_desc_get_section_page_id_by_name() {
        let mut flash_desc = FlashDesc::new(0x08000000, 0x10000, 0x400);

        let name = String::from("test");
        let address = 0x08000000;
        let size = 0x1000;

        flash_desc.add_section(&name, address, size).unwrap();

        let result = flash_desc.get_section_page_id(&name);

        assert_eq!(result.is_some(), true);
        assert_eq!(result.unwrap(), 0);
    }

    #[test]
    fn flash_desc_get_section_num_pages_by_name() {
        let mut flash_desc = FlashDesc::new(0x08000000, 0x10000, 0x400);

        let name = String::from("test");
        let address = 0x08000000;
        let size = 0x1000;

        flash_desc.add_section(&name, address, size).unwrap();

        let result = flash_desc.get_section_num_pages(&name);

        assert_eq!(result.is_some(), true);
        assert_eq!(result.unwrap(), 4);
    }
}
