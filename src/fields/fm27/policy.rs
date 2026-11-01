//! Windows Steam 27.0.0+2486695 policy trial metadata. Not generic editable fields.
//! Farnham/Dover permission controls and native Ccsp dispatch: see layout notes.

pub(crate) struct PolicyLayout {
    pub hash: u64,
    pub club_vtable_rva: u64,
    pub vtable_offset: usize,
    pub policy_offset: usize,
    pub policy_len: usize,
    pub contract_owner_offset: usize,
    pub original: u8,
    pub proposed: u8,
}

pub(crate) const WINDOWS_STEAM: PolicyLayout = PolicyLayout {
    hash: 0x74A331ED3E28B06C,
    club_vtable_rva: 0x4CDA1F8,
    vtable_offset: 0,
    policy_offset: 0xE0,
    policy_len: 7,
    contract_owner_offset: 8,
    original: 4,
    proposed: 0,
};
