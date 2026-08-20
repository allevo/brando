//! A benchmark for the cost of the tick.
//!
//! It answers one question only: **where does the time in `step()` go**, and in
//! particular how much of it goes into applying the commands (step 1) versus
//! the recomputations that follow (steps 2 and 3).
//!
//! The distinction matters because the two costs scale differently. The cost of
//! a command is O(size) and does not grow with the city; recomputing the
//! coverage is proportional to the number of providers and is paid **once per
//! tick**, not once per command. Measures `C` and `E` below exist to make that
//! difference visible, not to produce a single number.
//!
//! ## How to use it
//!
//! ```text
//! cargo run --release -p xtask -- bench
//! ```
//!
//! Always in `--release`: in debug the numbers say nothing useful.
//!
//! Two knobs move the city rather than the measuring. `--places` says how many
//! places to build per hundred residents — `--places 70` for a city short of
//! them, `--places well=70,farm=150` to move one kind and leave another where it
//! is — and `--tables <dir>` runs the whole thing against an edited copy of the
//! balancing tables, validated exactly as the production ones are.
//!
//! `--houses 1=2000,2=800,3=200` founds the city at the levels you name instead
//! of standing every house at the first one, and a house founded at a level is
//! founded full, holding that level's capacity. It moves an axis `--places`
//! cannot reach: a well's places cover twice as many small houses as large ones,
//! and only the levels above the first require food at all.
//!
//! `--seed <n>` throws the perfect street grid away for a ragged one: cross
//! streets missing, dead ends, blocks of uneven size, and the buildings
//! scattered over what is left instead of spread evenly across it. The lattice
//! roads 45% of the city's footprint in one piece with no dead ends at all, and
//! that flatters anything that skips road tiles — which is what the coverage
//! does for a living.
//!
//! All four are reported in the header, and every reference figure below was
//! measured at the default — 100 places, no histogram, the lattice, the
//! production tables.
//!
//! There is no failure threshold: absolute times depend on the machine. The way
//! to use it is to run it **before and after** a change on the same machine and
//! compare. What has to stay stable in absolute terms is the state `hash`: if it
//! moves without you having changed the game rules or the tables, the
//! optimisation has changed the semantics and it is a bug, not a win.
//!
//! ## Which city it measures
//!
//! A synthetic city on a lattice, using the **real tables** from `sim-data`:
//! the shares of houses and providers come from the capacity and output in the
//! tables, never from constants written here (D6). Change the balancing and the
//! load changes: that is why the header prints the dataset hash, so a measure
//! that shifts can be attributed.
//!
//! `--places` scales the count the capacity asks for, and **only** that count: a
//! producer is also bounded below by what its output sustains, and below that
//! bound a covered house would go hungry. So a kind can sit on its floor and not
//! move at all, and the header names the bound each kind is held by rather than
//! only the count — otherwise a run at half the places would look like a city
//! thinned all through when half of it was never touched.
//!
//! The header also says how well the city it built is served: the share of
//! residents each service reaches, and how many providers have no places left.
//! That last fraction is what decides what a provider's stop is worth, so a
//! saved benchmark output says which point of the axis it came from instead of
//! leaving it to be guessed.
//!
//! The only parameter forced by hand is the starting treasury, raised to a huge
//! value to take the economy out of the picture: what is measured here is the
//! cost of the tick, not whether the city can be afforded.

use std::collections::BTreeMap;
use std::sync::Arc;

use sim_core::{
    BuildingId, BuildingKindId, Coins, Command, DataSet, DifficultyId, Grid, Level, ServiceKind,
    Terrain, TilePos, World, step,
};

/// The profile the benchmark measures on.
///
/// Chosen **explicitly**, never taken as the first in the table or as a
/// default: if rebalancing the default moved these numbers, the tripwire would
/// stop being comparable with the reference figures below. `easy` and not
/// another because it fills a new house up to its level-1 capacity, which is the
/// population the recorded load was measured on; at `hard` the synthetic city
/// would have zero residents and would be measuring something else.
///
/// **The reference figures**, measured at the end of M0 at 200×200 with 15,000
/// residents, *after* the step-3 optimisation that gave 6.5×:
///
/// | | |
/// |---|---|
/// | `A` empty tick, nothing dirty | 248 µs |
/// | `C` tick, 10,000 rejected commands | 274 µs — 2 ns/command |
/// | `D` tick, 1 accepted command | 3.35 ms |
/// | `G` `Coverage::from_scratch` alone | 3.05 ms — 2.5 µs/provider |
///
/// `A` is the number to watch: it is paid on **every** tick, while `D` is paid
/// only when the player does something. Phase 14 then moved `A` to 3.600 ms by
/// making the coverage recompute almost every tick, which is the whole
/// subject of the open question at slot 19.5.
///
/// **Where they stand now**, same scale, after phase 14.9.9 stopped a provider
/// walking once its capacity has run out:
///
/// | | |
/// |---|---|
/// | `A` empty tick, nothing dirty | 2.059 ms |
/// | `D` tick, 1 accepted command | 1.955 ms |
/// | `G` `Coverage::from_scratch` alone | 1.601 ms — 1.3 µs/provider |
///
/// The table above it stays as the end-of-M0 baseline and is not rewritten:
/// what these two say side by side is that the whole of phase 14's 13× is not
/// yet paid back, and where the rest of it has to come from is still slot 19.5.
///
/// **`A` and not `G` is the honest measure of a change to the rules.** `G` runs
/// after the several hundred ticks the measures above it take, so two builds
/// that decide anything differently are timed on cities that have drifted
/// apart, and the difference between them is partly the algorithm and partly a
/// different city. `A` starts from the state whose hash is printed. Phase 14.9.9
/// found this the hard way: measured on `G`, its rule change looked 5.3% slower;
/// measured on `A`, it costs exactly nothing.
const BENCH_DIFFICULTY: &str = "easy";

/// The two sizes measured by default. They are not balancing numbers: they are
/// the project's reference scale (200x200, ~15,000 residents) and an
/// intermediate size that shows how the costs scale.
const PRESETS: [(&str, u16, u32); 2] = [("mid game", 100, 3_000), ("late game", 200, 15_000)];

pub fn bench(args: &[String]) -> Result<(), String> {
    let reps = super::number(args, "--reps")?.unwrap_or(40).max(1);
    let side = super::number(args, "--side")?;
    let residents = super::number(args, "--residents")?;
    let zero = args.iter().any(|a| a == "--zero-demographics");
    let places = Places::parse(super::flag(args, "--places").as_deref())?;
    let seed = super::number(args, "--seed")?.map(u64::from);
    let houses = Houses::parse(super::flag(args, "--houses").as_deref())?;
    if !houses.is_empty() && residents.is_some() {
        return Err("--houses says how many houses to found at each level and \
                    --residents how many residents to settle at the profile's \
                    occupancy: they are two spellings of the same thing, so give \
                    one or the other"
            .to_string());
    }

    // The tables are a directory and not a flag per field. Every number the game
    // has is already in RON, `load_from_dir` already runs the whole validation
    // over it, and a flag per field would be a second spelling of the tables
    // that nothing keeps in agreement with them.
    let tables = super::flag(args, "--tables");
    let dir = tables
        .as_ref()
        .map_or_else(sim_data::production_data_dir, std::path::PathBuf::from);
    let real = sim_data::load_from_dir(&dir).map_err(|e| format!("tables: {e}"))?;
    places.check_against(&real)?;

    println!(
        "dataset {}, difficulty '{BENCH_DIFFICULTY}'",
        &real.hash_hex()[..16]
    );
    if let Some(dir) = &tables {
        println!("tables from {dir}");
    }
    println!(
        "{} repetitions per measure — compare two runs on the same machine",
        reps
    );
    if zero {
        println!(
            "demographics switched off: A is H, the tick that pays for step 6 \
             but not for the recomputation it triggers"
        );
    }

    // With just one of --side, --residents and --houses you measure that profile
    // and nothing else; with none of them, the two reference profiles are
    // measured.
    let knobs = Knobs {
        reps,
        zero,
        places,
        houses,
        seed,
    };
    if side.is_none() && residents.is_none() && knobs.houses.is_empty() && knobs.seed.is_none() {
        for (name, s, r) in PRESETS {
            preset(&real, name, s, r, &knobs)?;
        }
    } else {
        preset(
            &real,
            "custom",
            side.unwrap_or(200).try_into().unwrap_or(u16::MAX),
            residents.unwrap_or(15_000),
            &knobs,
        )?;
    }
    Ok(())
}

fn preset(
    real: &DataSet,
    name: &str,
    side: u16,
    residents: u32,
    knobs: &Knobs,
) -> Result<(), String> {
    let reps = knobs.reps;
    let data = Arc::new(if knobs.zero {
        without_demographics(real)
    } else {
        with_unlimited_treasury(real)
    });
    let difficulty = data
        .difficulty_by_id(BENCH_DIFFICULTY)
        .ok_or_else(|| format!("the dataset has no '{BENCH_DIFFICULTY}' profile"))?;
    let layout = Layout::new(
        &data,
        difficulty,
        side,
        residents,
        &knobs.places,
        &knobs.houses,
        knobs.seed,
    )?;
    println!(
        "\n=== {name}: {side}x{side}, {} residents ===",
        layout.population
    );
    let mut w = build(&data, &layout, side, difficulty)?;

    // The share is of the **footprint**, the square the city actually covers, and
    // not of the map: a city that does not fill its map is the normal case, so
    // the share of the map would say more about `--side` than about the streets.
    let footprint = (layout.blocks_per_side * 4 + 1).pow(2);
    println!(
        "  {} houses ({} res.), {} providers, {} road tiles out of {} tiles ({}% of the footprint)",
        w.house_count(),
        w.population(),
        w.building_count(),
        layout.road_count,
        u32::from(side) * u32::from(side),
        layout.road_count * 100 / footprint.max(1),
    );
    match knobs.seed {
        None => println!("  map: the pitch-4 lattice"),
        Some(seed) => println!("  map: seeded {seed}, {STREETS_KEPT}% of the cross streets kept"),
    }
    println!("  houses per level: {}", super::level_distribution(&w));
    for provider in layout.small.iter().chain(&layout.large) {
        println!(
            "  {} {} at {} places per hundred residents, bound by {}",
            data.def(provider.kind).map_or("?", |d| d.id.as_str()),
            provider.how_many,
            provider.per_hundred,
            provider.bound.describe(),
        );
    }
    println!("  covered: {}", covered(&w));
    let (full, providers) = filled(&w);
    println!(
        "  full: {full} of {providers} providers ({}%)",
        full * 100 / providers.max(1)
    );
    println!(
        "  state hash: {}",
        &sim_replay::hash_hex(&sim_replay::hash_world(&w))[..16]
    );
    println!();
    println!("  {:<40} {:>12} {:>12}", "", "median", "worst");

    // A. An empty tick with nothing dirty: this is what you pay in the ticks
    //    where the player does not build, i.e. the vast majority.
    //
    //    The **count of recomputations** across it is printed alongside,
    //    because the number on its own cannot be diagnosed: an `A` costing
    //    milliseconds with zero recomputations is a completely different fault
    //    from one costing the same with a recomputation on every tick. It is
    //    also how `J` — the fraction of ticks in which the population moved —
    //    is read off directly instead of estimated (slot 19.5).
    let ticks_a = reps * 5;
    let recomputes_before = w.coverage().recomputes();
    let a = measure(ticks_a, |_| {
        step(&mut w, &[]);
    });
    let recomputed = w.coverage().recomputes() - recomputes_before;
    row(
        "A. empty tick, nothing dirty",
        &a,
        Some(format!(
            "{recomputed} recomputes in {} ticks (J = {}%)",
            a.ticks,
            recomputed * 100 / a.ticks.max(1)
        )),
    );

    // B. One rejected command: the full validation path, no invalidation and so
    //    no recomputation.
    let occupied = Command::PlaceRoad {
        at: layout.any_road,
    };
    let b = measure(reps * 5, |_| {
        let r = step(&mut w, &[occupied]);
        debug_assert_eq!(r.rejected.len(), 1);
    });
    row("B. tick, 1 rejected command", &b, None);

    // C. Many rejected commands in the same tick. It isolates the pure marginal
    //    cost of applying a command: (C - B) / (REJECT_BATCH - 1).
    //    The batch is deliberately absurd because the per-command cost is so
    //    small that below a thousand it would vanish into the noise of the rest
    //    of the tick; it is not a game scenario, it is a measuring instrument.
    let many: Vec<Command> = vec![occupied; REJECT_BATCH];
    let c = measure(reps, |_| {
        let r = step(&mut w, &many);
        debug_assert_eq!(r.rejected.len(), REJECT_BATCH);
    });
    let per_command = c.median.saturating_sub(b.median) / (REJECT_BATCH as u128 - 1);
    row(
        &format!("C. tick, {REJECT_BATCH} rejected commands"),
        &c,
        Some(format!("{per_command} ns/command")),
    );

    // D. One accepted command. The real worst case: today any construction or
    //    demolition invalidates the coverage of *every* provider, so what you
    //    read here is the full cost of step 3.
    //    It alternates building and demolishing on the same tile: after each
    //    pair the state is back where it was.
    let slot = layout.free_slots[0];
    let build_one = Command::PlaceBuilding {
        kind: layout.house,
        origin: slot,
    };
    let demolish_one = Command::Demolish { at: slot };
    let d = measure(reps, |i| {
        let cmd = if i % 2 == 0 { build_one } else { demolish_one };
        let r = step(&mut w, &[cmd]);
        assert!(r.rejected.is_empty(), "command rejected: {:?}", r.rejected);
    });
    row("D. tick, 1 accepted command", &d, None);

    // E. The same commands, but all in a single tick. If the cost were per
    //    command this would be N times D; it is barely more than 1x, and it is
    //    the measure that says applying a batch synchronously is not the
    //    bottleneck.
    let batch: Vec<TilePos> = layout.free_slots[1..].to_vec();
    let n = batch.len();
    let build_batch: Vec<Command> = batch
        .iter()
        .map(|p| Command::PlaceBuilding {
            kind: layout.house,
            origin: *p,
        })
        .collect();
    let demolish_batch: Vec<Command> = batch.iter().map(|p| Command::Demolish { at: *p }).collect();
    let e = measure(reps, |i| {
        let cmds = if i % 2 == 0 {
            &build_batch
        } else {
            &demolish_batch
        };
        let r = step(&mut w, cmds);
        assert!(r.rejected.is_empty(), "command rejected: {:?}", r.rejected);
    });
    let ratio = e.median.saturating_mul(1000) / d.median.max(1);
    row(
        &format!("E. tick, {n} accepted commands"),
        &e,
        Some(format!(
            "{}.{:03}x the cost of D",
            ratio / 1000,
            ratio % 1000
        )),
    );

    // F. A road, not a building. This is the structural worst case: changing
    //    the topology invalidates *everything* — the network has to be
    //    relabelled and no cleverness about incremental coverage can help. D
    //    measures the case that is frequent in a game (you build), F the one no
    //    cache will ever be able to cover, and both are needed: an optimisation
    //    that brings D down and leaves F where it is has covered half the
    //    problem.
    let lay = Command::PlaceRoad {
        at: layout.road_slot,
    };
    let remove = Command::Demolish {
        at: layout.road_slot,
    };
    let f = measure(reps, |i| {
        let cmd = if i % 2 == 0 { lay } else { remove };
        let r = step(&mut w, &[cmd]);
        assert!(r.rejected.is_empty(), "command rejected: {:?}", r.rejected);
    });
    row("F. tick, 1 road laid or removed", &f, None);

    // G. Step 3 alone, without the rest of the tick. This is the measure that
    //    attributes the cost instead of deducing it by subtracting D and A, and
    //    it is the one to watch when optimising coverage: D also contains
    //    production, events and command validation.
    let g = measure(reps, |_| {
        let _ = sim_core::Coverage::from_scratch(&w);
    });
    let per_provider = g.median / w.building_count().max(1) as u128;
    row(
        "G. Coverage::from_scratch only (step 3)",
        &g,
        Some(format!("{per_provider} ns/provider")),
    );

    Ok(())
}

// --- building the city ------------------------------------------------------

/// The knob's neutral setting: one place for every resident.
///
/// `--places` counts places per hundred residents, so 100 builds exactly the
/// city the tables ask for and the one every recorded figure was measured on. It
/// is the base of a percentage rather than a balancing number: no game rule
/// reads it, and turning it changes what the benchmark builds and nothing about
/// what the game does.
const FULL: u32 = 100;

/// How many places to build per hundred residents, by kind of provider.
///
/// A bare number sets every kind, `well=70,farm=150` sets them apart, and the
/// two compose: the last mention of a kind wins, so `70,farm=150` reads as
/// "everything thin, except the farms". The kinds are named by the id the tables
/// give them and are never listed here — a roster of building kinds written into
/// the code is what D6 forbids, and it is why the flag is `--places well=70` and
/// not `--well 70`.
struct Places {
    default: u32,
    by_kind: Vec<(String, u32)>,
}

impl Places {
    fn parse(arg: Option<&str>) -> Result<Self, String> {
        let mut out = Self {
            default: FULL,
            by_kind: Vec::new(),
        };
        let Some(arg) = arg else { return Ok(out) };
        for item in arg.split(',').map(str::trim).filter(|i| !i.is_empty()) {
            match item.split_once('=') {
                None => {
                    out.default = item.parse().map_err(|_| {
                        format!("--places wants a number or <kind>=<number>, found '{item}'")
                    })?;
                }
                Some((kind, n)) => {
                    let n = n
                        .parse()
                        .map_err(|_| format!("--places {kind}= wants a number, found '{n}'"))?;
                    out.by_kind.push((kind.to_string(), n));
                }
            }
        }
        Ok(out)
    }

    fn for_kind(&self, id: &str) -> u32 {
        self.by_kind
            .iter()
            .rev()
            .find(|(k, _)| k == id)
            .map_or(self.default, |(_, n)| *n)
    }

    /// Refuses a kind these tables do not have.
    ///
    /// A typo would otherwise be silent: it would match no kind, every provider
    /// would keep the default, and the run would be filed under a knob that
    /// never moved.
    fn check_against(&self, data: &DataSet) -> Result<(), String> {
        let known: Vec<&str> = data
            .buildings
            .iter()
            .filter(|d| d.service().is_some())
            .map(|d| d.id.as_str())
            .collect();
        for (kind, _) in &self.by_kind {
            if !known.contains(&kind.as_str()) {
                return Err(format!(
                    "--places names '{kind}', which is not a provider in these tables \
                     (known: {})",
                    known.join(", ")
                ));
            }
        }
        Ok(())
    }
}

/// How the run was configured: what city to build, and how hard to measure it.
///
/// One value rather than six parameters threaded through every call: they travel
/// together, they are all read off the same command line, and a function taking
/// them apart one by one is a function nobody can add a knob to.
struct Knobs {
    reps: u32,
    zero: bool,
    places: Places,
    houses: Houses,
    /// `None` is the pitch-4 lattice, which stays the default: every reference
    /// figure in this file was measured on it and loses its baseline the moment
    /// the default city changes.
    seed: Option<u64>,
}

/// How many houses to found at each level.
///
/// Empty unless `--houses` was given, and then the city is built exactly as it
/// always was: every house placed by a command and founded by the difficulty
/// profile, with the setup hook never called at all. That is what keeps the
/// default run byte-identical — not the same city arrived at twice, but the same
/// sequence of ticks.
struct Houses(Vec<(Level, u32)>);

impl Houses {
    fn parse(arg: Option<&str>) -> Result<Self, String> {
        let mut out = Vec::new();
        let Some(arg) = arg else { return Ok(Self(out)) };
        for item in arg.split(',').map(str::trim).filter(|i| !i.is_empty()) {
            let (level, n) = item
                .split_once('=')
                .ok_or_else(|| format!("--houses wants <level>=<number>, found '{item}'"))?;
            let level: u8 = level
                .parse()
                .map_err(|_| format!("--houses wants a level number, found '{level}'"))?;
            // Levels count from 1 in the tables, in the messages and here. There
            // is no level 0, and a flag that quietly accepted one would build a
            // different city than the one asked for.
            let level = Level::new(level)
                .ok_or_else(|| "--houses: there is no level 0, the first level is 1".to_string())?;
            let n = n
                .parse()
                .map_err(|_| format!("--houses {}= wants a number, found '{n}'", level.get()))?;
            out.push((level, n));
        }
        Ok(Self(out))
    }

    fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

/// The level each house is founded at, scattered rather than laid down in runs.
///
/// Levels in runs would put every level-2 house in one quarter of the city,
/// which changes the shape of the load on the walk exactly as bunched providers
/// would — the reason [`Spread`] exists. This one hands each position to the
/// level furthest behind the share it is owed, in integers, which scatters them
/// and still ends on the counts that were asked for to the house: every position
/// goes to somebody, and nobody is handed more than its quota.
fn level_sequence(levels: &[(Level, u32)], house_count: u32) -> Vec<Level> {
    let mut placed = vec![0u32; levels.len()];
    let mut out = Vec::with_capacity(house_count as usize);
    for i in 0..house_count {
        let behind = (0..levels.len())
            .filter(|&j| placed[j] < levels[j].1)
            .max_by_key(|&j| {
                i64::from(levels[j].1) * i64::from(i + 1)
                    - i64::from(placed[j]) * i64::from(house_count)
            });
        let Some(j) = behind else { break };
        placed[j] += 1;
        out.push(levels[j].0);
    }
    out
}

/// Which of the two bounds set a kind's count.
///
/// The count the capacity asks for is the one `--places` scales. The count the
/// output sustains is not, and that is not a matter of taste: below it a covered
/// house would go hungry and the agreement between a producer's capacity and
/// what its output feeds would stop holding. The two can coincide, and where
/// they do every setting below 100 leaves that kind exactly where it was — which
/// is why the header names the bound instead of only the count. A reader who
/// could not see it would credit the knob with a city it never built.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Bound {
    Capacity,
    Production,
}

impl Bound {
    const fn describe(self) -> &'static str {
        match self {
            Self::Capacity => "capacity",
            Self::Production => "what the output sustains",
        }
    }
}

/// How many providers of a kind the capacity asks for, at this setting of the
/// knob. `None` if the answer does not fit in a city.
///
/// In `u64` so that a large `--places` cannot overflow, and as one division
/// rather than a multiply after a divide, so that at [`FULL`] it comes out as
/// `population.div_ceil(capacity)` to the unit — ceil(a·k / b·k) is ceil(a/b).
/// That identity is the whole of the guarantee that adding the knob left the
/// city every recorded figure was measured on byte-identical. A capacity of zero
/// is refused before this is reached.
fn by_capacity(population: u32, capacity: u32, per_hundred: u32) -> Option<u32> {
    let wanted = (u64::from(population) * u64::from(per_hundred))
        .div_ceil(u64::from(FULL) * u64::from(capacity));
    u32::try_from(wanted).ok()
}

/// One kind of provider in the synthetic city: how many, and why that many.
struct ProviderKind {
    kind: BuildingKindId,
    how_many: u32,
    per_hundred: u32,
    bound: Bound,
}

/// How many houses and how many providers of each kind, and where to put them.
///
/// The lattice has a pitch of 4: blocks of 3x3 tiles, each with five 1x1 cells
/// on the ring (all facing a road) and a 2x2 square in the middle. The central
/// cell touches no road, so on its own it would be useless: the 2x2 square
/// exists precisely to make use of it.
struct Layout {
    house: BuildingKindId,
    house_count: u32,
    /// The residents the city really settles, which is what a provider's
    /// capacity and the food consumption are both counted against.
    population: u32,
    /// What level to found each house at, in the order `--houses` gave. Empty
    /// unless it was given.
    levels: Vec<(Level, u32)>,
    /// The seeded street network, or `None` for the pitch-4 lattice.
    streets: Option<Streets>,
    /// 1x1 providers, on the ring.
    small: Vec<ProviderKind>,
    /// 2x2 providers, in the middle of the block.
    large: Vec<ProviderKind>,
    blocks_per_side: u32,
    city_blocks: u32,
    road_count: u32,
    /// Free 1x1 slots beyond the city, for measures D and E.
    free_slots: Vec<TilePos>,
    /// The slot reserved for measure F, which builds and demolishes a road on
    /// it. Kept apart from `free_slots` because D and E can leave a building on
    /// it if the number of repetitions is odd.
    road_slot: TilePos,
    any_road: TilePos,
}

/// How many usable 1x1 cells a block has on its ring.
const RING_SLOTS: u32 = 5;
/// How many free slots D (1), E (the remaining 50) and F (1) need.
///
/// Going from 51 to 52 does not change the lattice — `52.div_ceil(5)` and
/// `51.div_ceil(5)` both give 11 blocks — so the measures stay comparable with
/// the reference figures on [`BENCH_DIFFICULTY`].
const TEST_SLOTS: u32 = 52;
/// How many invalid commands measure C sends.
const REJECT_BATCH: usize = 10_000;

impl Layout {
    fn new(
        data: &DataSet,
        difficulty: DifficultyId,
        side: u16,
        residents: u32,
        places: &Places,
        houses: &Houses,
        seed: Option<u64>,
    ) -> Result<Self, String> {
        let house = data
            .buildings
            .iter()
            .position(|d| d.is_house())
            .and_then(|i| u16::try_from(i).ok())
            .map(BuildingKindId::new)
            .ok_or("the dataset contains no house")?;

        // The residents a house is really born with, which is the difficulty's
        // knob and not `rules.house_levels`. At the profile
        // the benchmark measures on the two coincide; reading the profile is
        // what keeps the load honest if that ever stops being true.
        let per_house = u32::from(
            data.difficulty(difficulty)
                .ok_or("the benchmark's profile is not in the dataset")?
                .starting_residents_per_house,
        );
        if per_house == 0 {
            return Err("a house with zero residents does not make a city".into());
        }
        // Without `--houses` the city is what it always was: `residents` rounded
        // up to the last full house, every one of them founded at the first
        // level by the difficulty profile. With it, both numbers come from the
        // histogram instead, and a house founded at a level holds that level's
        // capacity. In `u64` on the way, so an absurd count is an error and not
        // a wrap.
        let (house_count, population) = if houses.is_empty() {
            let house_count = residents.div_ceil(per_house);
            (house_count, house_count * per_house)
        } else {
            let (mut count, mut settled) = (0u64, 0u64);
            for (level, n) in &houses.0 {
                let holds = data.rules.max_residents(*level).ok_or_else(|| {
                    format!(
                        "--houses names level {}, and the tables have {} of them",
                        level.get(),
                        data.rules.house_levels.len()
                    )
                })?;
                count += u64::from(*n);
                settled += u64::from(*n) * u64::from(holds);
            }
            let too_many = || "--houses asks for more than a city can hold".to_string();
            (
                u32::try_from(count).map_err(|_| too_many())?,
                u32::try_from(settled).map_err(|_| too_many())?,
            )
        };
        let per_resident = i64::from(data.rules.food_per_resident.to_millis());

        // The provider shares come out of the tables, not from here: how many
        // are needed to cover the population at the declared capacity — which
        // is in residents — and, for those that produce, how many to sustain
        // its consumption.
        let mut small = Vec::new();
        let mut large = Vec::new();
        for (i, def) in data.buildings.iter().enumerate() {
            let Some(service) = def.service() else {
                continue;
            };
            let kind = u16::try_from(i)
                .map(BuildingKindId::new)
                .map_err(|_| "too many kinds of building")?;
            let capacity = u32::from(
                service
                    .capacity(Level::FIRST)
                    .ok_or_else(|| format!("'{}' declares no capacity at level 1", def.id))?,
            );
            if capacity == 0 {
                return Err(format!("'{}' has capacity 0", def.id));
            }
            let per_hundred = places.for_kind(&def.id);
            let mut how_many = by_capacity(population, capacity, per_hundred).ok_or_else(|| {
                format!(
                    "'{}' at {per_hundred} places wants more providers than a city can hold",
                    def.id
                )
            })?;
            let mut bound = Bound::Capacity;
            if let Some(p) = def.production.as_ref() {
                let per_provider = i64::from(p.output_per_tick.to_millis()) / per_resident.max(1);
                if per_provider > 0 {
                    let sustainable = u32::try_from(per_provider).unwrap_or(u32::MAX);
                    let floor = population.div_ceil(sustainable);
                    // `>=` and not `>`: where the two bounds land on the same
                    // number both of them bind, and the floor is the one worth
                    // reporting, because it is the one a lower `--places` would
                    // meet.
                    if floor >= how_many {
                        how_many = floor;
                        bound = Bound::Production;
                    }
                }
            }
            let provider = ProviderKind {
                kind,
                how_many,
                per_hundred,
                bound,
            };
            match def.size {
                (1, 1) => small.push(provider),
                (2, 2) => large.push(provider),
                s => {
                    return Err(format!(
                        "'{}' has size {s:?}: the benchmark only knows how to lay out 1x1 and 2x2, update it",
                        def.id
                    ));
                }
            }
        }

        // Blocks needed: those the ring requires, and in any case no fewer than
        // the 2x2 providers ask for, one per block.
        let on_ring = house_count + small.iter().map(|p| p.how_many).sum::<u32>();
        let for_large = large.iter().map(|p| p.how_many).sum::<u32>();
        let city_blocks = on_ring.div_ceil(RING_SLOTS).max(for_large).max(1);
        // The blocks for D and E come after the city.
        let total_blocks = city_blocks + TEST_SLOTS.div_ceil(RING_SLOTS);
        let blocks_per_side = (1..).find(|n| n * n >= total_blocks).unwrap_or(1);

        if let Some(seed) = seed {
            // A seeded map needs more blocks than the lattice does for the same
            // city, because the cells that faced a dropped cross street go out
            // of use. So it starts from the lattice's answer and grows until the
            // slots it really finds are enough, rather than guessing a margin
            // and being wrong in one direction or the other.
            let wanted_small = on_ring + TEST_SLOTS;
            let mut blocks = blocks_per_side;
            let mut streets = loop {
                if blocks * 4 + 1 > u32::from(side) {
                    return Err(format!(
                        "a seeded map for {population} residents does not fit on a side of {side}"
                    ));
                }
                match Streets::lay(seed, blocks, for_large, wanted_small) {
                    Some(s) => break s,
                    None => blocks += 1,
                }
            };

            let road_count = streets.roads.len();
            let any_road = *streets.roads.first().ok_or("the seeded map has no roads")?;
            let keep = streets
                .small
                .len()
                .checked_sub(TEST_SLOTS as usize)
                .ok_or("not enough free slots left for measures D, E and F")?;
            let mut free_slots = streets.small.split_off(keep);
            let road_slot = free_slots.pop().ok_or("no slot for measure F")?;

            return Ok(Self {
                house,
                house_count,
                population,
                levels: houses.0.clone(),
                streets: Some(streets),
                small,
                large,
                blocks_per_side: blocks,
                // Every block is part of the city here: placement is over the
                // whole map and not block by block, so this is only what the
                // header reports, never what anything is laid out against.
                city_blocks: blocks * blocks,
                road_count: u32::try_from(road_count).unwrap_or(u32::MAX),
                free_slots,
                road_slot,
                any_road,
            });
        }

        // The lattice only covers the part of the map in use, with one closing
        // road tile: a city that does not fill the map is the normal case, and
        // laying roads everywhere would inflate the measure.
        let side_used = blocks_per_side * 4 + 1;
        if side_used > u32::from(side) {
            // What is needed is named, because three knobs can now decide it and
            // a message quoting only `--residents` would send a reader to the
            // one of them that did not move.
            return Err(format!(
                "{side_used} tiles of side are needed for {population} residents in \
                 {house_count} houses with {} providers, and the map has {side}",
                on_ring - house_count + for_large
            ));
        }

        let mut layout = Self {
            house,
            house_count,
            population,
            levels: houses.0.clone(),
            streets: None,
            small,
            large,
            blocks_per_side,
            city_blocks,
            // The square covered by the lattice minus the 3x3 of each block.
            road_count: side_used * side_used - blocks_per_side * blocks_per_side * 9,
            free_slots: Vec::new(),
            road_slot: TilePos::new(0, 0),
            any_road: TilePos::new(0, 0),
        };
        layout.free_slots = (city_blocks..total_blocks)
            .flat_map(|b| {
                (0..RING_SLOTS).filter_map(move |s| {
                    let (bx, by) = (b % blocks_per_side, b / blocks_per_side);
                    ring_cell(bx, by, s)
                })
            })
            .take(TEST_SLOTS as usize)
            .collect();
        if layout.free_slots.len() < TEST_SLOTS as usize {
            return Err("not enough free slots left for measures D, E and F".into());
        }
        layout.road_slot = layout.free_slots.pop().ok_or("no slot for measure F")?;
        Ok(layout)
    }
}

/// How much of each cross street a seeded map keeps, and how often it pokes a
/// dead end into a block, both in percent.
///
/// They are not balancing numbers — no rule of the game reads either — but they
/// are the shape of the map, so they are named here rather than buried in the
/// generator. Two thirds and a quarter give a network whose road share lands
/// well below the lattice's 45% while leaving most blocks reachable from more
/// than one side.
const STREETS_KEPT: u32 = 66;
const BLOCKS_WITH_A_DEAD_END: u32 = 25;

/// A seeded street network on the block skeleton, and the slots it leaves free.
///
/// The lattice roads 45% of the city's footprint in one component with no dead
/// ends at all, and that flatters anything that skips road tiles — which is what
/// the coverage does for a living. This is the same skeleton with pieces missing.
///
/// **It is connected by construction, and that is the whole argument.** Every
/// avenue runs the full height of the map and one street runs the full width, so
/// each avenue meets every other one whatever happens to the rest; a cross-street
/// segment can then be dropped freely, because nothing reaches anywhere only
/// through it. A dead end is laid only against a tile that is already road, so it
/// is a leaf, and a leaf disconnects nothing either. No walk over the result is
/// needed to know the network is one piece.
struct Streets {
    roads: Vec<TilePos>,
    /// Free 1x1 cells facing a road, shuffled.
    small: Vec<TilePos>,
    /// Free 2x2 squares facing a road, shuffled, none overlapping another or a
    /// cell in `small`.
    large: Vec<TilePos>,
}

impl Streets {
    /// Lays a map on `blocks_per_side` blocks, or `None` if it cannot seat the
    /// slots asked for — which is the caller's signal to try a bigger one.
    fn lay(seed: u64, blocks_per_side: u32, wanted_large: u32, wanted_small: u32) -> Option<Self> {
        use rand::seq::SliceRandom as _;
        use rand::{Rng as _, SeedableRng as _};

        let side = blocks_per_side * 4 + 1;
        u8::try_from(side - 1).ok()?;
        let mut rng = rand_pcg::Pcg64::seed_from_u64(seed);
        let at = |x: u32, y: u32| (y * side + x) as usize;
        let mut road = vec![false; (side * side) as usize];

        // The avenues, full height, and one street across the top: this is the
        // part that may not be dropped, and it is why the rest may.
        for y in 0..side {
            for x in (0..side).step_by(4) {
                road[at(x, y)] = true;
            }
        }
        for x in 0..side {
            road[at(x, 0)] = true;
        }
        // The cross streets, one segment between two avenues at a time.
        for y in (4..side).step_by(4) {
            for bx in 0..blocks_per_side {
                if rng.random_ratio(STREETS_KEPT, 100) {
                    for x in bx * 4..=bx * 4 + 4 {
                        road[at(x, y)] = true;
                    }
                }
            }
        }
        // The dead ends, one tile poked into a block off a street that is
        // already there. The check is what keeps it a leaf instead of an island.
        for by in 0..blocks_per_side {
            for bx in 0..blocks_per_side {
                if !rng.random_ratio(BLOCKS_WITH_A_DEAD_END, 100) {
                    continue;
                }
                let s = rng.random_range(0..RING_SLOTS);
                let (ox, oy) = corner(bx, by);
                let (dx, dy) = match s {
                    0 => (0, 0),
                    1 => (1, 0),
                    2 => (2, 0),
                    3 => (0, 1),
                    _ => (0, 2),
                };
                let (x, y) = (ox + dx, oy + dy);
                if touches_road(&road, at, side, x, y) {
                    road[at(x, y)] = true;
                }
            }
        }

        // The 2x2 squares first, because they are the pickier of the two and a
        // 1x1 taken from under one would be a slot lost for nothing.
        let mut squares: Vec<(u32, u32)> = Vec::new();
        for y in 0..side.saturating_sub(1) {
            for x in 0..side.saturating_sub(1) {
                let tiles = [(x, y), (x + 1, y), (x, y + 1), (x + 1, y + 1)];
                if tiles.iter().all(|(tx, ty)| !road[at(*tx, *ty)])
                    && tiles
                        .iter()
                        .any(|(tx, ty)| touches_road(&road, at, side, *tx, *ty))
                {
                    squares.push((x, y));
                }
            }
        }
        squares.shuffle(&mut rng);

        let mut taken = vec![false; (side * side) as usize];
        let mut large = Vec::new();
        for (x, y) in squares {
            if large.len() as u32 >= wanted_large {
                break;
            }
            let tiles = [(x, y), (x + 1, y), (x, y + 1), (x + 1, y + 1)];
            if tiles.iter().any(|(tx, ty)| taken[at(*tx, *ty)]) {
                continue;
            }
            for (tx, ty) in tiles {
                taken[at(tx, ty)] = true;
            }
            large.push(TilePos::new(u8::try_from(x).ok()?, u8::try_from(y).ok()?));
        }
        if (large.len() as u32) < wanted_large {
            return None;
        }

        let mut small = Vec::new();
        for y in 0..side {
            for x in 0..side {
                if !road[at(x, y)] && !taken[at(x, y)] && touches_road(&road, at, side, x, y) {
                    small.push(TilePos::new(u8::try_from(x).ok()?, u8::try_from(y).ok()?));
                }
            }
        }
        if (small.len() as u32) < wanted_small {
            return None;
        }
        small.shuffle(&mut rng);

        let mut roads = Vec::new();
        for y in 0..side {
            for x in 0..side {
                if road[at(x, y)] {
                    roads.push(TilePos::new(u8::try_from(x).ok()?, u8::try_from(y).ok()?));
                }
            }
        }
        Some(Self {
            roads,
            small,
            large,
        })
    }
}

/// Whether a tile has a road orthogonally beside it.
fn touches_road(road: &[bool], at: impl Fn(u32, u32) -> usize, side: u32, x: u32, y: u32) -> bool {
    let mut yes = false;
    if x > 0 {
        yes |= road[at(x - 1, y)];
    }
    if y > 0 {
        yes |= road[at(x, y - 1)];
    }
    if x + 1 < side {
        yes |= road[at(x + 1, y)];
    }
    if y + 1 < side {
        yes |= road[at(x, y + 1)];
    }
    yes
}

/// The top-left corner of a block, in tiles.
const fn corner(bx: u32, by: u32) -> (u32, u32) {
    (bx * 4 + 1, by * 4 + 1)
}

/// The `s`-th 1x1 cell on a block's ring.
///
/// The ring is the block minus the 2x2 square at the bottom right: five cells,
/// all of them facing a road.
fn ring_cell(bx: u32, by: u32, s: u32) -> Option<TilePos> {
    let (ox, oy) = corner(bx, by);
    let (dx, dy) = match s {
        0 => (0, 0),
        1 => (1, 0),
        2 => (2, 0),
        3 => (0, 1),
        4 => (0, 2),
        _ => return None,
    };
    let x = u8::try_from(ox + dx).ok()?;
    let y = u8::try_from(oy + dy).ok()?;
    Some(TilePos::new(x, y))
}

/// The corner of the 2x2 square in the middle of a block.
fn center_cell(bx: u32, by: u32) -> Option<TilePos> {
    let (ox, oy) = corner(bx, by);
    Some(TilePos::new(
        u8::try_from(ox + 1).ok()?,
        u8::try_from(oy + 1).ok()?,
    ))
}

/// Spreads `share` items evenly over `across` positions.
///
/// Bresenham on integers: no floats and no accumulating error, and the
/// providers end up scattered instead of bunched up at the start — which would
/// change the shape of the load on the BFS.
struct Spread {
    share: u32,
    across: u32,
    done: u32,
}

impl Spread {
    const fn new(share: u32, across: u32) -> Self {
        Self {
            share,
            across: if across == 0 { 1 } else { across },
            done: 0,
        }
    }

    fn takes(&mut self, i: u32) -> bool {
        if (i + 1) * self.share / self.across > self.done {
            self.done += 1;
            true
        } else {
            false
        }
    }
}

fn build(
    data: &Arc<DataSet>,
    layout: &Layout,
    side: u16,
    difficulty: DifficultyId,
) -> Result<World, String> {
    let grid = Grid::new(side, side, Terrain::Plain).map_err(|e| format!("grid: {e}"))?;
    // The world's seed stays where it was, and `--seed` never touches it: the
    // map's dice and the simulation's are two different things, so two maps
    // differ in their roads and not in their births and deaths, and the sweep
    // stays comparable.
    let mut w = World::new(grid, Arc::clone(data), 42, difficulty);

    let (roads, buildings) = match &layout.streets {
        Some(streets) => seeded_plan(layout, streets),
        None => lattice_plan(layout),
    };

    // The roads first, all in one tick: the topology has to be there before the
    // buildings go looking for an entrance.
    apply(&mut w, &roads)?;
    build_city(&mut w, layout, &buildings)?;
    Ok(w)
}
/// Where every road and every building goes on the pitch-4 lattice.
fn lattice_plan(layout: &Layout) -> (Vec<Command>, Vec<Command>) {
    let mut roads = Vec::new();
    let side_used = layout.blocks_per_side * 4 + 1;
    for y in 0..side_used {
        for x in 0..side_used {
            if x % 4 == 0 || y % 4 == 0 {
                let (Ok(x), Ok(y)) = (u8::try_from(x), u8::try_from(y)) else {
                    continue;
                };
                roads.push(Command::PlaceRoad {
                    at: TilePos::new(x, y),
                });
            }
        }
    }

    let mut buildings = Vec::new();
    let mut small: Vec<(BuildingKindId, Spread)> = layout
        .small
        .iter()
        .map(|p| {
            (
                p.kind,
                Spread::new(p.how_many, layout.city_blocks * RING_SLOTS),
            )
        })
        .collect();
    let mut large: Vec<(BuildingKindId, Spread)> = layout
        .large
        .iter()
        .map(|p| (p.kind, Spread::new(p.how_many, layout.city_blocks)))
        .collect();
    let mut houses_left = layout.house_count;

    for b in 0..layout.city_blocks {
        let (bx, by) = (b % layout.blocks_per_side, b / layout.blocks_per_side);

        for (kind, sp) in &mut large {
            if sp.takes(b) {
                if let Some(origin) = center_cell(bx, by) {
                    buildings.push(Command::PlaceBuilding {
                        kind: *kind,
                        origin,
                    });
                }
                break;
            }
        }

        for s in 0..RING_SLOTS {
            let Some(origin) = ring_cell(bx, by, s) else {
                continue;
            };
            let i = b * RING_SLOTS + s;
            // The first provider that claims this slot takes it; the others
            // slide to the next one, because `takes` only advances when it
            // places something.
            let mut provider = None;
            for (k, sp) in &mut small {
                if sp.takes(i) {
                    provider = Some(*k);
                    break;
                }
            }
            let kind = match provider {
                Some(k) => k,
                None if houses_left > 0 => {
                    houses_left -= 1;
                    layout.house
                }
                None => continue,
            };
            buildings.push(Command::PlaceBuilding { kind, origin });
        }
    }
    (roads, buildings)
}

/// Where every road and every building goes on a seeded map.
///
/// No [`Spread`] here: the slots came out of [`Streets::lay`] already shuffled,
/// so handing them out a kind at a time scatters each kind over the whole map
/// instead of bunching it. What the lattice needs Bresenham for, the shuffle has
/// already done — and it does it *unevenly*, which is the point. The lattice
/// never puts two providers side by side, and a real city does.
fn seeded_plan(layout: &Layout, streets: &Streets) -> (Vec<Command>, Vec<Command>) {
    let roads = streets
        .roads
        .iter()
        .map(|at| Command::PlaceRoad { at: *at })
        .collect();

    let mut buildings = Vec::new();
    let mut squares = streets.large.iter();
    for p in &layout.large {
        for origin in squares.by_ref().take(p.how_many as usize) {
            buildings.push(Command::PlaceBuilding {
                kind: p.kind,
                origin: *origin,
            });
        }
    }
    let mut cells = streets.small.iter();
    for p in &layout.small {
        for origin in cells.by_ref().take(p.how_many as usize) {
            buildings.push(Command::PlaceBuilding {
                kind: p.kind,
                origin: *origin,
            });
        }
    }
    for origin in cells.take(layout.house_count as usize) {
        buildings.push(Command::PlaceBuilding {
            kind: layout.house,
            origin: *origin,
        });
    }
    (roads, buildings)
}

/// Puts the buildings up, founding the levels on the way if `--houses` asked for
/// any.
fn build_city(w: &mut World, layout: &Layout, buildings: &[Command]) -> Result<(), String> {
    if layout.levels.is_empty() {
        return apply(w, buildings);
    }

    // With `--houses` the houses go up first, are founded at their levels, and
    // only then do the providers arrive — so the first coverage ever computed is
    // computed over the occupancies the run really has, and no state the
    // measures could start on is ever a half-built one. It costs a tick that the
    // default city does not pay, which is why the default takes the branch above
    // and places everything at once, exactly as it always did.
    let (houses, providers): (Vec<Command>, Vec<Command>) = buildings
        .iter()
        .partition(|c| matches!(c, Command::PlaceBuilding { kind, .. } if *kind == layout.house));
    apply(w, &houses)?;

    let ids: Vec<_> = w.houses().map(|(id, _)| id).collect();
    for (id, level) in ids
        .into_iter()
        .zip(level_sequence(&layout.levels, layout.house_count))
    {
        if !w.set_house_level(id, level) {
            return Err(format!(
                "the tables have no level {}, which --houses asked for",
                level.get()
            ));
        }
    }
    apply(w, &providers)
}

fn apply(w: &mut World, cmds: &[Command]) -> Result<(), String> {
    let r = step(w, cmds);
    match r.rejected.first() {
        None => Ok(()),
        Some((i, e)) => Err(format!(
            "the benchmark produced an invalid command ({} of {}): {e}",
            i,
            cmds.len()
        )),
    }
}

/// A copy of the dataset with only the starting treasury raised off the scale.
///
/// It is not a balancing number: it is how the economy gets taken out of the
/// measure, so the city always gets built in full. Everything else stays as
/// `sim-data` has it.
fn with_unlimited_treasury(real: &DataSet) -> DataSet {
    rebuilt(real, |rules| {
        rules.starting_treasury = Coins::new(i32::MAX / 2);
    })
}

/// The same dataset with the demographics **switched off**.
///
/// What `--zero-demographics` measures on, and it is the term slot 19.5 needs:
/// `A` with the rates at zero is `H`, the tick that pays for step 6 but not for the
/// coverage recomputation step 6 triggers. Without `H` the cost of the coverage
/// chasing the population cannot be told apart from the cost of the demographics
/// themselves, and "optimise the recomputation" would be a guess — which is
/// exactly the mistake this project has already made twice: the obvious
/// hypothesis about where the cost lay was wrong both times, and only the
/// measurement said so.
fn without_demographics(real: &DataSet) -> DataSet {
    rebuilt(real, |rules| {
        rules.starting_treasury = Coins::new(i32::MAX / 2);
        rules.demographics.births_per_thousand_per_month = 0;
        rules.demographics.deaths_per_thousand_per_month = 0;
        rules
            .demographics
            .deaths_per_thousand_per_month_when_unserved = 0;
        // Migration's flows move the population exactly as the two above do, so
        // `H` — the tick that pays for step 6 but not for the recomputation it
        // triggers — has to switch these off too, or a "zero rates" run still
        // has migration quietly moving people and `H` stops meaning what its own
        // name says. The founding rate too: the reference city starts under the
        // founding threshold like every other, so that is the rate it reads
        // first, and left running it would fill the city while the flag claims
        // nothing moves.
        rules.migration.founding_immigration_per_thousand_per_month = 0;
        rules.migration.immigration_per_thousand_per_month = 0;
        rules.migration.emigration_per_thousand_per_month_unhappy = 0;
    })
}

fn rebuilt(real: &DataSet, edit: impl FnOnce(&mut sim_core::Rules)) -> DataSet {
    let mut rules = real.rules.clone();
    edit(&mut rules);
    DataSet::new(
        rules,
        real.road_cost_per_terrain,
        real.buildings.clone(),
        real.difficulties.clone(),
    )
}

/// What share of the residents each service reaches, and what share live in a
/// house that has everything its own level asks for.
///
/// One share per service and not a single figure: the first house level is a hut
/// that requires water only, so a number read off the levels' requirements alone
/// would say nothing whatever about the farms in a city whose houses are all at
/// level 1 — which is every city this benchmark builds. Counted in residents and
/// not in houses, because residents are the unit a provider's capacity is
/// counted in.
fn covered(w: &World) -> String {
    let population = w.population();
    let share = |served: u32| served * 100 / population.max(1);
    let residents_where = |wanted: &dyn Fn(&sim_core::House) -> bool| -> u32 {
        w.houses()
            .filter(|(_, h)| wanted(h))
            .map(|(_, h)| u32::from(h.residents))
            .sum()
    };

    let mut out = String::new();
    for k in ServiceKind::ALL {
        let served = residents_where(&|h| h.served.get(k));
        out.push_str(&format!("{} {}%, ", k.as_id(), share(served)));
    }
    let by_level = residents_where(&|h| {
        w.data()
            .rules
            .required_at(h.level)
            .iter()
            .all(|k| h.served.get(*k))
    });
    out.push_str(&format!(
        "own level's needs {}% of residents",
        share(by_level)
    ));
    out
}

/// How many providers ran out of capacity, out of how many there are.
///
/// A provider stops walking the moment its capacity reaches zero, so one that
/// stopped has consumed its capacity exactly, and `used == capacity` is the same
/// statement as "it filled up" — no counter inside the core is needed to say so.
/// That fraction is what decides what the stop is worth: build the services
/// generously and few providers fill, so the stop fires seldom and earns little;
/// build them thinly and more fill, and it earns more.
fn filled(w: &World) -> (u32, u32) {
    let mut used: BTreeMap<BuildingId, u32> = BTreeMap::new();
    for (house, h) in w.houses() {
        for k in ServiceKind::ALL {
            if let Some(provider) = w.coverage().served_by(house, k) {
                *used.entry(provider).or_insert(0) += u32::from(h.residents);
            }
        }
    }

    let (mut full, mut providers) = (0, 0);
    for (id, b) in w.buildings() {
        let Some(capacity) = w
            .data()
            .def(b.kind)
            .and_then(|d| d.service())
            .and_then(|s| s.capacity(b.level))
        else {
            continue;
        };
        providers += 1;
        if used.get(&id).copied().unwrap_or(0) >= u32::from(capacity) {
            full += 1;
        }
    }
    (full, providers)
}

// --- measuring and printing -------------------------------------------------

struct Measurement {
    median: u128,
    worst: u128,
    /// How many times the measured closure ran, warm-ups included. It is what
    /// a count taken across the whole measure — the recomputations — has to be
    /// divided by.
    ticks: u32,
}

fn measure(reps: u32, mut f: impl FnMut(u32)) -> Measurement {
    // Two warm-up rounds before timing anything: the first tick of a measure
    // pays for allocations and cold caches, and on its own it tripled the
    // "worst" figure. There are two and not one because D and E alternate
    // building and demolishing, and after a pair the state is back where it was.
    f(0);
    f(1);

    let mut samples: Vec<u128> = (0..reps).map(|i| time_it(|| f(i))).collect();
    samples.sort_unstable();
    Measurement {
        // The median and not the mean: a preemption by the operating system
        // moves the mean and not the median, and what matters here is the
        // typical cost.
        median: samples.get(samples.len() / 2).copied().unwrap_or(0),
        worst: samples.last().copied().unwrap_or(0),
        ticks: reps + 2,
    }
}

/// The only place in the project that reads the clock.
///
/// `clippy.toml` forbids `Instant::now` because **the core** must not touch the
/// clock (D4). Here we are in the headless runner, outside the core, and timing
/// is precisely its job: the exception sits in exactly one place on purpose,
/// rather than loosening the rule in `clippy.toml`.
#[allow(clippy::disallowed_methods)]
fn time_it(f: impl FnOnce()) -> u128 {
    let t0 = std::time::Instant::now();
    f();
    t0.elapsed().as_nanos()
}

fn row(name: &str, m: &Measurement, note: Option<String>) {
    println!(
        "  {:<40} {:>12} {:>12}  {}",
        name,
        format_duration(m.median),
        format_duration(m.worst),
        note.unwrap_or_default()
    );
}

/// Formats nanoseconds without floats: `float_arithmetic` is `deny` across the
/// workspace, and in any case the decimals come out better from an integer
/// division than from a binary rounding.
fn format_duration(ns: u128) -> String {
    if ns >= 1_000_000 {
        format!("{}.{:03} ms", ns / 1_000_000, (ns / 1_000) % 1_000)
    } else if ns >= 1_000 {
        format!("{}.{:03} us", ns / 1_000, ns % 1_000)
    } else {
        format!("{ns} ns")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tables() -> DataSet {
        sim_data::load_default().expect("the production tables load")
    }

    fn profile(data: &DataSet) -> DifficultyId {
        data.difficulty_by_id(BENCH_DIFFICULTY)
            .expect("the benchmark's profile is in the dataset")
    }

    fn layout(data: &DataSet, residents: u32, places: &str) -> Layout {
        let places = Places::parse(Some(places)).expect("the knob parses");
        Layout::new(
            data,
            profile(data),
            200,
            residents,
            &places,
            &Houses::parse(None).expect("no histogram"),
            None,
        )
        .expect("the layout fits the map")
    }

    fn how_many(layout: &Layout, data: &DataSet, id: &str) -> u32 {
        layout
            .small
            .iter()
            .chain(&layout.large)
            .find(|p| data.def(p.kind).is_some_and(|d| d.id == id))
            .map_or(0, |p| p.how_many)
    }

    /// At its neutral setting the knob is an identity, not an approximation.
    ///
    /// This is the automated half of "the default run prints the hash it printed
    /// before". A recorded hash constant would say the same thing once and then
    /// rot the first time a table moved; this holds however the tables change.
    #[test]
    fn a_hundred_places_asks_for_exactly_what_the_capacity_asked_for() {
        for population in [0, 1, 19, 20, 21, 3_000, 14_970, 15_000, u32::MAX / 2] {
            for capacity in [1, 20, 32, 255, u32::MAX] {
                assert_eq!(
                    by_capacity(population, capacity, FULL),
                    Some(population.div_ceil(capacity)),
                    "{population} residents against a capacity of {capacity}"
                );
            }
        }
    }

    /// The knob scales that count, and says so rather than overflowing when the
    /// answer is absurd.
    #[test]
    fn the_knob_scales_the_count_and_refuses_an_impossible_one() {
        assert_eq!(by_capacity(15_000, 32, 70), Some(329));
        assert_eq!(by_capacity(15_000, 32, FULL), Some(469));
        assert_eq!(by_capacity(15_000, 32, 150), Some(704));
        // No places at all is a city with no wells, not an error: it is one end
        // of the axis the knob exists to sweep.
        assert_eq!(by_capacity(15_000, 32, 0), Some(0));
        assert_eq!(by_capacity(u32::MAX, 1, u32::MAX), None);
    }

    /// Below the neutral setting a producer does not move, because what its
    /// output sustains is a floor the knob does not scale.
    ///
    /// The header names the bound for exactly this reason: without it a run at
    /// `--places 70` would look like a city thinned all through, when half of it
    /// was never touched, and the measurement would be filed under the wrong
    /// cause.
    #[test]
    fn a_producer_holds_its_floor_below_the_neutral_setting() {
        let data = tables();
        let neutral = layout(&data, 15_000, "100");

        let thin = layout(&data, 15_000, "70");
        for p in thin.small.iter().chain(&thin.large) {
            let id = &data.def(p.kind).expect("the kind is in the tables").id;
            let before = how_many(&neutral, &data, id);
            match p.bound {
                Bound::Production => assert_eq!(p.how_many, before, "'{id}' is held by its floor"),
                Bound::Capacity => assert!(p.how_many < before, "'{id}' thins out"),
            }
        }

        let generous = layout(&data, 15_000, "150");
        for p in generous.small.iter().chain(&generous.large) {
            let id = &data.def(p.kind).expect("the kind is in the tables").id;
            assert!(
                p.how_many > how_many(&neutral, &data, id),
                "'{id}' grows past its floor"
            );
            assert_eq!(p.bound, Bound::Capacity, "'{id}' is clear of its floor");
        }
    }

    /// A kind these tables do not have is a typo, and a typo that matched
    /// nothing would leave every provider at its default and file the run under
    /// a knob that never moved.
    #[test]
    fn a_kind_the_tables_do_not_have_is_refused() {
        let data = tables();
        let complaint = Places::parse(Some("wel=70"))
            .expect("it parses")
            .check_against(&data)
            .expect_err("an unknown kind is refused");
        assert!(complaint.contains("wel"), "{complaint}");
        assert!(
            complaint.contains("well"),
            "names the ones it knows: {complaint}"
        );
        // A house is not a provider, so naming one is the same mistake.
        assert!(
            Places::parse(Some("house=70"))
                .expect("it parses")
                .check_against(&data)
                .is_err()
        );
    }

    /// The last mention of a kind wins, so a bare number can be written first
    /// and then narrowed.
    #[test]
    fn a_bare_number_sets_every_kind_and_a_named_one_narrows_it() {
        let places = Places::parse(Some("70,farm=150")).expect("it parses");
        assert_eq!(places.for_kind("well"), 70);
        assert_eq!(places.for_kind("farm"), 150);
        assert_eq!(
            Places::parse(None).expect("it parses").for_kind("well"),
            FULL
        );
    }

    /// The levels come out exactly as asked for, and scattered rather than in
    /// runs.
    ///
    /// The exact sequence is asserted and not merely the counts: what it is
    /// costs nothing to write down, and it is the only way a change that quietly
    /// started bunching them would be caught.
    #[test]
    fn the_levels_come_out_exactly_as_asked_for_and_scattered() {
        let (one, two) = (
            Level::new(1).expect("level 1"),
            Level::new(2).expect("level 2"),
        );
        assert_eq!(
            level_sequence(&[(one, 4), (two, 2)], 6),
            [one, two, one, one, two, one]
        );
        // Nobody is owed anything when nobody was asked for.
        assert!(level_sequence(&[], 0).is_empty());
    }

    /// The histogram founds exactly the city it names, and the residents come
    /// with the levels.
    #[test]
    fn the_histogram_founds_exactly_the_city_it_names() {
        let data = Arc::new(with_unlimited_treasury(&tables()));
        let difficulty = profile(&data);
        let side = 60;
        let wanted = [(1u8, 100u32), (2, 40), (3, 10)];

        let houses = Houses::parse(Some("1=100,2=40,3=10")).expect("the knob parses");
        let places = Places::parse(None).expect("the knob parses");
        let layout = Layout::new(&data, difficulty, side, 0, &places, &houses, None)
            .expect("the layout fits the map");
        let w = build(&data, &layout, side, difficulty).expect("the city gets built");

        let settled: u32 = wanted
            .iter()
            .map(|(l, n)| {
                let level = Level::new(*l).expect("a level the tables have");
                n * u32::from(data.rules.max_residents(level).expect("its capacity"))
            })
            .sum();
        assert_eq!(layout.population, settled, "the residents the levels hold");

        assert_eq!(w.house_count(), 150);
        let mut per_level = vec![0u32; data.rules.house_levels.len()];
        for (_, h) in w.houses() {
            per_level[h.level.as_usize()] += 1;
        }
        assert_eq!(
            per_level,
            vec![100, 40, 10],
            "to the house, not about right"
        );
    }

    /// A level nobody could build at is refused, and refused where it is asked
    /// for rather than clamped into the nearest one that exists.
    #[test]
    fn a_level_the_tables_do_not_have_is_refused() {
        assert!(
            Houses::parse(Some("0=10")).is_err(),
            "there is no level 0: the first level is 1"
        );
        let data = tables();
        let refused = Layout::new(
            &data,
            profile(&data),
            200,
            0,
            &Places::parse(None).expect("the knob parses"),
            &Houses::parse(Some("9=10")).expect("it parses"),
            None,
        );
        let Err(complaint) = refused else {
            panic!("a level past the end of the table has to be refused")
        };
        assert!(complaint.contains("level 9"), "{complaint}");
    }

    /// A seeded map is one road network, whatever the dice did to it.
    ///
    /// The generator argues this rather than checking it — full-height avenues
    /// meeting one full-width street, and dead ends laid only against a tile
    /// that is already road — and an argument in a comment is worth what the
    /// test beside it is worth. Several seeds, because one that happened to keep
    /// every segment would prove nothing.
    #[test]
    fn a_seeded_map_is_a_single_road_network() {
        let data = Arc::new(with_unlimited_treasury(&tables()));
        let difficulty = profile(&data);
        let side = 60;
        for seed in 0..6 {
            let layout = Layout::new(
                &data,
                difficulty,
                side,
                600,
                &Places::parse(None).expect("the knob parses"),
                &Houses::parse(None).expect("no histogram"),
                Some(seed),
            )
            .expect("the layout fits the map");
            let w = build(&data, &layout, side, difficulty).expect("the city gets built");
            assert_eq!(
                w.roads().component_count(),
                1,
                "seed {seed} left the streets in more than one piece"
            );
        }
    }

    /// A seeded map is raggeder than the lattice in the two ways that matter:
    /// fewer roads, and dead ends.
    ///
    /// The lattice has neither — every one of its road tiles has at least two
    /// road neighbours — and both are what flatters a walk that skips road
    /// tiles. Asserting the lattice has **no** dead end is half the value here:
    /// it is what makes the other half a difference rather than a number.
    #[test]
    fn a_seeded_map_has_fewer_roads_and_dead_ends_where_the_lattice_has_neither() {
        let data = tables();
        let of = |seed: Option<u64>| {
            let layout = Layout::new(
                &data,
                profile(&data),
                200,
                15_000,
                &Places::parse(None).expect("the knob parses"),
                &Houses::parse(None).expect("no histogram"),
                seed,
            )
            .expect("the layout fits the map");
            let roads: std::collections::BTreeSet<(u8, u8)> = match &layout.streets {
                Some(streets) => streets.roads.iter().map(|p| (p.x, p.y)).collect(),
                None => lattice_plan(&layout)
                    .0
                    .iter()
                    .filter_map(|c| match c {
                        Command::PlaceRoad { at } => Some((at.x, at.y)),
                        _ => None,
                    })
                    .collect(),
            };
            let dead_ends = roads
                .iter()
                .filter(|(x, y)| {
                    [
                        (x.wrapping_sub(1), *y),
                        (x + 1, *y),
                        (*x, y.wrapping_sub(1)),
                        (*x, y + 1),
                    ]
                    .iter()
                    .filter(|n| roads.contains(n))
                    .count()
                        == 1
                })
                .count();
            (roads.len(), dead_ends)
        };

        let (lattice_roads, lattice_dead_ends) = of(None);
        assert_eq!(
            lattice_dead_ends, 0,
            "the lattice is the thing with no dead ends"
        );
        let (seeded_roads, seeded_dead_ends) = of(Some(1));
        assert!(
            seeded_roads < lattice_roads,
            "{seeded_roads} roads against the lattice's {lattice_roads}"
        );
        assert!(seeded_dead_ends > 0, "a ragged map has somewhere to stop");
    }

    /// The same seed builds the same city, and a different one does not.
    ///
    /// Without the first half a sweep across seeds could not be repeated; without
    /// the second the knob would be doing nothing and every run would agree for
    /// the wrong reason.
    #[test]
    fn a_seed_is_the_whole_of_what_makes_the_map() {
        let data = Arc::new(with_unlimited_treasury(&tables()));
        let difficulty = profile(&data);
        let side = 60;
        let city = |seed: Option<u64>| {
            let layout = Layout::new(
                &data,
                difficulty,
                side,
                600,
                &Places::parse(None).expect("the knob parses"),
                &Houses::parse(None).expect("no histogram"),
                seed,
            )
            .expect("the layout fits the map");
            let w = build(&data, &layout, side, difficulty).expect("the city gets built");
            sim_replay::hash_hex(&sim_replay::hash_world(&w))
        };
        assert_eq!(city(Some(3)), city(Some(3)), "a seed repeats");
        assert_ne!(city(Some(3)), city(Some(4)), "and two seeds differ");
        assert_ne!(city(Some(3)), city(None), "and none of them is the lattice");
    }

    /// The knob reaches the city, and it is the served share that says so.
    ///
    /// The provider count moving is not the claim worth testing — that is
    /// arithmetic already covered above. What matters is that thinning the wells
    /// really leaves residents without water, counted in residents, because
    /// residents are the unit a provider's capacity is counted in.
    #[test]
    fn thinner_services_reach_fewer_residents() {
        let data = Arc::new(with_unlimited_treasury(&tables()));
        let difficulty = profile(&data);
        let side = 60;
        let share = |places: &str| -> u32 {
            let places = Places::parse(Some(places)).expect("the knob parses");
            let layout = Layout::new(
                &data,
                difficulty,
                side,
                600,
                &places,
                &Houses::parse(None).expect("no histogram"),
                None,
            )
            .expect("the layout fits the map");
            let w = build(&data, &layout, side, difficulty).expect("the city gets built");
            let served: u32 = w
                .houses()
                .filter(|(_, h)| h.served.get(ServiceKind::Water))
                .map(|(_, h)| u32::from(h.residents))
                .sum();
            served * 100 / w.population().max(1)
        };
        let (thin, neutral, generous) = (share("70"), share("100"), share("150"));
        assert!(
            thin < neutral,
            "70 places reaches fewer than 100: {thin} vs {neutral}"
        );
        assert!(
            neutral < generous,
            "150 places reaches more than 100: {generous} vs {neutral}"
        );
    }
}
