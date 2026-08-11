//! The balancing dataset: the validated shape the core consumes.
//!
//! The **definitions** live here rather than in `sim-data` because of the
//! direction of the dependencies: the `World` holds an `Arc<DataSet>` (A2), and
//! sim-core cannot depend on sim-data. What stays in `sim-data` is the RON
//! parsing, the validation and the I/O — that is, everything the core must not
//! do (D4).

use std::collections::BTreeMap;

use crate::data_hash::dataset_hash;
use crate::grid::Terrain;
use crate::ids::BuildingKindId;
use crate::service::ServiceKind;
use crate::units::{Coins, Milli};

/// Global simulation constants.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rules {
    pub ticks_per_month: u32,
    pub months_per_year: u32,
    pub starting_treasury: Coins,
    /// Indexed by house level (level 1 = index 0).
    pub residents_per_house_level: Vec<u16>,
    pub food_per_resident: Milli,
}

impl Rules {
    /// Ticks in a game year. Scenario objectives are expressed in months and
    /// years, never in ticks (M1).
    pub const fn ticks_per_year(&self) -> u32 {
        self.ticks_per_month * self.months_per_year
    }

    /// The most residents a house can hold at the given level (level 1 = index
    /// 0).
    ///
    /// `None` out of range, like [`ServiceDef::range`] and
    /// [`ServiceDef::capacity`]: a level that does not exist in the table is a
    /// data error, not a panic.
    ///
    /// It is a function and not a direct access to the `Vec` because phase 13
    /// restructures `residents_per_house_level` into a per-level table: there
    /// the **body** changes, not the callers.
    pub fn max_residents(&self, level: u8) -> Option<u16> {
        self.residents_per_house_level
            .get(usize::from(level).checked_sub(1)?)
            .copied()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerrainDef {
    pub buildable: bool,
    pub walkable: bool,
    pub road_cost: Coins,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServiceDef {
    pub kind: ServiceKind,
    /// Range in tiles walked along the road network (D2), one value per level.
    pub range_per_level: Vec<u16>,
    /// **Residents** served at the same time, one value per level.
    ///
    /// Not houses: once houses have levels (M1) the population varies from
    /// house to house, and a capacity counted in houses would no longer say
    /// how many people the provider can really serve.
    pub capacity_per_level: Vec<u16>,
}

impl ServiceDef {
    /// The range at the given level (level 1 = index 0), `None` out of range.
    pub fn range(&self, level: u8) -> Option<u16> {
        self.range_per_level
            .get(usize::from(level).checked_sub(1)?)
            .copied()
    }

    /// The capacity in residents at the given level, `None` out of range.
    pub fn capacity(&self, level: u8) -> Option<u16> {
        self.capacity_per_level
            .get(usize::from(level).checked_sub(1)?)
            .copied()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildingDef {
    pub id: String,
    /// (width, height) in tiles.
    pub size: (u8, u8),
    pub cost: Coins,
    pub levels: u8,
    pub service: Option<ServiceDef>,
    pub required_services: Vec<ServiceKind>,
    pub output_per_tick: Option<Milli>,
    pub max_stock: Option<Milli>,
}

impl BuildingDef {
    /// A building is a house if it requires services instead of providing them.
    /// A structural rule, not a number: it belongs in the code on purpose.
    pub fn is_house(&self) -> bool {
        self.service.is_none() && !self.required_services.is_empty()
    }

    pub fn is_producer(&self) -> bool {
        self.output_per_tick.is_some()
    }

    /// How many tiles it takes up.
    pub const fn tile_count(&self) -> u16 {
        self.size.0 as u16 * self.size.1 as u16
    }
}

/// An index into the profiles table.
///
/// Not an enum: difficulty is data (D6), and a civilisation or a scenario will
/// be able to declare its own without touching the code.
///
/// No `Default`. It looks like an inconvenience and it is not: [`World::new`]
/// has four callers, and a default is exactly the mechanism by which one of the
/// four would be left behind with nothing to flag it.
///
/// It is not `Serialize` either, on purpose: what travels in a replay's header
/// is the **textual** id, never this index. A `Serialize` impl here is the one
/// thing that would make writing the index into a save file look natural (A13).
///
/// [`World::new`]: crate::world::World::new
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct DifficultyId(u8);

impl DifficultyId {
    /// Crate-private: outside `sim-core` an id can only be obtained from
    /// [`DataSet::difficulty_by_id`], so one that resolves to no profile cannot
    /// be built by mistake.
    pub(crate) const fn new(v: u8) -> Self {
        Self(v)
    }

    pub const fn get(self) -> u8 {
        self.0
    }

    pub(crate) const fn as_usize(self) -> usize {
        self.0 as usize
    }
}

/// A difficulty profile: the knobs chosen at the start of a game.
///
/// Born with one field. Phases 13, 14 and 16 add theirs here without touching
/// the replay's header or [`World::new`]'s signature again.
///
/// [`World::new`]: crate::world::World::new
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DifficultyDef {
    pub id: String,
    /// The residents of a newly-built house. Zero is legitimate: the house
    /// fills up by migration (phase 15).
    pub starting_residents_per_house: u16,
}

/// The validated tables, ready for the core.
///
/// It lives behind an `Arc` inside the `World` (A2): loading is I/O and stays
/// outside the core, but the systems need the tables every tick.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataSet {
    pub rules: Rules,
    /// A `BTreeMap` and not a `HashMap`: the iteration order is a contract (D4).
    pub terrain: BTreeMap<Terrain, TerrainDef>,
    /// Indexed by [`BuildingKindId`].
    pub buildings: Vec<BuildingDef>,
    /// Indexed by [`DifficultyId`]. At most 256 of them, which is what makes
    /// the index a `u8`.
    pub difficulties: Vec<DifficultyDef>,
    /// blake3 of the **validated** content, not of the files' bytes:
    /// reformatting a RON file or adding a comment does not change the hash,
    /// changing a number does. It feeds into the state hash (A2).
    /// [`DataSet::new`] computes it.
    pub hash: [u8; 32],
}

impl DataSet {
    /// Builds the dataset and computes its hash. The only way to obtain a
    /// `DataSet`, so the hash cannot fall out of sync with the content.
    pub fn new(
        rules: Rules,
        terrain: BTreeMap<Terrain, TerrainDef>,
        buildings: Vec<BuildingDef>,
        difficulties: Vec<DifficultyDef>,
    ) -> Self {
        let hash = dataset_hash(&rules, &terrain, &buildings, &difficulties);
        Self {
            rules,
            terrain,
            buildings,
            difficulties,
            hash,
        }
    }

    pub fn def(&self, kind: BuildingKindId) -> Option<&BuildingDef> {
        self.buildings.get(kind.as_usize())
    }

    /// Resolves the textual id used in the tables and in the scenarios.
    pub fn kind_by_id(&self, id: &str) -> Option<BuildingKindId> {
        let pos = self.buildings.iter().position(|b| b.id == id)?;
        u16::try_from(pos).ok().map(BuildingKindId::new)
    }

    pub fn difficulty(&self, d: DifficultyId) -> Option<&DifficultyDef> {
        self.difficulties.get(d.as_usize())
    }

    /// Resolves the textual id used in the tables and in the replay headers.
    pub fn difficulty_by_id(&self, id: &str) -> Option<DifficultyId> {
        let pos = self.difficulties.iter().position(|d| d.id == id)?;
        u8::try_from(pos).ok().map(DifficultyId::new)
    }

    /// The known profile ids, for error messages.
    pub fn difficulty_ids(&self) -> String {
        self.difficulties
            .iter()
            .map(|d| d.id.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    }

    pub fn terrain(&self, t: Terrain) -> Option<&TerrainDef> {
        self.terrain.get(&t)
    }

    /// The hash in hexadecimal, for error messages and replay headers.
    pub fn hash_hex(&self) -> String {
        self.hash.iter().map(|b| format!("{b:02x}")).collect()
    }

    /// The food providers that claim more capacity than their output can
    /// sustain.
    ///
    /// **Why this is a check and not a convention in the comments.** If a food
    /// provider can take on more residents than it feeds, the houses in excess
    /// stay assigned to it — the first provider wins a contested house — and
    /// stop eating: hunger becomes an **absorbing** state, one that not even
    /// building a second farm dissolves. With this check green the reverse
    /// implication holds instead, and it is a rule of the game: *a house
    /// covered by the food service always eats*.
    ///
    /// The arithmetic is for the worst case, an empty stock at the start of the
    /// tick: what the provider can hand out is the smaller of one tick's output
    /// and what the granary can hold.
    ///
    /// **It lives exactly as long as A5.** It is the right rule while the farm
    /// produces into its own stock; in M3 the goods will come from a warehouse
    /// via real logistics walkers, capacity will stop depending on local
    /// output, and this check has to go along with the simplification it
    /// guards.
    pub fn unsustainable_food_capacity(&self) -> Vec<UnsustainableCapacity> {
        let mut out = Vec::new();
        let per_resident = self.rules.food_per_resident.to_millis();
        if per_resident <= 0 {
            // Free food: any capacity is sustainable. It is not this check's
            // job to say the dataset makes no sense.
            return out;
        }

        for (building, def) in self.buildings.iter().enumerate() {
            let Some(service) = def.service.as_ref() else {
                continue;
            };
            if service.kind != ServiceKind::Food {
                continue;
            }
            let (Some(output), Some(max_stock)) = (def.output_per_tick, def.max_stock) else {
                continue;
            };

            let available = output.to_millis().min(max_stock.to_millis());
            let sustainable = u16::try_from(available / per_resident).unwrap_or(u16::MAX);
            for (i, &capacity) in service.capacity_per_level.iter().enumerate() {
                if capacity > sustainable {
                    out.push(UnsustainableCapacity {
                        building,
                        level: u8::try_from(i + 1).unwrap_or(u8::MAX),
                        capacity,
                        sustainable,
                    });
                }
            }
        }
        out
    }

    /// The difficulty profiles that would build a house beyond its own
    /// capacity.
    ///
    /// A cross-table check like [`DataSet::unsustainable_food_capacity`], and
    /// it lives here for the same two reasons: it crosses `rules` and
    /// `difficulties`, and `sim-core`'s test fixture does not go through
    /// `sim-data`'s validation and could otherwise drift onto a balancing that
    /// production would reject.
    ///
    /// A house born beyond `max_residents(1)` is a state the rest of the game
    /// cannot represent: from phase 13 on, `residents <= max_residents(level)`
    /// is assumed everywhere.
    pub fn difficulty_beyond_house_capacity(&self) -> Vec<DifficultyBeyondCapacity> {
        // No level 1 in the table is `rules.residents_per_house_level` being
        // empty, which validation reports on its own: saying it twice would be
        // noise.
        let Some(max) = self.rules.max_residents(1) else {
            return Vec::new();
        };
        self.difficulties
            .iter()
            .enumerate()
            .filter(|(_, def)| def.starting_residents_per_house > max)
            .map(|(profile, def)| DifficultyBeyondCapacity {
                profile,
                starting: def.starting_residents_per_house,
                max,
            })
            .collect()
    }
}

/// A food provider whose capacity exceeds what its output sustains.
/// [`DataSet::unsustainable_food_capacity`] finds them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnsustainableCapacity {
    /// Index into [`DataSet::buildings`].
    pub building: usize,
    /// The level at which the capacity overshoots, counting from 1.
    pub level: u8,
    /// Residents claimed in the table.
    pub capacity: u16,
    /// Residents the output really sustains.
    pub sustainable: u16,
}

/// A difficulty profile that would build a house beyond its own capacity.
/// [`DataSet::difficulty_beyond_house_capacity`] finds them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DifficultyBeyondCapacity {
    /// Index into [`DataSet::difficulties`].
    pub profile: usize,
    /// Residents the profile puts in a newly-built house.
    pub starting: u16,
    /// Residents a house can hold at level 1.
    pub max: u16,
}
