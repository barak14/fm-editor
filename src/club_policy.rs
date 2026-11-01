//! A narrowly scoped policy trial. No write occurs while preparing a proposal.
//! The opaque guard must accompany Apply AND Undo through WriteTracker.

use crate::fields::fm27::policy::{PolicyLayout, WINDOWS_STEAM};
use crate::fields::{SubRecordParent, VersionedSchemas};
use crate::game_version::GameBuild;
use crate::process::FmProcess;

type Reader<'a> = dyn Fn(u64, usize) -> Result<Vec<u8>, String> + 'a;
type Writer<'a> = dyn Fn(u64, &[u8]) -> Result<(), String> + 'a;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ClubIdentity {
    pub address: u64,
    pub index: usize,
    pub uid: u32,
}

#[derive(Clone, Debug)]
struct Evidence {
    label: &'static str,
    address: u64,
    bytes: Vec<u8>,
}

/// Not serializable, and cannot be constructed by a UI field widget or MCP caller.
#[derive(Clone, Debug)]
pub struct PolicyGuard {
    epoch: u64,
    pid: u32,
    identity: ClubIdentity,
    evidence: Vec<Evidence>,
    info_evidence: usize,
    offset: usize,
    original: u8,
    proposed: u8,
}

#[derive(Clone, Debug)]
pub struct PolicyProposal {
    pub identity: ClubIdentity,
    pub current_codes: Vec<u8>,
    pub before: u8,
    pub after: u8,
    pub guard: PolicyGuard,
}

#[derive(Debug)]
pub(crate) struct PolicyWriteError {
    pub write_attempted: bool,
    pub message: String,
}

impl PolicyWriteError {
    fn refused(message: impl Into<String>) -> Self {
        Self {
            write_attempted: false,
            message: message.into(),
        }
    }
}

impl PolicyProposal {
    pub fn address(&self) -> u64 {
        // The checked address was established during preparation.
        self.guard.evidence[self.guard.info_evidence].address + self.guard.offset as u64
    }
}

fn address(base: u64, offset: usize, len: usize) -> Result<u64, String> {
    let a = base.checked_add(u64::try_from(offset).map_err(|_| "Offset overflow")?);
    let end = a.and_then(|a| a.checked_add(u64::try_from(len.checked_sub(1)?).ok()?));
    match (a, end) {
        (Some(a), Some(b))
            if crate::player::is_valid_ptr(base)
                && crate::player::is_valid_ptr(a)
                && crate::player::is_valid_ptr(b) =>
        {
            Ok(a)
        }
        _ => Err("Invalid policy memory range".into()),
    }
}

fn read(read: &Reader<'_>, at: u64, offset: usize, len: usize) -> Result<Vec<u8>, String> {
    if len == 0 || len > 4096 {
        return Err("Policy reads must be bounded".into());
    }
    let bytes = read(address(at, offset, len)?, len)?;
    if bytes.len() != len {
        return Err("Truncated policy record".into());
    }
    Ok(bytes)
}

fn part<const N: usize>(bytes: &[u8], offset: usize) -> Result<[u8; N], String> {
    bytes
        .get(offset..offset.checked_add(N).ok_or("Offset overflow")?)
        .and_then(|b| b.try_into().ok())
        .ok_or_else(|| "Truncated policy field".into())
}

fn pointer(bytes: &[u8], offset: usize) -> Result<u64, String> {
    let p = u64::from_le_bytes(part(bytes, offset)?);
    if !crate::player::is_valid_ptr(p) {
        return Err("Invalid policy pointer".into());
    }
    Ok(p)
}

fn field(s: &'static VersionedSchemas, key: &str) -> Result<usize, String> {
    match s.field(key).and_then(|f| f.offset) {
        Some(crate::fields::FieldLocation::Direct { offset, .. }) => {
            usize::try_from(offset).map_err(|_| "Schema offset overflow".into())
        }
        _ => Err(format!("Unmapped direct policy identity field: {key}")),
    }
}

fn sub<'a>(
    subs: &'a [crate::fields::SubRecord],
    id: &str,
) -> Result<&'a crate::fields::SubRecord, String> {
    subs.iter()
        .find(|s| s.id == id)
        .ok_or_else(|| format!("Unmapped policy relationship: {id}"))
}

fn take(
    read_fn: &Reader<'_>,
    evidence: &mut Vec<Evidence>,
    at: u64,
    len: usize,
) -> Result<Vec<u8>, String> {
    take_named(read_fn, evidence, at, len, "retained save record")
}

fn take_named(
    read_fn: &Reader<'_>,
    evidence: &mut Vec<Evidence>,
    at: u64,
    len: usize,
    label: &'static str,
) -> Result<Vec<u8>, String> {
    let bytes = read(read_fn, at, 0, len)?;
    evidence.push(Evidence {
        label,
        address: at,
        bytes: bytes.clone(),
    });
    Ok(bytes)
}

fn retain_field(
    evidence: &mut Vec<Evidence>,
    base: u64,
    raw: &[u8],
    offset: usize,
    len: usize,
    label: &'static str,
) -> Result<(), String> {
    let end = offset.checked_add(len).ok_or("Guard field overflow")?;
    let bytes = raw
        .get(offset..end)
        .ok_or("Truncated guard field")?
        .to_vec();
    evidence.push(Evidence {
        label,
        address: address(base, offset, len)?,
        bytes,
    });
    Ok(())
}

fn array(
    read_fn: &Reader<'_>,
    evidence: &mut Vec<Evidence>,
    module: u64,
    begin_rva: u64,
    end_rva: u64,
) -> Result<(u64, usize), String> {
    let a = address(
        module,
        usize::try_from(begin_rva).map_err(|_| "RVA overflow")?,
        8,
    )?;
    let b = address(
        module,
        usize::try_from(end_rva).map_err(|_| "RVA overflow")?,
        8,
    )?;
    let begin = pointer(&take(read_fn, evidence, a, 8)?, 0)?;
    let end = pointer(&take(read_fn, evidence, b, 8)?, 0)?;
    let span = end
        .checked_sub(begin)
        .ok_or("Reversed policy identity array")?;
    if span % 8 != 0 || span / 8 > 1_000_000 {
        return Err("Invalid policy identity array".into());
    }
    Ok((begin, (span / 8) as usize))
}

fn supported(fm: &FmProcess) -> Result<&'static PolicyLayout, String> {
    layout_for(fm.build, fm.module_header_hash()?, cfg!(windows))
}

fn layout_for(build: GameBuild, hash: u64, windows: bool) -> Result<&'static PolicyLayout, String> {
    if !windows || build != GameBuild::Fm27Steam || hash != WINDOWS_STEAM.hash {
        return Err("Policy trial unsupported for this platform/build/fingerprint".into());
    }
    Ok(&WINDOWS_STEAM)
}

pub(crate) fn prepare(
    fm: &FmProcess,
    epoch: u64,
    identity: ClubIdentity,
) -> Result<PolicyProposal, String> {
    let layout = supported(fm)?;
    prepare_with(
        &|a, n| fm.read_bytes(a, n),
        fm.schemas(),
        layout,
        fm.gp_base,
        fm.pid,
        epoch,
        identity,
    )
}

fn prepare_with(
    read_fn: &Reader<'_>,
    s: &'static VersionedSchemas,
    layout: &PolicyLayout,
    module: u64,
    pid: u32,
    epoch: u64,
    identity: ClubIdentity,
) -> Result<PolicyProposal, String> {
    let mut evidence = Vec::new();
    let (clubs, count) = array(
        read_fn,
        &mut evidence,
        module,
        s.rva_clubs_data,
        s.rva_clubs_end,
    )?;
    if identity.uid == 0 || identity.index >= count {
        return Err("Stale club index/UID".into());
    }
    let slot = address(
        clubs,
        identity.index.checked_mul(8).ok_or("Index overflow")?,
        8,
    )?;
    if pointer(&take(read_fn, &mut evidence, slot, 8)?, 0)? != identity.address {
        return Err("Stale club address/index".into());
    }
    // An extra sample supplements array roots when FM reloads a similar save.
    take(
        read_fn,
        &mut evidence,
        clubs,
        count.min(8).checked_mul(8).ok_or("Array overflow")?,
    )?;
    let info_sub = sub(s.club_sub_records, "info")?;
    if !matches!(info_sub.parent, SubRecordParent::Direct) {
        return Err("Unsupported club info chain".into());
    }
    let info_offset = usize::try_from(info_sub.ptr_offset).map_err(|_| "Schema overflow")?;
    let uid_offset = field(s, "c_uid")?;
    let prefix_len = (info_offset + 8)
        .max(uid_offset + 4)
        .max(layout.vtable_offset + 8);
    let raw = take(read_fn, &mut evidence, identity.address, prefix_len)?;
    if u32::from_le_bytes(part(&raw, uid_offset)?) != identity.uid {
        return Err("Stale club UID".into());
    }
    if pointer(&raw, layout.vtable_offset)? != address(module, layout.club_vtable_rva as usize, 8)?
    {
        return Err("Unverified club class".into());
    }
    let info = pointer(&raw, info_offset)?;
    let info_evidence = evidence.len();
    let info_raw = take_named(read_fn, &mut evidence, info, info_sub.size, "club info")?;
    let codes = info_raw
        .get(
            layout.policy_offset
                ..layout
                    .policy_offset
                    .checked_add(layout.policy_len)
                    .ok_or("Policy overflow")?,
        )
        .ok_or("Truncated policy table")?
        .to_vec();
    if !codes.iter().all(|&c| c == layout.original) {
        return Err(format!(
            "Trial requires all witnessed policy inputs to be Non-Contract (4); observed {codes:?}. No default or coercion applied."
        ));
    }
    address(info, layout.policy_offset, 1)?;
    // Preserve the full packed date, including flags. A later day invalidates this trial and Undo.
    if s.rva_current_date == 0 {
        return Err("Unmapped current game date".into());
    }
    let date_bytes = take_named(
        read_fn,
        &mut evidence,
        address(module, s.rva_current_date as usize, 4)?,
        4,
        "game date",
    )?;
    crate::game_date::GameDate::from_bytes(&date_bytes).ok_or("Invalid current game date")?;
    let (staff, staff_count) = array(
        read_fn,
        &mut evidence,
        module,
        s.rva_staff_data,
        s.rva_staff_end,
    )?;
    let shift = s.human_manager_shift;
    let staff_uid = field(s, "s_id")?
        .checked_add(shift)
        .ok_or("Manager overflow")?;
    let contract_sub = sub(s.staff_sub_records, "contract")?;
    let team_sub = sub(s.staff_sub_records, "team")?;
    let club_sub = sub(s.staff_sub_records, "club")?;
    if !matches!(contract_sub.parent, SubRecordParent::Direct)
        || !matches!(team_sub.parent, SubRecordParent::In("contract"))
        || !matches!(club_sub.parent, SubRecordParent::In("team"))
    {
        return Err("Unsupported manager ownership chain".into());
    }
    let cp_offset = (contract_sub.ptr_offset as usize)
        .checked_add(shift)
        .ok_or("Manager overflow")?;
    let person_uid_relative = field(s, "p_uid")?
        .checked_sub(s.player_person_offset)
        .ok_or("Person schema mismatch")?;
    let person_offset = staff_uid
        .checked_sub(person_uid_relative)
        .ok_or("Manager schema mismatch")?;
    let mut owner = None;
    for index in 0..staff_count {
        let slot = address(staff, index.checked_mul(8).ok_or("Staff overflow")?, 8)?;
        let Ok(slot_raw) = read(read_fn, slot, 0, 8) else {
            continue;
        };
        let Ok(manager) = pointer(&slot_raw, 0) else {
            continue;
        };
        let Ok(header) = read(read_fn, manager, 0, 8) else {
            continue;
        };
        if u64::from_le_bytes(part(&header, 0)?)
            != module
                .checked_add(s.human_manager_vtable_rva)
                .ok_or("Vtable overflow")?
        {
            continue;
        }
        let mut proof = Vec::new();
        let candidate = (|| -> Result<(), String> {
            if pointer(&take(read_fn, &mut proof, slot, 8)?, 0)? != manager {
                return Err("Manager array changed during preview".into());
            }
            let mr = read(read_fn, manager, 0, (cp_offset + 8).max(staff_uid + 4))?;
            if u64::from_le_bytes(part(&mr, 0)?)
                != module
                    .checked_add(s.human_manager_vtable_rva)
                    .ok_or("Vtable overflow")?
            {
                return Err("Manager class changed during preview".into());
            }
            if u32::from_le_bytes(part(&mr, staff_uid)?) == 0 {
                return Err("No manager identity".into());
            }
            // Negotiation vectors and other mutable manager state lie between
            // these fields. Retain identity/ownership, not that entire prefix.
            retain_field(&mut proof, manager, &mr, 0, 8, "human manager class")?;
            retain_field(&mut proof, manager, &mr, staff_uid, 4, "human manager UID")?;
            retain_field(
                &mut proof,
                manager,
                &mr,
                cp_offset,
                8,
                "human manager contract pointer",
            )?;
            let cp = pointer(&mr, cp_offset)?;
            let ctr = take(read_fn, &mut proof, cp, contract_sub.size)?;
            if pointer(&ctr, layout.contract_owner_offset)? != address(manager, person_offset, 1)? {
                return Err("Foreign manager contract".into());
            }
            let team = pointer(&ctr, team_sub.ptr_offset as usize)?;
            let tr = take(
                read_fn,
                &mut proof,
                team,
                (club_sub.ptr_offset as usize) + 8,
            )?;
            if pointer(&tr, club_sub.ptr_offset as usize)? != identity.address {
                return Err("Another managed club".into());
            }
            Ok(())
        })();
        if candidate.is_ok() {
            owner = Some(proof);
            break;
        }
    }
    evidence.extend(
        owner.ok_or("Selected club is not linked to a validated human manager in this save")?,
    );
    for check in &evidence {
        if read(read_fn, check.address, 0, check.bytes.len())? != check.bytes {
            return Err("Save changed during policy preview; prepare again".into());
        }
    }
    Ok(PolicyProposal {
        identity,
        current_codes: codes,
        before: layout.original,
        after: layout.proposed,
        guard: PolicyGuard {
            epoch,
            pid,
            identity,
            evidence,
            info_evidence,
            offset: layout.policy_offset,
            original: layout.original,
            proposed: layout.proposed,
        },
    })
}

impl PolicyGuard {
    pub fn identity(&self) -> ClubIdentity {
        self.identity
    }

    fn validate(&self, read_fn: &Reader<'_>, expected: u8) -> Result<(), String> {
        for (index, check) in self.evidence.iter().enumerate() {
            let mut wanted = check.bytes.clone();
            if index == self.info_evidence {
                *wanted.get_mut(self.offset).ok_or("Invalid guard")? = expected;
            }
            let observed = read(read_fn, check.address, 0, wanted.len()).map_err(|e| {
                format!(
                    "Cannot validate {} at 0x{:X}: {e}",
                    check.label, check.address
                )
            })?;
            if observed != wanted {
                let offset = observed
                    .iter()
                    .zip(&wanted)
                    .position(|(a, b)| a != b)
                    .unwrap_or(0);
                return Err(format!(
                    "Stale policy preview/Undo: {} at 0x{:X} (+0x{offset:X}) changed; identity, ownership, date, policy or save must be rechecked",
                    check.label, check.address
                ));
            }
        }
        Ok(())
    }

    pub(crate) fn write(
        &self,
        fm: &FmProcess,
        epoch: u64,
        at: u64,
        expected: &[u8],
        data: &[u8],
    ) -> Result<Option<String>, PolicyWriteError> {
        supported(fm).map_err(PolicyWriteError::refused)?;
        self.session(fm.pid, epoch)
            .map_err(PolicyWriteError::refused)?;
        self.write_with(
            &|a, n| fm.read_bytes(a, n),
            &|a, b| fm.write_bytes(a, b),
            at,
            expected,
            data,
        )
    }

    fn session(&self, pid: u32, epoch: u64) -> Result<(), String> {
        if epoch != self.epoch || pid != self.pid {
            return Err("Stale policy attachment session".into());
        }
        Ok(())
    }

    fn write_with(
        &self,
        reader: &Reader<'_>,
        writer: &Writer<'_>,
        at: u64,
        expected: &[u8],
        data: &[u8],
    ) -> Result<Option<String>, PolicyWriteError> {
        (|| -> Result<(), String> {
            let target = address(self.evidence[self.info_evidence].address, self.offset, 1)?;
            let directions = (expected == [self.original] && data == [self.proposed])
                || (expected == [self.proposed] && data == [self.original]);
            if at != target || !directions {
                return Err(
                    "Policy guard permits only the previewed one-byte Apply or Undo".into(),
                );
            }
            self.validate(reader, expected[0])?;
            if read(reader, target, 0, 1)? != expected {
                return Err(
                    "Original policy byte changed immediately before writing; prepare again".into(),
                );
            }
            Ok(())
        })()
        .map_err(PolicyWriteError::refused)?;
        let result = writer(at, data);
        if let Err(check) = self.validate(reader, data[0]) {
            return Err(PolicyWriteError {
                write_attempted: true,
                message: format!(
                    "Policy write not confirmed; memory may have changed. {check}. Write API: {result:?}. Inspect readback before retrying; the untouched backup is the recovery path."
                ),
            });
        }
        Ok(result.err().map(|e|format!("Write API reported {e}, but complete guarded readback confirms the requested byte. Undo is available.")))
    }
}

#[cfg(test)]
pub(crate) fn test_proposal() -> PolicyProposal {
    tests::proposal_for_ui()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::{Cell, RefCell};
    use std::collections::BTreeMap;

    const MODULE: u64 = 0x10000000;
    const CLUBS: u64 = 0x20000000;
    const STAFF: u64 = 0x21000000;
    const CLUB: u64 = 0x30000000;
    const INFO: u64 = 0x31000000;
    const MANAGER: u64 = 0x40000000;
    const CONTRACT: u64 = 0x41000000;
    const TEAM: u64 = 0x42000000;

    struct Memory {
        bytes: RefCell<BTreeMap<u64, u8>>,
        writes: Cell<usize>,
        unreadable: Cell<bool>,
    }
    impl Memory {
        fn put(&self, a: u64, b: &[u8]) {
            for (i, &v) in b.iter().enumerate() {
                self.bytes.borrow_mut().insert(a + i as u64, v);
            }
        }
        fn read(&self, a: u64, n: usize) -> Result<Vec<u8>, String> {
            if self.unreadable.get() {
                return Err("read failed".into());
            }
            (0..n)
                .map(|i| {
                    self.bytes
                        .borrow()
                        .get(&(a + i as u64))
                        .copied()
                        .ok_or_else(|| "unmapped".into())
                })
                .collect()
        }
        fn write(&self, a: u64, b: &[u8]) -> Result<(), String> {
            self.writes.set(self.writes.get() + 1);
            self.put(a, b);
            Ok(())
        }
    }

    fn fixture() -> (Memory, PolicyProposal) {
        let m = Memory {
            bytes: RefCell::new(BTreeMap::new()),
            writes: Cell::new(0),
            unreadable: Cell::new(false),
        };
        let s = GameBuild::Fm27Steam.schemas();
        let l = &WINDOWS_STEAM;
        m.put(MODULE + s.rva_clubs_data, &CLUBS.to_le_bytes());
        m.put(MODULE + s.rva_clubs_end, &(CLUBS + 8).to_le_bytes());
        m.put(CLUBS, &CLUB.to_le_bytes());
        m.put(MODULE + s.rva_staff_data, &STAFF.to_le_bytes());
        m.put(MODULE + s.rva_staff_end, &(STAFF + 8).to_le_bytes());
        m.put(STAFF, &MANAGER.to_le_bytes());
        let info_sub = sub(s.club_sub_records, "info").unwrap();
        m.put(CLUB, &vec![0; info_sub.ptr_offset as usize + 8]);
        m.put(CLUB, &(MODULE + l.club_vtable_rva).to_le_bytes());
        m.put(
            CLUB + field(s, "c_uid").unwrap() as u64,
            &17u32.to_le_bytes(),
        );
        m.put(CLUB + info_sub.ptr_offset, &INFO.to_le_bytes());
        m.put(INFO, &vec![0xA5; info_sub.size]);
        m.put(INFO + l.policy_offset as u64, &vec![4; l.policy_len]);
        m.put(MODULE + s.rva_current_date, &[0xC2, 0x1A, 0xEA, 0x07]);
        let cp = sub(s.staff_sub_records, "contract").unwrap();
        let ts = sub(s.staff_sub_records, "team").unwrap();
        let cs = sub(s.staff_sub_records, "club").unwrap();
        let cpo = cp.ptr_offset + s.human_manager_shift as u64;
        m.put(MANAGER, &vec![0; cpo as usize + 8]);
        m.put(
            MANAGER,
            &(MODULE + s.human_manager_vtable_rva).to_le_bytes(),
        );
        let uid = field(s, "s_id").unwrap() + s.human_manager_shift;
        m.put(MANAGER + uid as u64, &29u32.to_le_bytes());
        m.put(MANAGER + cpo, &CONTRACT.to_le_bytes());
        m.put(CONTRACT, &vec![0; cp.size]);
        let owner =
            MANAGER + uid as u64 - (field(s, "p_uid").unwrap() - s.player_person_offset) as u64;
        m.put(
            CONTRACT + l.contract_owner_offset as u64,
            &owner.to_le_bytes(),
        );
        m.put(CONTRACT + ts.ptr_offset, &TEAM.to_le_bytes());
        m.put(TEAM, &vec![0; cs.ptr_offset as usize + 8]);
        m.put(TEAM + cs.ptr_offset, &CLUB.to_le_bytes());
        let p = prepare_with(
            &|a, n| m.read(a, n),
            s,
            l,
            MODULE,
            7,
            11,
            ClubIdentity {
                address: CLUB,
                index: 0,
                uid: 17,
            },
        )
        .unwrap();
        (m, p)
    }

    #[test]
    fn prepare_is_read_only_and_round_trip_preserves_all_other_bytes_and_date_flags() {
        let (m, p) = fixture();
        assert_eq!(m.writes.get(), 0);
        let initial = m.bytes.borrow().clone();
        p.guard
            .write_with(
                &|a, n| m.read(a, n),
                &|a, b| m.write(a, b),
                p.address(),
                &[4],
                &[0],
            )
            .unwrap();
        let changes: Vec<_> = initial
            .iter()
            .filter(|(a, b)| m.bytes.borrow().get(a) != Some(b))
            .map(|(&a, _)| a)
            .collect();
        assert_eq!(changes, vec![p.address()]);
        p.guard
            .write_with(
                &|a, n| m.read(a, n),
                &|a, b| m.write(a, b),
                p.address(),
                &[0],
                &[4],
            )
            .unwrap();
        assert_eq!(*m.bytes.borrow(), initial);
        assert_eq!(m.writes.get(), 2);
    }

    #[test]
    fn eligibility_follows_selected_club_identity_without_a_witness_uid_rule() {
        let s = GameBuild::Fm27Steam.schemas();
        for uid in [1, 42, u32::MAX] {
            let (m, p) = fixture();
            m.put(CLUB + field(s, "c_uid").unwrap() as u64, &uid.to_le_bytes());
            let identity = ClubIdentity { uid, ..p.identity };
            let proposal = prepare_with(
                &|a, n| m.read(a, n),
                s,
                &WINDOWS_STEAM,
                MODULE,
                7,
                11,
                identity,
            )
            .unwrap();
            assert_eq!(proposal.identity, identity);
            assert_eq!(proposal.current_codes, vec![4; 7]);
            assert_eq!(m.writes.get(), 0);
        }
    }

    #[test]
    fn multiple_managers_resolve_the_selected_club_and_bind_only_its_owner() {
        let (m, p) = fixture();
        let s = GameBuild::Fm27Steam.schemas();
        let contract = sub(s.staff_sub_records, "contract").unwrap();
        let team = sub(s.staff_sub_records, "team").unwrap();
        let club = sub(s.staff_sub_records, "club").unwrap();
        let cp = contract.ptr_offset + s.human_manager_shift as u64;
        let uid = (field(s, "s_id").unwrap() + s.human_manager_shift) as u64;
        let person = uid - (field(s, "p_uid").unwrap() - s.player_person_offset) as u64;
        let manager2 = MANAGER + 0x10000;
        let contract2 = CONTRACT + 0x10000;
        let another_team = TEAM + 0x10000;
        m.put(manager2, &m.read(MANAGER, cp as usize + 8).unwrap());
        m.put(contract2, &m.read(CONTRACT, contract.size).unwrap());
        m.put(manager2 + uid, &43u32.to_le_bytes());
        m.put(manager2 + cp, &contract2.to_le_bytes());
        m.put(
            contract2 + WINDOWS_STEAM.contract_owner_offset as u64,
            &(manager2 + person).to_le_bytes(),
        );
        m.put(STAFF + 8, &manager2.to_le_bytes());
        m.put(MODULE + s.rva_staff_end, &(STAFF + 16).to_le_bytes());
        // The first human manages another club; the second owns this club.
        m.put(another_team, &vec![0; club.ptr_offset as usize + 8]);
        m.put(
            another_team + club.ptr_offset,
            &(CLUB + 0x10000).to_le_bytes(),
        );
        m.put(CONTRACT + team.ptr_offset, &another_team.to_le_bytes());
        let proposal = prepare_with(
            &|a, n| m.read(a, n),
            s,
            &WINDOWS_STEAM,
            MODULE,
            7,
            11,
            p.identity,
        )
        .unwrap();
        m.put(MANAGER + uid, &44u32.to_le_bytes());
        assert!(proposal.guard.validate(&|a, n| m.read(a, n), 4).is_ok());
        m.put(manager2 + uid, &45u32.to_le_bytes());
        let error = proposal
            .guard
            .validate(&|a, n| m.read(a, n), 4)
            .unwrap_err();
        assert!(error.contains("human manager UID"));
        assert_eq!(m.writes.get(), 0);
    }

    #[test]
    fn opening_a_negotiation_does_not_invalidate_undo_or_restore_its_session_data() {
        let (m, p) = fixture();
        p.guard
            .write_with(
                &|a, n| m.read(a, n),
                &|a, b| m.write(a, b),
                p.address(),
                &[4],
                &[0],
            )
            .unwrap();
        // The witnessed session vectors are manager-local state, not identity.
        // Changing these fixture bytes models opening an offer after Apply.
        m.put(MANAGER + 0x200, &[0xA7; 48]);
        let before_undo = m.bytes.borrow().clone();
        p.guard
            .write_with(
                &|a, n| m.read(a, n),
                &|a, b| m.write(a, b),
                p.address(),
                &[0],
                &[4],
            )
            .unwrap();
        let changed: Vec<_> = before_undo
            .iter()
            .filter(|(a, b)| m.bytes.borrow().get(a) != Some(b))
            .map(|(&a, _)| a)
            .collect();
        assert_eq!(changed, vec![p.address()]);
        assert_eq!(m.read(MANAGER + 0x200, 48).unwrap(), vec![0xA7; 48]);
        assert_eq!(m.writes.get(), 2);
    }

    #[test]
    fn unknown_and_already_supported_policy_inputs_are_never_coerced() {
        for code in 0..=255u8 {
            let (m, p) = fixture();
            m.put(INFO + WINDOWS_STEAM.policy_offset as u64 + 3, &[code]);
            let result = prepare_with(
                &|a, n| m.read(a, n),
                GameBuild::Fm27Steam.schemas(),
                &WINDOWS_STEAM,
                MODULE,
                7,
                11,
                p.identity,
            );
            assert_eq!(result.is_ok(), code == 4);
            assert_eq!(m.writes.get(), 0);
        }
    }

    #[test]
    fn invalid_game_dates_cannot_authorize_a_preview() {
        let s = GameBuild::Fm27Steam.schemas();
        for invalid in [[0, 0, 0, 0], [0, 0, 0xEA, 0x07], [0x6E, 0x01, 0xEA, 0x07]] {
            let (m, p) = fixture();
            m.put(MODULE + s.rva_current_date, &invalid);
            let error = prepare_with(
                &|a, n| m.read(a, n),
                s,
                &WINDOWS_STEAM,
                MODULE,
                7,
                11,
                p.identity,
            )
            .unwrap_err();
            assert!(error.contains("Invalid current game date"));
            assert_eq!(m.writes.get(), 0);
        }
    }

    #[test]
    fn manager_slot_or_class_changing_during_preparation_cannot_become_a_guard() {
        let s = GameBuild::Fm27Steam.schemas();
        for change_slot in [true, false] {
            let (m, p) = fixture();
            let slot_reads = Cell::new(0);
            let reader = |a, n| {
                if a == STAFF && n == 8 {
                    slot_reads.set(slot_reads.get() + 1);
                    if change_slot && slot_reads.get() == 2 {
                        m.put(STAFF, &(MANAGER + 0x1000).to_le_bytes());
                    }
                }
                if !change_slot && a == MANAGER && n > 8 {
                    m.put(MANAGER, &0x30001u64.to_le_bytes());
                }
                m.read(a, n)
            };
            assert!(prepare_with(&reader, s, &WINDOWS_STEAM, MODULE, 7, 11, p.identity).is_err());
            assert_eq!(m.writes.get(), 0);
        }
    }

    #[test]
    fn stale_owner_identity_pointer_date_policy_and_reload_are_rejected_before_any_write() {
        let s = GameBuild::Fm27Steam.schemas();
        for at in [
            CLUBS,
            CLUB + field(s, "c_uid").unwrap() as u64,
            CLUB + sub(s.club_sub_records, "info").unwrap().ptr_offset,
            CONTRACT + WINDOWS_STEAM.contract_owner_offset as u64,
            TEAM + sub(s.staff_sub_records, "club").unwrap().ptr_offset,
            MODULE + s.rva_current_date,
            MODULE + s.rva_clubs_data,
            INFO + WINDOWS_STEAM.policy_offset as u64 + 1,
            MANAGER,
            MANAGER + (field(s, "s_id").unwrap() + s.human_manager_shift) as u64,
            MANAGER
                + sub(s.staff_sub_records, "contract").unwrap().ptr_offset
                + s.human_manager_shift as u64,
        ] {
            let (m, p) = fixture();
            let old = m.read(at, 1).unwrap()[0];
            m.put(at, &[old ^ 1]);
            let error = p
                .guard
                .write_with(
                    &|a, n| m.read(a, n),
                    &|a, b| m.write(a, b),
                    p.address(),
                    &[4],
                    &[0],
                )
                .unwrap_err();
            assert!(!error.write_attempted);
            assert_eq!(m.writes.get(), 0);
        }
    }

    #[test]
    fn missing_truncated_foreign_classes_and_unmanaged_records_do_not_produce_a_preview() {
        let s = GameBuild::Fm27Steam.schemas();
        for at in [INFO, CLUB, MANAGER, CONTRACT] {
            let (m, p) = fixture();
            m.bytes.borrow_mut().remove(&at);
            assert!(
                prepare_with(
                    &|a, n| m.read(a, n),
                    s,
                    &WINDOWS_STEAM,
                    MODULE,
                    7,
                    11,
                    p.identity
                )
                .is_err()
            );
        }
        let (m, p) = fixture();
        m.put(CLUB, &0x30001u64.to_le_bytes());
        assert!(
            prepare_with(
                &|a, n| m.read(a, n),
                s,
                &WINDOWS_STEAM,
                MODULE,
                7,
                11,
                p.identity
            )
            .is_err()
        );
        let (m, p) = fixture();
        m.put(
            TEAM + sub(s.staff_sub_records, "club").unwrap().ptr_offset,
            &0x30001u64.to_le_bytes(),
        );
        assert!(
            prepare_with(
                &|a, n| m.read(a, n),
                s,
                &WINDOWS_STEAM,
                MODULE,
                7,
                11,
                p.identity
            )
            .is_err()
        );
        let (m, p) = fixture();
        assert!(
            prepare_with(
                &|a, n| m.read(a, n).map(|mut b| {
                    b.pop();
                    b
                }),
                s,
                &WINDOWS_STEAM,
                MODULE,
                7,
                11,
                p.identity
            )
            .is_err()
        );
    }

    #[test]
    fn guard_rejects_other_addresses_widths_and_values() {
        let (m, p) = fixture();
        for (at, old, new) in [
            (p.address() + 1, vec![4], vec![0]),
            (p.address(), vec![4], vec![1]),
            (p.address(), vec![4, 4], vec![0, 0]),
            (p.address(), vec![], vec![]),
        ] {
            assert!(
                p.guard
                    .write_with(&|a, n| m.read(a, n), &|a, b| m.write(a, b), at, &old, &new)
                    .is_err()
            );
        }
        assert_eq!(m.writes.get(), 0);
        assert!(p.guard.session(8, 11).is_err());
        assert!(p.guard.session(7, 12).is_err());
        assert!(p.guard.session(7, 11).is_ok());
        for (base, offset, len) in [
            (0x10000, 0, 1),
            (u64::MAX, 1, 1),
            (0x20000, usize::MAX, 1),
            (0x20000, 0, 0),
        ] {
            assert!(address(base, offset, len).is_err());
        }
    }

    #[test]
    fn policy_layout_never_inherits_another_storefront_platform_or_fingerprint() {
        assert!(layout_for(GameBuild::Fm27Steam, WINDOWS_STEAM.hash, true).is_ok());
        assert!(layout_for(GameBuild::Fm27Steam, 0, true).is_err());
        assert!(layout_for(GameBuild::Fm27Steam, WINDOWS_STEAM.hash, false).is_err());
        assert!(layout_for(GameBuild::Fm27GamePass, WINDOWS_STEAM.hash, true).is_err());
    }

    #[test]
    fn failed_or_unreadable_write_is_not_success_but_verified_api_error_keeps_undo_possible() {
        let (m, p) = fixture();
        let error = p
            .guard
            .write_with(
                &|a, n| m.read(a, n),
                &|_, _| Err("denied".into()),
                p.address(),
                &[4],
                &[0],
            )
            .unwrap_err();
        assert!(error.write_attempted);
        let warning = p
            .guard
            .write_with(
                &|a, n| m.read(a, n),
                &|a, b| {
                    m.write(a, b)?;
                    Err("API error".into())
                },
                p.address(),
                &[4],
                &[0],
            )
            .unwrap();
        assert!(warning.unwrap().contains("readback confirms"));
        p.guard
            .write_with(
                &|a, n| m.read(a, n),
                &|a, b| m.write(a, b),
                p.address(),
                &[0],
                &[4],
            )
            .unwrap();
        let error = p
            .guard
            .write_with(
                &|a, n| m.read(a, n),
                &|a, b| {
                    m.write(a, b)?;
                    m.unreadable.set(true);
                    Ok(())
                },
                p.address(),
                &[4],
                &[0],
            )
            .unwrap_err();
        assert!(error.write_attempted);
        assert!(error.message.contains("not confirmed"));
        assert!(error.message.contains("backup"));
    }

    pub(crate) fn proposal_for_ui() -> PolicyProposal {
        fixture().1
    }
}
