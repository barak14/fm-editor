//! Read-only discovery of module addresses using the attached release's record layouts.

use std::collections::{BTreeMap, BTreeSet};

use crate::bridge::entity_load::{load_club_view, resolve_club_sub_ptr};
use crate::fields::{Category, Encoding, FieldLocation, ModuleAddresses, RecordedBuild};
use crate::game_version::GameBuild;
use crate::player::{self, is_valid_ptr};
use crate::staff;

use super::FmProcess;
use super::layout_validate::{
    GateCheck, PersonSample, PlayerSample, entry, indices, person_validity, range, score,
    score_players, score_staff,
};
use super::rtti;

const CHUNK: u64 = 1024 * 1024;
const PAGE: u64 = 4096;
const SCN_EXECUTE: u32 = 0x2000_0000;
const SCN_WRITE: u32 = 0x8000_0000;
const ARRAY_SAMPLES: usize = 64;
const ARRAY_PERCENT: usize = 90;
const MIN_PLAYERS: usize = 1000;
const CLASS_SAMPLES: usize = 4096;
const DATE_WINDOW: u64 = 0x20000;

/// Result of scanning an attached process for its release's module addresses.
pub struct ScanResult {
    /// Detected addresses. Undetected class RVAs and the date are 0.
    pub addresses: ModuleAddresses,
    /// Recorded build whose relative cluster offsets matched (base layout/labels).
    pub base: &'static RecordedBuild,
    /// Class identifiers used by the release's recorded builds but not found.
    pub missing: Vec<&'static str>,
    /// Detection steps, scores, chosen RVAs and reasons for missing values.
    pub notes: Vec<String>,
}

/// One PE section: name, RVA range and characteristics.
pub struct Section {
    pub name: String,
    pub start: u64,
    pub end: u64,
    pub flags: u32,
}

fn module_size(fm: &FmProcess) -> Result<u64, String> {
    fm.gp_end
        .checked_sub(fm.gp_base)
        .ok_or("invalid module bounds".into())
}

fn module_address(fm: &FmProcess, rva: u64, size: usize) -> Result<u64, String> {
    if rva
        .checked_add(size as u64)
        .ok_or("module range overflow")?
        > module_size(fm)?
    {
        return Err("range outside module".into());
    }
    range(
        fm.gp_base
            .checked_add(rva)
            .ok_or("module address overflow")?,
        size,
    )
}

pub(crate) fn module_read(fm: &FmProcess, rva: u64, size: usize) -> Result<Vec<u8>, String> {
    fm.read_bytes(module_address(fm, rva, size)?, size)
}

fn word<const N: usize>(bytes: &[u8], offset: usize) -> Option<[u8; N]> {
    bytes.get(offset..offset.checked_add(N)?)?.try_into().ok()
}

/// Read the attached module's section table from its PE header.
pub fn sections(fm: &FmProcess) -> Result<Vec<Section>, String> {
    let dos = module_read(fm, 0, 64)?;
    if dos.get(..2) != Some(b"MZ") {
        return Err("no DOS signature at the module base".into());
    }
    let pe = u64::from(u32::from_le_bytes(
        word(&dos, 0x3c).ok_or("short DOS header")?,
    ));
    let header = module_read(fm, pe, 24)?;
    if header.get(..4) != Some(b"PE\0\0") {
        return Err("no PE signature in module header".into());
    }
    let count = usize::from(u16::from_le_bytes(
        word(&header, 6).ok_or("short PE header")?,
    ));
    let optional = u64::from(u16::from_le_bytes(
        word(&header, 20).ok_or("short PE header")?,
    ));
    if count == 0 {
        return Err("empty PE section table".into());
    }
    let table = pe
        .checked_add(24 + optional)
        .ok_or("section table overflow")?;
    let bytes = module_read(
        fm,
        table,
        count.checked_mul(40).ok_or("section table size overflow")?,
    )?;
    bytes
        .chunks_exact(40)
        .map(|section| {
            let name = String::from_utf8_lossy(&section[..8])
                .trim_end_matches('\0')
                .to_owned();
            let size = u64::from(u32::from_le_bytes(
                word(section, 8).ok_or("short PE section")?,
            ));
            let start = u64::from(u32::from_le_bytes(
                word(section, 12).ok_or("short PE section")?,
            ));
            let end = start.checked_add(size).ok_or("section bounds overflow")?;
            let flags = u32::from_le_bytes(word(section, 36).ok_or("short PE section")?);
            Ok(Section {
                name,
                start,
                end,
                flags,
            })
        })
        .collect()
}

fn visit_chunks(fm: &FmProcess, start: u64, end: u64, mut visit: impl FnMut(u64, &[u8])) -> usize {
    let mut skipped = 0;
    let mut rva = start;
    while rva < end {
        let size = (end - rva).min(CHUNK);
        if let Ok(bytes) = module_read(fm, rva, size as usize) {
            visit(rva, &bytes);
        } else {
            let chunk_end = rva + size;
            let mut page = rva;
            while page < chunk_end {
                let size = (PAGE - page % PAGE).min(chunk_end - page);
                match module_read(fm, page, size as usize) {
                    Ok(bytes) => visit(page, &bytes),
                    Err(_) => skipped += 1,
                }
                page += size;
            }
        }
        rva += size;
    }
    skipped
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Array {
    rva: u64,
    data: u64,
    count: usize,
}

impl Array {
    fn from_pair(rva: u64, data: u64, end: u64) -> Option<Self> {
        let size = end.checked_sub(data)?;
        if rva == 0 || rva % 8 != 0 || !is_valid_ptr(data) || !is_valid_ptr(end) || size % 8 != 0 {
            return None;
        }
        let count = size / 8;
        (100..=2_000_000).contains(&count).then_some(Self {
            rva,
            data,
            count: count as usize,
        })
    }
}

fn aligned(rva: u64, alignment: u64) -> u64 {
    rva.saturating_add(rva.wrapping_neg() % alignment)
}

fn pairs(rva: u64, bytes: &[u8]) -> impl Iterator<Item = Array> {
    let start = aligned(rva, 8);
    bytes
        .get((start - rva) as usize..)
        .unwrap_or_default()
        .windows(16)
        .step_by(8)
        .enumerate()
        .filter_map(move |(index, pair)| {
            Array::from_pair(
                start.checked_add(index as u64 * 8)?,
                u64::from_le_bytes(word(pair, 0)?),
                u64::from_le_bytes(word(pair, 8)?),
            )
        })
}

fn vtable_rva(fm: &FmProcess, ptr: u64) -> Option<u64> {
    let vtable = fm.read_u64(range(ptr, 8).ok()?).ok()?;
    let rva = vtable.checked_sub(fm.gp_base)?;
    (is_valid_ptr(vtable) && rva != 0 && vtable < fm.gp_end).then_some(rva)
}

fn prefiltered(fm: &FmProcess, array: Array) -> bool {
    indices(array.count, 5)
        .into_iter()
        .filter(|&index| {
            entry(fm, array.data, index, 8)
                .ok()
                .and_then(|ptr| vtable_rva(fm, ptr))
                .is_some()
        })
        .take(3)
        .count()
        == 3
}

fn find_arrays(
    fm: &FmProcess,
    sections: &[Section],
    notes: &mut Vec<String>,
) -> Result<BTreeMap<u64, Array>, String> {
    let mut candidates = BTreeMap::new();
    let mut pairs_seen = 0;
    let mut skipped = 0;
    let size = module_size(fm)?;
    for section in sections
        .iter()
        .filter(|s| s.flags & (SCN_WRITE | SCN_EXECUTE) == SCN_WRITE)
    {
        let mut tail: Option<(u64, u64)> = None;
        skipped += visit_chunks(
            fm,
            aligned(section.start, 8),
            section.end.min(size),
            |rva, bytes| {
                let mut consider = |array: Array| {
                    pairs_seen += 1;
                    if !candidates.contains_key(&array.rva) && prefiltered(fm, array) {
                        candidates.insert(array.rva, array);
                    }
                };
                // A vector's two words can straddle consecutive successful reads.
                if let Some((previous_rva, data)) = tail
                    && Some(rva) == previous_rva.checked_add(8)
                    && let Some(end) = word(bytes, 0)
                    && let Some(array) =
                        Array::from_pair(previous_rva, data, u64::from_le_bytes(end))
                {
                    consider(array);
                }
                pairs(rva, bytes).for_each(consider);
                tail = bytes.len().checked_sub(8).and_then(|offset| {
                    Some((
                        rva.checked_add(offset as u64)?,
                        u64::from_le_bytes(word(bytes, offset)?),
                    ))
                });
            },
        );
    }
    notes.push(format!("writable sections: {pairs_seen} pointer pairs, {} passed the vtable prefilter, {skipped} unreadable pages", candidates.len()));
    Ok(candidates)
}

fn player_sample(fm: &FmProcess, ptr: u64, index: usize) -> Option<PlayerSample> {
    let record = player::read_player_at(fm, ptr, index).ok()?;
    Some(PlayerSample {
        person: PersonSample {
            uid: record.field_u32("p_uid"),
            dob_year: record.field_u32("p_dob").map(|raw| (raw >> 16) as u16),
            name: record.name.clone(),
        },
        ca: record.field_u16("p_ca"),
        pa: fm
            .schemas()
            .field("p_pa")
            .and_then(|field| match field.offset?.encoding() {
                Encoding::Raw | Encoding::RawCapped(_) => {
                    record.field_u16("p_pa").map(|value| value as i16)
                }
                _ => None,
            }),
    })
}

fn staff_sample(fm: &FmProcess, ptr: u64, index: usize) -> Option<PersonSample> {
    let layout = fm.schemas();
    let size = layout
        .staff_struct_size
        .checked_add(layout.human_manager_shift)?;
    let record = staff::read_staff_at(fm, range(ptr, size).ok()?, index).ok()?;
    Some(PersonSample {
        name: staff::read_staff_name(fm, &record.raw),
        uid: record.field_u32("s_id"),
        dob_year: record.field_u32("s_dob").map(|raw| (raw >> 16) as u16),
    })
}

fn enough(passed: usize, sampled: usize, percent: usize) -> bool {
    sampled != 0 && passed * 100 >= sampled * percent
}

fn passes_players(count: usize, check: &GateCheck) -> bool {
    count >= MIN_PLAYERS && enough(check.passed, check.sampled, ARRAY_PERCENT)
}

fn score_humans(fm: &mut FmProcess, array: Array, notes: &mut Vec<String>) -> bool {
    fm.build.addresses.rva_humans_data = array.rva;
    fm.build.addresses.rva_humans_end = array.rva + 8;
    let samples: Vec<_> = indices(array.count, ARRAY_SAMPLES)
        .into_iter()
        .map(|index| {
            entry(fm, array.data, index, fm.schemas().player_struct_size)
                .ok()
                .and_then(|ptr| player_sample(fm, ptr, index))
                .unwrap_or_default()
        })
        .collect();
    let check = score_players(&samples, fm.schemas().age_reference_year);
    let passed = passes_players(array.count, &check);
    note_score(notes, array, &check, passed);
    passed
}

fn note_score(notes: &mut Vec<String>, array: Array, check: &GateCheck, passed: bool) {
    notes.push(format!(
        "{} gp+0x{:X}: {}/{} valid, {} entries ({})",
        check.name,
        array.rva,
        check.passed,
        check.sampled,
        array.count,
        if passed { "pass" } else { "fail" }
    ));
}

#[derive(Clone, Copy)]
enum Related {
    Staff,
    Clubs,
}

impl Related {
    fn name(self) -> &'static str {
        match self {
            Self::Staff => "staff",
            Self::Clubs => "clubs",
        }
    }

    fn check(self, fm: &FmProcess, array: Array) -> GateCheck {
        let layout = fm.schemas();
        let indices = indices(array.count, ARRAY_SAMPLES);
        match self {
            Self::Staff => {
                let samples: Vec<_> = indices
                    .into_iter()
                    .map(|index| {
                        entry(fm, array.data, index, layout.staff_struct_size)
                            .ok()
                            .and_then(|ptr| staff_sample(fm, ptr, index))
                            .unwrap_or_default()
                    })
                    .collect();
                score_staff(&samples, layout.age_reference_year)
            }
            Self::Clubs => {
                let samples: Vec<_> = indices
                    .into_iter()
                    .map(|index| {
                        entry(fm, array.data, index, layout.club_read_size)
                            .and_then(|ptr| load_club_view(fm, ptr, index))
                            .is_ok_and(|club| {
                                !club.full_name.trim().is_empty()
                                    || !club.short_name.trim().is_empty()
                            })
                    })
                    .collect();
                score("clubs", &samples, ARRAY_PERCENT as u8)
            }
        }
    }
}

struct RelatedScan {
    kind: Related,
    results: BTreeMap<u64, Option<Array>>,
}

impl RelatedScan {
    fn new(kind: Related) -> Self {
        Self {
            kind,
            results: BTreeMap::new(),
        }
    }

    fn check(&mut self, fm: &FmProcess, array: Array, notes: &mut Vec<String>) -> Option<Array> {
        *self.results.entry(array.rva).or_insert_with(|| {
            let check = self.kind.check(fm, array);
            let passed = enough(check.passed, check.sampled, ARRAY_PERCENT);
            note_score(notes, array, &check, passed);
            passed.then_some(array)
        })
    }

    fn derived(
        &mut self,
        fm: &FmProcess,
        rva: Option<u64>,
        notes: &mut Vec<String>,
    ) -> Option<Array> {
        let rva = rva?;
        if let Some(result) = self.results.get(&rva) {
            return *result;
        }
        let array = module_read(fm, rva, 16).ok().and_then(|pair| {
            Array::from_pair(
                rva,
                u64::from_le_bytes(word(&pair, 0)?),
                u64::from_le_bytes(word(&pair, 8)?),
            )
        });
        if let Some(array) = array {
            self.check(fm, array, notes)
        } else {
            notes.push(format!(
                "{} gp+0x{rva:X}: invalid or unreadable derived pointer pair",
                self.kind.name()
            ));
            self.results.insert(rva, None);
            None
        }
    }

    fn resolve(
        &mut self,
        fm: &FmProcess,
        preferred: Option<Array>,
        derived: impl Iterator<Item = Array>,
        candidates: &BTreeMap<u64, Array>,
        humans: Array,
        notes: &mut Vec<String>,
    ) -> Result<Array, String> {
        if let Some(array) = preferred {
            return Ok(array);
        }
        let name = self.kind.name();
        let derived: BTreeMap<_, _> = derived.map(|array| (array.rva, array)).collect();
        match (derived.len(), derived.into_values().next()) {
            (1, Some(array)) => return Ok(array),
            (0, _) => {}
            _ => {
                return Err(format!(
                    "ambiguous {name} array: multiple passing derived RVAs"
                ));
            }
        }
        notes.push(format!(
            "{name}: no passing recorded offset; trying standalone candidates"
        ));
        let mut passing = candidates
            .values()
            .copied()
            .filter(|array| array.rva != humans.rva)
            .filter_map(|array| self.check(fm, array, notes));
        let Some(first) = passing.next() else {
            return Err(format!(
                "{name} array not found: no candidate reached {ARRAY_PERCENT}% valid samples"
            ));
        };
        unique_best(first, passing, |array| array.count, |array| array.rva).ok_or_else(|| {
            format!("ambiguous {name} array: multiple passing RVAs share the largest count")
        })
    }
}

fn relocated(humans_rva: u64, recorded_humans: u64, target: u64) -> Option<u64> {
    if target == 0 || recorded_humans == 0 {
        return None;
    }
    if target >= recorded_humans {
        humans_rva.checked_add(target - recorded_humans)
    } else {
        humans_rva.checked_sub(recorded_humans - target)
    }
    .filter(|&rva| rva != 0)
}

fn adjacent_arrays(addresses: ModuleAddresses) -> bool {
    [
        (addresses.rva_humans_data, addresses.rva_humans_end),
        (addresses.rva_staff_data, addresses.rva_staff_end),
        (addresses.rva_clubs_data, addresses.rva_clubs_end),
    ]
    .into_iter()
    .all(|(data, end)| data != 0 && data.checked_add(8) == Some(end))
}

#[derive(Clone, Copy)]
struct Cluster {
    humans: Array,
    staff: Option<Array>,
    clubs: Option<Array>,
    base: &'static RecordedBuild,
}

fn unique_best<T: Copy, K: Ord>(
    mut best: T,
    rest: impl IntoIterator<Item = T>,
    rank: impl Fn(T) -> K,
    identity: impl Fn(T) -> u64,
) -> Option<T> {
    let mut ambiguous = false;
    for item in rest {
        match rank(item).cmp(&rank(best)) {
            std::cmp::Ordering::Greater => {
                best = item;
                ambiguous = false;
            }
            std::cmp::Ordering::Equal if identity(item) != identity(best) => ambiguous = true,
            _ => {}
        }
    }
    (!ambiguous).then_some(best)
}

fn best_cluster(clusters: &[Cluster]) -> Result<Cluster, String> {
    let Some((&first, rest)) = clusters.split_first() else {
        return Err(format!(
            "players array not found: no candidate with at least {MIN_PLAYERS} entries reached {ARRAY_PERCENT}% valid samples"
        ));
    };
    unique_best(
        first,
        rest.iter().copied(),
        |cluster| {
            (
                cluster.staff.is_some() && cluster.clubs.is_some(),
                cluster.humans.count,
            )
        },
        |cluster| cluster.humans.rva,
    )
    .ok_or_else(|| "ambiguous players array: equally ranked clusters at different RVAs".into())
}

type Census = BTreeMap<u64, Vec<(usize, u64)>>;

fn census(fm: &FmProcess, array: Array, tail: usize) -> Census {
    let mut samples = indices(array.count, CLASS_SAMPLES);
    let tail_start = array.count.saturating_sub(tail);
    samples.retain(|&index| index < tail_start);
    samples.extend(tail_start..array.count);
    let mut classes = Census::new();
    for index in samples {
        if let Ok(ptr) = entry(fm, array.data, index, 8)
            && let Some(rva) = vtable_rva(fm, ptr)
        {
            classes.entry(rva).or_default().push((index, ptr));
        }
    }
    classes
}

fn unique_rva(mut matches: impl Iterator<Item = u64>) -> Result<u64, &'static str> {
    let first = matches.next().ok_or("no qualifying candidate")?;
    if matches.any(|rva| rva != first) {
        return Err("multiple qualifying RVAs");
    }
    Ok(first)
}

#[derive(Clone, Copy)]
pub(crate) enum Class {
    PlayerCoach,
    HumanManager,
    Finance,
}

impl Class {
    pub(crate) const ALL: [Self; 3] = [Self::PlayerCoach, Self::HumanManager, Self::Finance];

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::PlayerCoach => "player-coach class",
            Self::HumanManager => "human-manager class",
            Self::Finance => "finance class",
        }
    }

    pub(crate) fn address(self, addresses: &mut ModuleAddresses) -> &mut u64 {
        match self {
            Self::PlayerCoach => &mut addresses.player_coach_vtable_rva,
            Self::HumanManager => &mut addresses.human_manager_vtable_rva,
            Self::Finance => &mut addresses.full_finance_vtable_rva,
        }
    }

    pub(crate) fn used_by(self, bases: &[&RecordedBuild]) -> bool {
        bases.iter().any(|base| {
            let mut addresses = base.addresses;
            *self.address(&mut addresses) != 0
        })
    }

    pub(crate) fn from_rtti(self, fm: &FmProcess) -> Option<Result<u64, &'static str>> {
        let tables = rtti::index(fm).class(match self {
            Self::PlayerCoach => rtti::PLAYER_COACH,
            Self::HumanManager => rtti::HUMAN_MANAGER,
            Self::Finance => rtti::FINANCE,
        })?;
        if !matches!(self, Self::HumanManager) {
            return Some(unique_rva(
                tables.iter().filter(|v| v.offset == 0).map(|v| v.rva),
            ));
        }

        let addresses = fm.addresses();
        let data = addresses.rva_staff_data;
        let end = addresses.rva_staff_end;
        let entries = module_address(fm, data, 8)
            .and_then(|_| module_address(fm, end, 8))
            .and_then(|_| fm.read_ptr_array(data, end));
        let Ok((data, _, count)) = entries else {
            return Some(Err("staff array unreadable"));
        };
        if count > 2_000_000 {
            return Some(Err("staff array count exceeds scan limit"));
        }
        let tables: Vec<_> = tables
            .iter()
            .filter(|table| table.offset != 0)
            .filter_map(|table| {
                let ptr = fm.gp_base.checked_add(table.rva)?;
                (ptr < fm.gp_end && is_valid_ptr(ptr)).then_some((u64::from(table.offset), ptr))
            })
            .collect();
        let result = human_manager_vbtable(
            (0..count).filter_map(|index| entry(fm, data, index, 8).ok()),
            &tables,
            fm.gp_base..fm.gp_end,
            |address| fm.read_u64(range(address, 8).ok()?).ok(),
        );
        Some(result)
    }
}

fn human_manager_vbtable(
    objects: impl Iterator<Item = u64>,
    tables: &[(u64, u64)],
    module: std::ops::Range<u64>,
    mut read: impl FnMut(u64) -> Option<u64>,
) -> Result<u64, &'static str> {
    // A manager starts with a vbptr; its RTTI identifies the secondary vftables.
    let mut found = None;
    for object in objects {
        if !tables.iter().any(|&(offset, expected)| {
            object.checked_add(offset).and_then(&mut read) == Some(expected)
        }) {
            continue;
        }
        let vbtable = read(object)
            .filter(|&ptr| ptr != module.start && is_valid_ptr(ptr) && module.contains(&ptr))
            .ok_or("RTTI-matched manager has no valid module vbtable")?;
        let rva = vbtable - module.start;
        if found.is_some_and(|previous| rva != previous) {
            return Err("multiple qualifying RVAs");
        }
        found = Some(rva);
    }
    found.ok_or("no qualifying candidate")
}

fn prefer_rtti(
    class: Class,
    rtti: Option<Result<u64, &'static str>>,
    census: Result<u64, &'static str>,
    notes: &mut Vec<String>,
) -> Result<u64, &'static str> {
    let label = class.label();
    match rtti {
        None => census,
        Some(Ok(rva)) => {
            if let Ok(other) = census
                && other != rva
            {
                notes.push(format!(
                    "{label}: MSVC RTTI gp+0x{rva:X} differs from census gp+0x{other:X}; using RTTI"
                ));
            } else {
                notes.push(format!("{label}: identified by MSVC RTTI"));
            }
            Ok(rva)
        }
        Some(Err(reason)) => {
            notes.push(format!(
                "{label}: MSVC RTTI class found, but {reason}; ignoring census"
            ));
            Err(reason)
        }
    }
}

fn identity_class(
    fm: &mut FmProcess,
    array: Array,
    manager: bool,
    notes: &mut Vec<String>,
) -> Result<u64, &'static str> {
    let classes = census(fm, array, if manager { 16 } else { 0 });
    let max_count = classes.values().map(Vec::len).max().unwrap_or(0);
    let label = if manager {
        "human-manager"
    } else {
        "player-coach"
    };
    notes.push(format!(
        "{label} census: {} classes across {} sampled objects with module vtables",
        classes.len(),
        classes.values().map(Vec::len).sum::<usize>()
    ));
    let reference_year = fm.schemas().age_reference_year;
    let set_class = |fm: &mut FmProcess, rva| {
        if manager {
            fm.build.addresses.human_manager_vtable_rva = rva;
        } else {
            fm.build.addresses.player_coach_vtable_rva = rva;
        }
    };
    let valid_identities = |fm: &FmProcess, entries: &[(usize, u64)]| {
        let samples: Vec<_> = entries
            .iter()
            .map(|&(index, ptr)| {
                if manager {
                    staff_sample(fm, ptr, index)
                } else {
                    player_sample(fm, ptr, index).map(|sample| sample.person)
                }
                .unwrap_or_default()
            })
            .collect();
        person_validity(&samples, reference_year)
            .into_iter()
            .filter(|&valid| valid)
            .count()
    };
    let mut matches = Vec::new();
    for (rva, entries) in classes {
        let total = entries.len();
        if !manager && (total == max_count || total < 2) {
            notes.push(format!(
                "{label} gp+0x{rva:X}: {total} samples, dominant or fewer than two"
            ));
            continue;
        }
        let without = valid_identities(fm, &entries);
        let max_without = if manager { 0 } else { total / 10 };
        if without > max_without {
            notes.push(format!(
                "{label} gp+0x{rva:X}: {without}/{total} valid unshifted, exceeds limit"
            ));
            continue;
        }
        set_class(fm, rva);
        let with = valid_identities(fm, &entries);
        set_class(fm, 0);
        notes.push(format!(
            "{label} gp+0x{rva:X}: {with}/{total} valid shifted, {without}/{total} unshifted"
        ));
        if enough(with, total, if manager { 100 } else { 90 }) {
            matches.push(rva);
        }
    }
    unique_rva(matches.into_iter())
}

fn date_shaped(raw: u32, min_year: u16, max_year: u16) -> bool {
    (min_year..=max_year).contains(&((raw >> 16) as u16)) && raw & 0x1ff <= 365
}

fn plausible_money(raw: u32) -> bool {
    let float = f32::from_bits(raw).abs();
    let float_shaped = (0.01..=1_000_000.0).contains(&float) && raw & 0xfff <= 0xf;
    !date_shaped(raw, 1990, 2100) && !float_shaped && (raw as i32).unsigned_abs() <= 2_000_000_000
}

fn qualifying_finance_class(classes: &BTreeMap<u64, (usize, usize)>) -> Result<u64, &'static str> {
    unique_rva(
        classes
            .iter()
            .filter_map(|(&rva, &(passed, sampled))| enough(passed, sampled, 90).then_some(rva)),
    )
}

fn finance_class(
    fm: &FmProcess,
    clubs: Array,
    notes: &mut Vec<String>,
) -> Result<u64, &'static str> {
    let layout = fm.schemas();
    let fields: Vec<_> = layout
        .club_fields
        .iter()
        .filter_map(|field| {
            if field.category != Category::Finance {
                return None;
            }
            match field.offset? {
                FieldLocation::SubRecord {
                    container: "finance",
                    offset,
                    encoding,
                } => Some((offset, encoding)),
                _ => None,
            }
        })
        .collect();
    if fields.is_empty() {
        return Err("no mapped finance money fields");
    }
    let mut classes = BTreeMap::<u64, (usize, usize)>::new();
    for index in indices(clubs.count, CLASS_SAMPLES) {
        let Some((finance, rva)) = entry(fm, clubs.data, index, layout.club_read_size)
            .ok()
            .and_then(|ptr| resolve_club_sub_ptr(fm, ptr, "finance"))
            .and_then(|ptr| Some((ptr, vtable_rva(fm, ptr)?)))
        else {
            continue;
        };
        let valid = fields.iter().all(|&(offset, encoding)| {
            let Some(raw) = finance
                .checked_add(offset)
                .and_then(|ptr| range(ptr, 4).ok())
                .and_then(|ptr| fm.read_u32(ptr).ok())
            else {
                return false;
            };
            match encoding {
                Encoding::Raw | Encoding::RawU32 => plausible_money(raw),
                Encoding::F32Raw => (0.0..=10_000.0).contains(&f32::from_bits(raw)),
                _ => false,
            }
        });
        let (passed, sampled) = classes.entry(rva).or_default();
        *sampled += 1;
        *passed += usize::from(valid);
    }
    notes.push(format!(
        "finance census: {} classes, {} mapped money fields",
        classes.len(),
        fields.len()
    ));
    for (rva, (passed, sampled)) in &classes {
        notes.push(format!(
            "finance gp+0x{rva:X}: {passed}/{sampled} clubs with plausible money fields"
        ));
    }
    let full = qualifying_finance_class(&classes)?;
    let lower_class = full
        .checked_sub(0x4d0)
        .is_some_and(|rva| classes.contains_key(&rva));
    notes.push(format!(
        "finance: another sampled class 0x4D0 below full class: {lower_class}"
    ));
    Ok(full)
}

fn record_class(
    result: Result<u64, &'static str>,
    label: &'static str,
    missing: &mut Vec<&'static str>,
    notes: &mut Vec<String>,
) -> u64 {
    match result {
        Ok(rva) => {
            notes.push(format!("{label}: gp+0x{rva:X}"));
            rva
        }
        Err(reason) => {
            notes.push(format!("{label} not found: {reason}"));
            missing.push(label);
            0
        }
    }
}

fn dates(rva: u64, bytes: &[u8], reference_year: u16) -> impl Iterator<Item = u64> {
    bytes
        .chunks_exact(4)
        .enumerate()
        .filter_map(move |(index, raw)| {
            date_shaped(
                u32::from_le_bytes(raw.try_into().ok()?),
                reference_year.saturating_sub(1),
                reference_year.saturating_add(40),
            )
            .then_some(rva + 4 * index as u64)
        })
}

fn current_date(
    fm: &FmProcess,
    humans_rva: u64,
    bases: &[&RecordedBuild],
    notes: &mut Vec<String>,
) -> u64 {
    let Ok(size) = module_size(fm) else { return 0 };
    let windows: BTreeSet<_> = bases
        .iter()
        .filter_map(|base| {
            let a = base.addresses;
            let center = relocated(humans_rva, a.rva_humans_data, a.rva_current_date)?;
            let start = aligned(center.saturating_sub(DATE_WINDOW).max(4), 4);
            let end = center.saturating_add(DATE_WINDOW + 4).min(size) & !3;
            (start < end).then_some((start, end))
        })
        .collect();
    let mut matches = BTreeSet::new();
    let mut skipped = 0;
    for &(start, end) in &windows {
        skipped += visit_chunks(fm, start, end, |rva, bytes| {
            matches.extend(dates(rva, bytes, fm.schemas().age_reference_year));
        });
    }
    let result = if windows.is_empty() {
        Err("no recorded date window in module")
    } else if skipped != 0 {
        Err("date window contains unreadable pages")
    } else {
        unique_rva(matches.iter().copied())
    };
    match result {
        Ok(rva) => {
            notes.push(format!(
                "current date: gp+0x{rva:X}, unique across {} windows",
                windows.len()
            ));
            rva
        }
        Err(reason) => {
            notes.push(format!(
                "current date not found: {reason} ({} matches across {} windows)",
                matches.len(),
                windows.len()
            ));
            0
        }
    }
}

/// Find the entity arrays, class vtables and current date without writing to the game.
/// The attached build is restored before returning.
pub fn scan(fm: &mut FmProcess) -> Result<ScanResult, String> {
    let original = fm.build;
    fm.build.addresses = ModuleAddresses::default();
    let result = scan_addresses(fm);
    fm.build = original;
    result
}

fn scan_addresses(fm: &mut FmProcess) -> Result<ScanResult, String> {
    let mut notes = Vec::new();
    let bases = GameBuild::candidates(fm.build.version(), fm.build.storefront());
    let sections = sections(fm).map_err(|error| format!("PE section walk failed: {error}"))?;
    let candidates = find_arrays(fm, &sections, &mut notes)?;
    let mut staff_scan = RelatedScan::new(Related::Staff);
    let mut club_scan = RelatedScan::new(Related::Clubs);
    let mut clusters = Vec::new();
    for humans in candidates
        .values()
        .copied()
        .filter(|array| array.count >= MIN_PLAYERS)
    {
        if !score_humans(fm, humans, &mut notes) {
            continue;
        }
        for &base in &bases {
            let a = base.addresses;
            if !adjacent_arrays(a) {
                notes.push(format!(
                    "{}: unsupported non-adjacent array bounds",
                    base.schema_name
                ));
                continue;
            }
            let at = |target| {
                relocated(humans.rva, a.rva_humans_data, target).filter(|&rva| rva != humans.rva)
            };
            let staff = staff_scan.derived(fm, at(a.rva_staff_data), &mut notes);
            let clubs = club_scan.derived(fm, at(a.rva_clubs_data), &mut notes);
            clusters.push(Cluster {
                humans,
                staff,
                clubs,
                base,
            });
        }
    }
    let cluster = best_cluster(&clusters).map_err(|error| {
        format!(
            "{error}; {} prefiltered candidates, {} passing clusters",
            candidates.len(),
            clusters.len()
        )
    })?;
    let humans = cluster.humans;
    let same_humans = || clusters.iter().filter(|row| row.humans.rva == humans.rva);
    let staff = staff_scan.resolve(
        fm,
        cluster.staff,
        same_humans().filter_map(|row| row.staff),
        &candidates,
        humans,
        &mut notes,
    )?;
    let clubs = club_scan.resolve(
        fm,
        cluster.clubs,
        same_humans().filter_map(|row| row.clubs),
        &candidates,
        humans,
        &mut notes,
    )?;
    if staff.rva == clubs.rva {
        return Err("ambiguous staff and clubs arrays: both selected the same RVA".into());
    }
    notes.push(format!(
        "cluster ({}): players gp+0x{:X}, staff gp+0x{:X}, clubs gp+0x{:X}",
        cluster.base.schema_name, humans.rva, staff.rva, clubs.rva
    ));
    let recorded = cluster.base.addresses;
    let stadium = relocated(
        humans.rva,
        recorded.rva_humans_data,
        recorded.rva_stadium_base,
    )
    .filter(|&rva| module_address(fm, rva, 8).is_ok());
    notes.push(match stadium {
        Some(rva) => format!("stadium derived, unchecked: gp+0x{rva:X}"),
        None => "stadium not derived from recorded offsets".into(),
    });
    let mut addresses = ModuleAddresses {
        rva_humans_data: humans.rva,
        rva_humans_end: humans.rva + 8,
        rva_staff_data: staff.rva,
        rva_staff_end: staff.rva + 8,
        rva_clubs_data: clubs.rva,
        rva_clubs_end: clubs.rva + 8,
        rva_stadium_base: stadium.unwrap_or(0),
        ..ModuleAddresses::default()
    };
    fm.build.addresses = addresses;
    let mut missing = Vec::new();
    for class in Class::ALL.into_iter().filter(|class| class.used_by(&bases)) {
        let from_rtti = class.from_rtti(fm);
        let from_census = match class {
            Class::PlayerCoach => identity_class(fm, humans, false, &mut notes),
            Class::HumanManager => identity_class(fm, staff, true, &mut notes),
            Class::Finance => finance_class(fm, clubs, &mut notes),
        };
        *class.address(&mut addresses) = record_class(
            prefer_rtti(class, from_rtti, from_census, &mut notes),
            class.label(),
            &mut missing,
            &mut notes,
        );
    }
    addresses.rva_current_date = current_date(fm, humans.rva, &bases, &mut notes);
    Ok(ScanResult {
        addresses,
        base: cluster.base,
        missing,
        notes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn human_manager_uses_consistent_primary_vbtable_from_secondary_rtti_matches() {
        let base = 0x20000;
        let managers = [0x40000, 0x42000];
        let decoy = 0x44000;
        let secondary = [(0x440, base + 0x100), (0x468, base + 0x200)];
        let mut words = BTreeMap::from([
            (managers[0], base + 0x14),
            (managers[0] + secondary[0].0, secondary[0].1),
            (managers[1], base + 0x14),
            (managers[1] + secondary[1].0, secondary[1].1),
            (decoy, base + 0x24),
            (decoy + secondary[0].0, secondary[1].1),
        ]);
        let resolve = |words: &BTreeMap<_, _>| {
            human_manager_vbtable(
                managers.into_iter().chain([decoy, u64::MAX]),
                &secondary,
                base..base + 0x1000,
                |address| words.get(&address).copied(),
            )
        };
        assert_eq!(resolve(&words), Ok(0x14));
        words.insert(managers[1], base + 0x24);
        assert_eq!(resolve(&words), Err("multiple qualifying RVAs"));
        words.insert(managers[1], base + 0x1000);
        assert_eq!(
            resolve(&words),
            Err("RTTI-matched manager has no valid module vbtable")
        );
        words.clear();
        assert_eq!(resolve(&words), Err("no qualifying candidate"));
    }

    #[test]
    fn rtti_takes_precedence_over_disagreeing_census() {
        let mut notes = Vec::new();
        let class = Class::HumanManager;
        assert_eq!(
            prefer_rtti(class, Some(Ok(0x100)), Ok(0x200), &mut notes),
            Ok(0x100)
        );
        assert!(notes[0].contains("differs from census gp+0x200"));
        assert!(
            prefer_rtti(
                class,
                Some(Err("multiple qualifying RVAs")),
                Ok(0x200),
                &mut notes
            )
            .is_err()
        );
        assert_eq!(prefer_rtti(class, None, Ok(0x200), &mut notes), Ok(0x200));
    }

    fn pair_bytes(data: u64, end: u64) -> [u8; 16] {
        let mut bytes = [0; 16];
        bytes[..8].copy_from_slice(&data.to_le_bytes());
        bytes[8..].copy_from_slice(&end.to_le_bytes());
        bytes
    }

    #[test]
    fn pointer_pairs_require_aligned_statics_valid_pointers_and_bounded_count() {
        let data = 0x20000;
        let valid = pair_bytes(data, data + 800);
        assert_eq!(
            pairs(0x1000, &valid).next(),
            Some(Array {
                rva: 0x1000,
                data,
                count: 100
            })
        );
        assert_eq!(pairs(0x1001, &valid).count(), 0);
        let mut padded = vec![0; 7];
        padded.extend(valid);
        assert_eq!(pairs(0x1001, &padded).next().unwrap().rva, 0x1008);
        assert_eq!(
            pairs(0x1000, &pair_bytes(data, data + 8 * 2_000_000)).count(),
            1
        );
        for (start, end) in [
            (data, data - 8),
            (data, data),
            (data, data + 801),
            (data, data + 792),
            (data, data + 8 * 2_000_001),
            (0x10000, 0x10000 + 800),
            (0x7fff_ffff_ffff - 800, 0x7fff_ffff_ffff),
        ] {
            assert_eq!(
                pairs(0x1000, &pair_bytes(start, end)).count(),
                0,
                "{start:X}..{end:X}"
            );
        }
    }

    #[test]
    fn recorded_arrays_have_adjacent_bounds() {
        for build in GameBuild::ALL {
            assert!(adjacent_arrays(build.addresses), "{}", build.schema_name());
        }
    }

    fn cluster(rva: u64, count: usize, complete: bool) -> Cluster {
        let humans = Array {
            rva,
            count,
            data: 0x20000,
        };
        Cluster {
            humans,
            staff: complete.then_some(Array {
                rva: rva + 0x40,
                ..humans
            }),
            clubs: complete.then_some(Array {
                rva: rva + 0x100,
                ..humans
            }),
            base: GameBuild::FM27_STEAM.recorded,
        }
    }

    #[test]
    fn cluster_rank_prefers_both_related_arrays_then_largest_players_count() {
        let incomplete = cluster(0x1000, 5000, false);
        let smaller = cluster(0x2000, 1000, true);
        let largest = cluster(0x3000, 2000, true);
        let same_humans = Cluster {
            base: GameBuild::FM27_GAMEPASS.recorded,
            ..largest
        };
        let best = best_cluster(&[incomplete, smaller, largest, same_humans]).unwrap();
        assert_eq!(best.humans, largest.humans);
        assert!(std::ptr::eq(best.base, largest.base));
        let tied = cluster(0x4000, 2000, true);
        assert_eq!(
            best_cluster(&[largest, same_humans, tied]).err().unwrap(),
            "ambiguous players array: equally ranked clusters at different RVAs"
        );
    }

    #[test]
    fn player_threshold_accepts_ninety_percent_but_rejects_staff_and_small_arrays() {
        let people: Vec<_> = (1..=10)
            .map(|uid| PersonSample {
                name: Some(format!("Person {uid}")),
                uid: Some(uid),
                dob_year: Some(1990),
            })
            .collect();
        let staff: Vec<_> = people
            .iter()
            .cloned()
            .map(|person| PlayerSample {
                person,
                ..PlayerSample::default()
            })
            .collect();
        assert!(score_staff(&people, 2026).ok);
        assert!(!passes_players(1000, &score_players(&staff, 2026)));
        let players: Vec<_> = people
            .into_iter()
            .enumerate()
            .map(|(index, person)| PlayerSample {
                person,
                ca: (index != 0).then_some(100),
                pa: Some(-95),
            })
            .collect();
        let score = score_players(&players, 2026);
        assert!(passes_players(1000, &score));
        assert!(!passes_players(999, &score));
    }

    #[test]
    fn class_census_requires_one_qualifying_class() {
        let mut classes = BTreeMap::from([(0x1000, (89, 100))]);
        assert_eq!(
            qualifying_finance_class(&classes),
            Err("no qualifying candidate")
        );
        classes.insert(0x2000, (9, 10));
        assert_eq!(qualifying_finance_class(&classes), Ok(0x2000));
        classes.insert(0x3000, (10, 10));
        let mut missing = Vec::new();
        let mut notes = Vec::new();
        assert_eq!(
            record_class(
                qualifying_finance_class(&classes),
                "finance class",
                &mut missing,
                &mut notes
            ),
            0
        );
        assert_eq!(missing, ["finance class"]);
        assert!(notes[0].contains("multiple qualifying RVAs"));
    }

    #[test]
    fn date_selection_requires_one_distinct_rva() {
        let encode = |year: u32, low: u32| ((year << 16) | low).to_le_bytes();
        let valid = encode(2025, 0x1200 | 365);
        let invalid_day = encode(2026, 366);
        let invalid_year = encode(2067, 366);
        assert_eq!(
            unique_rva(dates(0x1000, &valid, 2026).chain(dates(0x1000, &valid, 2026))),
            Ok(0x1000)
        );
        assert_eq!(
            unique_rva(dates(0x1000, &[valid, encode(2066, 0)].concat(), 2026)),
            Err("multiple qualifying RVAs")
        );
        assert_eq!(
            unique_rva(dates(
                0x1000,
                &[invalid_day, invalid_year, encode(2024, 0)].concat(),
                2026
            )),
            Err("no qualifying candidate")
        );
    }

    #[test]
    fn money_rejects_packed_dates_floats_and_outsize_integers() {
        for raw in [
            ((2026 << 16) | 0x1ac2),
            1.25_f32.to_bits(),
            (-65.0_f32).to_bits(),
            i32::MAX as u32,
        ] {
            assert!(!plausible_money(raw), "0x{raw:X}");
        }
        for raw in [0, 25_900_000, 478_500_000, -400_000_i32 as u32] {
            assert!(plausible_money(raw), "0x{raw:X}");
        }
    }
}
