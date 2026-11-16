//! Read-only decoding gate for a candidate set of module addresses.

use std::collections::HashMap;

use crate::bridge::entity_load::{load_club_view, read_club_field_u32, resolve_club_sub_ptr};
use crate::fields::Encoding;
use crate::player::{self, is_valid_ptr};
use crate::staff;

use super::FmProcess;

const PLAYER_SAMPLES: usize = 256;
const STAFF_SAMPLES: usize = 128;
const CLUB_SAMPLES: usize = 128;
const ENTITY_PERCENT: u8 = 95;
const CONTRACT_PERCENT: u8 = 90;
const MAX_TRANSFER_BUDGET: u64 = 10_000_000_000;

pub struct GateCheck {
    pub name: &'static str,
    pub sampled: usize,
    pub passed: usize,
    pub required_percent: u8,
    pub ok: bool,
    pub note: Option<String>,
}

pub struct GateReport {
    pub checks: Vec<GateCheck>,
}

impl GateReport {
    pub fn passed(&self) -> bool {
        self.checks.iter().all(|check| check.ok)
    }

    pub fn summary(&self) -> String {
        self.checks
            .iter()
            .map(|check| {
                let percent = if check.sampled == 0 {
                    0
                } else {
                    check.passed * 100 / check.sampled
                };
                let requirement = if check.required_percent == 0 {
                    "need at least one".to_owned()
                } else {
                    format!("need {}%", check.required_percent)
                };
                let mut line = format!(
                    "{} {}/{} ({}%, {})",
                    check.name, check.passed, check.sampled, percent, requirement
                );
                if let Some(note) = &check.note {
                    line.push_str(&format!(" — {note}"));
                }
                line
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    pub fn first_failure(&self) -> Option<&GateCheck> {
        self.checks.iter().find(|check| !check.ok)
    }
}

fn score(name: &'static str, samples: &[bool], required_percent: u8) -> GateCheck {
    let passed = samples.iter().filter(|&&valid| valid).count();
    GateCheck {
        name,
        sampled: samples.len(),
        passed,
        required_percent,
        ok: !samples.is_empty()
            && if required_percent == 0 {
                passed > 0
            } else {
                passed * 100 >= samples.len() * usize::from(required_percent)
            },
        note: None,
    }
}

fn not_applicable(name: &'static str, reason: &str) -> GateCheck {
    GateCheck {
        name,
        sampled: 0,
        passed: 0,
        required_percent: 0,
        ok: true,
        note: Some(format!("not applicable: {reason}")),
    }
}

#[derive(Clone, Default)]
struct PersonSample {
    name: Option<String>,
    uid: Option<u32>,
    dob_year: Option<u16>,
}

#[derive(Clone, Default)]
struct PlayerSample {
    person: PersonSample,
    ca: Option<u16>,
    pa: Option<i16>,
}

fn named(name: &Option<String>) -> bool {
    name.as_ref().is_some_and(|name| !name.trim().is_empty())
}

fn person_validity(samples: &[PersonSample], reference_year: u16) -> Vec<bool> {
    let mut frequencies = HashMap::new();
    for sample in samples {
        if let Some(uid) = sample.uid {
            *frequencies.entry(uid).or_insert(0usize) += 1;
        }
    }
    samples
        .iter()
        .map(|sample| {
            named(&sample.name)
                && sample
                    .uid
                    .is_some_and(|uid| uid != 0 && frequencies[&uid] == 1)
                && sample
                    .dob_year
                    .is_some_and(|year| (1900..=reference_year.saturating_add(1)).contains(&year))
        })
        .collect()
}

fn score_players(samples: &[PlayerSample], reference_year: u16) -> GateCheck {
    let people = samples
        .iter()
        .map(|sample| sample.person.clone())
        .collect::<Vec<_>>();
    let valid = person_validity(&people, reference_year)
        .into_iter()
        .zip(samples)
        .map(|(person, sample)| {
            person
                && sample.ca.is_some_and(|ca| (1..=200).contains(&ca))
                && sample
                    .pa
                    .is_some_and(|pa| (1..=200).contains(&pa) || (-200..=-1).contains(&pa))
        })
        .collect::<Vec<_>>();
    score("players", &valid, ENTITY_PERCENT)
}

fn score_staff(samples: &[PersonSample], reference_year: u16) -> GateCheck {
    score(
        "staff",
        &person_validity(samples, reference_year),
        ENTITY_PERCENT,
    )
}

fn indices(count: usize, limit: usize) -> Vec<usize> {
    let size = count.min(limit);
    (0..size)
        .map(|index| {
            if size == 1 {
                0
            } else {
                (index as u128 * (count - 1) as u128 / (size - 1) as u128) as usize
            }
        })
        .collect()
}

fn range(address: u64, size: usize) -> Result<u64, String> {
    let end = address.checked_add(size.saturating_sub(1) as u64);
    if size == 0 || !is_valid_ptr(address) || !end.is_some_and(is_valid_ptr) {
        return Err("invalid memory range".into());
    }
    Ok(address)
}

fn array(fm: &FmProcess, data_rva: u64, end_rva: u64) -> Result<(u64, u64, usize), String> {
    for rva in [data_rva, end_rva] {
        range(
            fm.gp_base
                .checked_add(rva)
                .ok_or("module address overflow")?,
            8,
        )?;
    }
    fm.read_ptr_array(data_rva, end_rva)
}

fn entry(fm: &FmProcess, data: u64, index: usize, size: usize) -> Result<u64, String> {
    let offset = (index as u64)
        .checked_mul(8)
        .ok_or("array index overflow")?;
    let slot = data.checked_add(offset).ok_or("array address overflow")?;
    let ptr = fm.read_u64(range(slot, 8)?)?;
    range(ptr, size)
}

fn remember_error(note: &mut Option<String>, error: String) {
    if note.is_none() {
        *note = Some(error);
    }
}

fn contract_valid(record: &player::PlayerRecord) -> bool {
    record.contract_addr.is_some_and(is_valid_ptr)
        && record.contract_raw.is_some()
        && record.field_u32("p_contract_wage").is_some()
        && record
            .field_u32("p_contract_expiry")
            .and_then(player::GameDate::from_raw)
            .is_some()
}

/// Validate the attached process's current build (fm.build / fm.addresses()).
pub fn run_gate(fm: &FmProcess) -> GateReport {
    let layout = fm.schemas();
    let addresses = fm.addresses();
    let humans = array(fm, addresses.rva_humans_data, addresses.rva_humans_end);
    let staff_array = array(fm, addresses.rva_staff_data, addresses.rva_staff_end);
    let clubs = array(fm, addresses.rva_clubs_data, addresses.rva_clubs_end);
    let mut player_samples = Vec::new();
    let mut contract_samples = Vec::new();
    let mut player_note = humans.as_ref().err().cloned();
    let mut contract_note = None;
    let contracts_mapped = layout
        .field("p_contract_ptr")
        .and_then(|field| field.offset)
        .is_some();
    if let Ok((data, _, count)) = humans {
        for index in indices(count, PLAYER_SAMPLES) {
            let record = entry(fm, data, index, layout.player_struct_size)
                .and_then(|ptr| player::read_player_at(fm, ptr, index));
            match record {
                Ok(record) => {
                    player_samples.push(PlayerSample {
                        person: PersonSample {
                            name: record.name.clone(),
                            uid: record.field_u32("p_uid"),
                            dob_year: record.field_u32("p_dob").map(|date| (date >> 16) as u16),
                        },
                        ca: record.field_u16("p_ca"),
                        pa: layout.field("p_pa").and_then(|field| {
                            let encoding = field.offset?.encoding();
                            match encoding {
                                Encoding::Raw | Encoding::RawCapped(_) => {
                                    record.field_u16("p_pa").map(|value| value as i16)
                                }
                                _ => None,
                            }
                        }),
                    });
                    if contracts_mapped {
                        match record.field_u64("p_contract_ptr") {
                            Some(0) => {}
                            Some(_) => contract_samples.push(contract_valid(&record)),
                            None => {
                                contract_samples.push(false);
                                remember_error(
                                    &mut contract_note,
                                    "unreadable contract pointer".into(),
                                );
                            }
                        }
                    }
                }
                Err(error) => {
                    player_samples.push(PlayerSample::default());
                    remember_error(&mut player_note, error.clone());
                    if contracts_mapped {
                        contract_samples.push(false);
                        remember_error(&mut contract_note, error);
                    }
                }
            }
        }
    }
    let mut players = score_players(&player_samples, layout.age_reference_year);
    players.note = player_note;
    let mut staff_samples = Vec::new();
    let mut staff_note = staff_array.as_ref().err().cloned();
    if let Ok((data, _, count)) = staff_array {
        for index in indices(count, STAFF_SAMPLES) {
            let record = entry(fm, data, index, layout.staff_struct_size)
                .and_then(|ptr| staff::read_staff_at(fm, ptr, index));
            match record {
                Ok(record) => staff_samples.push(PersonSample {
                    name: staff::read_staff_name(fm, &record.raw),
                    uid: record.field_u32("s_id"),
                    dob_year: record.field_u32("s_dob").map(|date| (date >> 16) as u16),
                }),
                Err(error) => {
                    staff_samples.push(PersonSample::default());
                    remember_error(&mut staff_note, error);
                }
            }
        }
    }
    let mut staff = score_staff(&staff_samples, layout.age_reference_year);
    staff.note = staff_note;
    let mut club_samples = Vec::new();
    let mut club_note = clubs.as_ref().err().cloned();
    if let Ok((data, _, count)) = clubs {
        for index in indices(count, CLUB_SAMPLES) {
            let club = entry(fm, data, index, layout.club_read_size)
                .and_then(|ptr| load_club_view(fm, ptr, index));
            match club {
                Ok(club) => club_samples
                    .push(!club.full_name.trim().is_empty() || !club.short_name.trim().is_empty()),
                Err(error) => {
                    club_samples.push(false);
                    remember_error(&mut club_note, error);
                }
            }
        }
    }
    let mut club_check = score("clubs", &club_samples, ENTITY_PERCENT);
    club_check.note = club_note;
    let finance = if addresses.full_finance_vtable_rva == 0 {
        not_applicable("club finance", "no finance class for this build")
    } else {
        let mut sampled = 0;
        let mut passed = 0;
        let mut note = clubs.as_ref().err().cloned();
        if let Ok((data, _, count)) = clubs {
            for index in 0..count {
                sampled += 1;
                let result = entry(fm, data, index, layout.club_read_size).and_then(|ptr| {
                    let finance = resolve_club_sub_ptr(fm, ptr, "finance")
                        .ok_or("unresolved full finance object")?;
                    let expected = fm
                        .gp_base
                        .checked_add(addresses.full_finance_vtable_rva)
                        .ok_or("vtable overflow")?;
                    let vtable = fm.read_u64(range(finance, 8)?)?;
                    let budget = read_club_field_u32(fm, ptr, "c_transfer_budget")
                        .ok_or("unreadable transfer budget")?;
                    Ok(vtable == expected && u64::from(budget) <= MAX_TRANSFER_BUDGET)
                });
                match result {
                    Ok(true) => {
                        passed = 1;
                        break;
                    }
                    Ok(false) => {}
                    Err(error) => remember_error(&mut note, error),
                }
            }
        }
        GateCheck {
            name: "club finance",
            sampled,
            passed,
            required_percent: 0,
            ok: passed > 0,
            note,
        }
    };
    let contracts = if contracts_mapped {
        let mut check = score("contracts", &contract_samples, CONTRACT_PERCENT);
        check.note = contract_note;
        check
    } else {
        not_applicable("contracts", "no player contract pointer for this build")
    };
    let stadium = if addresses.rva_stadium_base == 0 {
        not_applicable("stadium", "no stadium array root for this build")
    } else {
        not_applicable(
            "stadium",
            "stadium root chain offsets are not mapped in the layout",
        )
    };
    GateReport {
        checks: vec![players, staff, club_check, finance, contracts, stadium],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_players(count: usize) -> Vec<PlayerSample> {
        (1..=count)
            .map(|uid| PlayerSample {
                person: PersonSample {
                    name: Some(format!("Player {uid}")),
                    uid: Some(uid as u32),
                    dob_year: Some(2000),
                },
                ca: Some(100),
                pa: Some(150),
            })
            .collect()
    }

    #[test]
    fn valid_sample_passes() {
        assert!(score_players(&valid_players(PLAYER_SAMPLES), 2026).ok);
    }

    #[test]
    fn garbage_fails() {
        assert!(!score_players(&vec![PlayerSample::default(); PLAYER_SAMPLES], 2026).ok);
    }

    #[test]
    fn staff_decoded_as_players_fails() {
        let mut samples = valid_players(PLAYER_SAMPLES);
        for sample in &mut samples {
            sample.ca = Some(0);
            sample.pa = Some(0);
        }
        assert!(!score_players(&samples, 2026).ok);
    }

    #[test]
    fn duplicate_uids_fail() {
        let mut samples = valid_players(20);
        samples[0].person.uid = samples[1].person.uid;
        let check = score_players(&samples, 2026);
        assert_eq!(check.passed, 18);
        assert!(!check.ok);
        assert!(
            !score_staff(
                &samples
                    .iter()
                    .map(|sample| sample.person.clone())
                    .collect::<Vec<_>>(),
                2026
            )
            .ok
        );
    }

    #[test]
    fn exact_95_percent_boundary() {
        let mut samples = valid_players(20);
        samples[0].ca = None;
        assert!(score_players(&samples, 2026).ok);
        samples[1].ca = None;
        assert!(!score_players(&samples, 2026).ok);
        assert!(score("clubs", &[true; 20], ENTITY_PERCENT).ok);
    }

    #[test]
    fn empty_samples_fail() {
        assert!(!score_players(&[], 2026).ok);
        assert!(!score_staff(&[], 2026).ok);
        assert!(!score("clubs", &[], ENTITY_PERCENT).ok);
        assert!(!score("contracts", &[], CONTRACT_PERCENT).ok);
        assert!(!score("club finance", &[], 0).ok);
    }

    #[test]
    fn not_applicable_checks_pass() {
        let report = GateReport {
            checks: vec![
                not_applicable("stadium", "no chain"),
                not_applicable("contracts", "no pointer"),
            ],
        };
        assert!(report.passed());
        assert!(report.first_failure().is_none());
        assert!(report.summary().contains("not applicable"));
    }

    #[test]
    fn pa_and_dob_boundaries() {
        for pa in [-200, -95, -10, -1, 1, 200] {
            let mut samples = valid_players(1);
            samples[0].pa = Some(pa);
            samples[0].person.dob_year = Some(2027);
            assert!(score_players(&samples, 2026).ok);
        }
        for pa in [-201, 0, 201] {
            let mut samples = valid_players(1);
            samples[0].pa = Some(pa);
            assert!(!score_players(&samples, 2026).ok);
        }
        for year in [1899, 2028] {
            let mut samples = valid_players(1);
            samples[0].person.dob_year = Some(year);
            assert!(!score_players(&samples, 2026).ok);
        }
    }

    #[test]
    fn contract_boundary_and_at_least_one() {
        let mut samples = [true; 10];
        samples[0] = false;
        assert!(score("contracts", &samples, CONTRACT_PERCENT).ok);
        samples[1] = false;
        assert!(!score("contracts", &samples, CONTRACT_PERCENT).ok);
        assert!(score("club finance", &[false, true], 0).ok);
    }

    #[test]
    fn samples_span_array_without_repeats() {
        assert_eq!(indices(0, 256), Vec::<usize>::new());
        assert_eq!(indices(1, 256), vec![0]);
        assert_eq!(indices(4, 256), vec![0, 1, 2, 3]);
        let sample = indices(1000, 256);
        assert_eq!(sample.len(), 256);
        assert_eq!(sample[0], 0);
        assert_eq!(sample[255], 999);
        assert!(sample.windows(2).all(|pair| pair[0] < pair[1]));
    }

    #[test]
    fn reports_first_failure_and_rates() {
        let report = GateReport {
            checks: vec![
                score("players", &vec![true; 256], 95),
                score("staff", &[false], 95),
            ],
        };
        assert!(!report.passed());
        assert_eq!(report.first_failure().unwrap().name, "staff");
        assert!(
            report
                .summary()
                .contains("players 256/256 (100%, need 95%)")
        );
    }

    #[test]
    fn invalid_ranges_fail() {
        assert!(range(0x10000, 8).is_err());
        assert!(range(u64::MAX, 8).is_err());
        assert!(range(0x7FFF_FFFF_FFFE, 8).is_err());
        assert!(range(0x20000, 8).is_ok());
    }
}
