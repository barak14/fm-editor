//! Stadium entity field definitions for FM24. Aggregated via
//! `fields::fm24::STADIUM_FIELDS`.
//!
//! Flat inline array, stride 0xB0, reached from a club through the
//! `home_stadium` sub-record chain.
//!
//! Evidence: Allianz-Arena (UID 8700726, capacity 75000, owner -> FC Bayern)
//! and Tottenham Hotspur Stadium (UID 28096287, capacity 62850, expansion
//! 78562 matching the FM26 witness, owner -> Tottenham), both 105m x 68m.

use crate::fields::{fm_string, pointer, raw, raw_u32, Field};
use crate::fields::Category::*;
use crate::fields::Entity::Stadium;

pub(crate) const FIELDS: &[Field] = &[
    Field { key: "st_uid",          label: "UID",          entity: Stadium, category: General,  offset: raw_u32(0x0C) },
    Field { key: "st_name",         label: "Name",         entity: Stadium, category: General,  offset: fm_string(0x38) },
    Field { key: "st_name2",        label: "Name 2",       entity: Stadium, category: General,  offset: fm_string(0x40) },
    Field { key: "st_capacity",     label: "Capacity",     entity: Stadium, category: General,  offset: raw_u32(0x6C) },
    Field { key: "st_seated",       label: "Seated",       entity: Stadium, category: General,  offset: raw_u32(0x70) },
    Field { key: "st_used",         label: "Used",         entity: Stadium, category: General,  offset: raw_u32(0x74) },
    Field { key: "st_all_seater",   label: "All-Seater",   entity: Stadium, category: General,  offset: raw_u32(0x78) },
    Field { key: "st_expansion",    label: "Expansion",    entity: Stadium, category: General,  offset: raw_u32(0x7C) },
    Field { key: "st_club_ptr",     label: "Club Owner",   entity: Stadium, category: Internal, offset: pointer(0x80) },
    // Values are x10: 1050 = 105m.
    Field { key: "st_pitch_length", label: "Pitch Length", entity: Stadium, category: General,  offset: raw(0x88) },
    Field { key: "st_pitch_width",  label: "Pitch Width",  entity: Stadium, category: General,  offset: raw(0x8A) },
    Field { key: "st_standing",     label: "Standing",     entity: Stadium, category: General,  offset: None },
    Field { key: "st_environment",  label: "Environment",  entity: Stadium, category: General,  offset: None },
    Field { key: "st_condition",    label: "Condition",    entity: Stadium, category: General,  offset: None },
];
