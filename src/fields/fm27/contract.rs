//! Read-only contract inspection metadata, not a contract-type/write mapping.
//! Windows Steam 27.0.0+2486695, 8/13 Sep 2026 saves. See the matching layout notes.

pub(crate) struct ContractInspection {
    pub module_header_hash: u64,
    pub contract_vtable_rva: u64,
    pub vtable_ptr_offset: usize,
    pub owner_ptr_offset: usize,
    pub type_byte_offset: usize,
    pub type_labels: &'static [(u8, &'static str)],
}

impl ContractInspection {
    /// Exact observed byte values only. Do not mask unknown bits or coerce a
    /// new value to one of the witnessed contract types.
    pub(crate) fn type_label(&self, value: u8) -> Option<&'static str> {
        self.type_labels
            .iter()
            .find(|(code, _)| *code == value)
            .map(|(_, label)| *label)
    }
}

// Re-witnessed against Roman Charles-Cook, Sam Hutchinson, Jack Dean,
// Brad House and Aaron Williams (UID 2000445820). The owner is the embedded
// person object, not the start of the player object. This does not calibrate
// the rest of the FM27 schema or prove any conversion is safe.
pub(crate) const STEAM_INSPECTION: ContractInspection = ContractInspection {
    // Uses the existing editor gate's historical multiplier, not standard FNV-1a.
    module_header_hash: 0x74A331ED3E28B06C,
    // All seven witnesses share this contract class. Bounded code reads:
    // setter RVA 0x2342E73 stores one byte at +0xC3; serializer RVA 0x2337E0A
    // takes +0xC3 and passes length 1. No game functions were invoked.
    contract_vtable_rva: 0x4B00AE8,
    vtable_ptr_offset: 0x00,
    owner_ptr_offset: 0x08,
    type_byte_offset: 0xC3,
    type_labels: &[
        (0, "Part Time Contract"),
        (1, "Full Time Contract"),
        (4, "Non-Contract"),
    ],
};
