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
//! The only parameter forced by hand is the starting treasury, raised to a huge
//! value to take the economy out of the picture: what is measured here is the
//! cost of the tick, not whether the city can be afforded.

use std::sync::Arc;

use sim_core::{
    BuildingKindId, Coins, Command, DataSet, DifficultyId, Grid, Level, Terrain, TilePos, World,
    step,
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
/// | `G` `compute_from_scratch` alone | 3.05 ms — 2.5 µs/provider |
///
/// `A` is the number to watch: it is paid on **every** tick, while `D` is paid
/// only when the player does something. Phase 14 then moved `A` to 3.600 ms by
/// making the coverage recompute almost every tick, which is the whole
/// subject of the open question at slot 18.5.
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

    let real = sim_data::load_default().map_err(|e| format!("tables: {e}"))?;
    println!(
        "dataset {}, difficulty '{BENCH_DIFFICULTY}'",
        &real.hash_hex()[..16]
    );
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

    // With just one of --side and --residents you measure that profile and
    // nothing else; with neither, the two reference profiles are measured.
    match (side, residents) {
        (None, None) => {
            for (name, s, r) in PRESETS {
                preset(&real, name, s, r, reps, zero)?;
            }
        }
        (s, r) => preset(
            &real,
            "custom",
            s.unwrap_or(200).try_into().unwrap_or(u16::MAX),
            r.unwrap_or(15_000),
            reps,
            zero,
        )?,
    }
    Ok(())
}

fn preset(
    real: &DataSet,
    name: &str,
    side: u16,
    residents: u32,
    reps: u32,
    zero: bool,
) -> Result<(), String> {
    println!("\n=== {name}: {side}x{side}, {residents} residents ===");

    let data = Arc::new(if zero {
        without_demographics(real)
    } else {
        with_unlimited_treasury(real)
    });
    let difficulty = data
        .difficulty_by_id(BENCH_DIFFICULTY)
        .ok_or_else(|| format!("the dataset has no '{BENCH_DIFFICULTY}' profile"))?;
    let layout = Layout::new(&data, difficulty, side, residents)?;
    let mut w = build(&data, &layout, side, difficulty)?;

    println!(
        "  {} houses ({} res.), {} providers, {} road tiles out of {} tiles",
        w.house_count(),
        w.population(),
        w.building_count(),
        layout.road_count,
        u32::from(side) * u32::from(side),
    );
    println!(
        "  state hash: {}",
        &sim_replay::hash_hex(&sim_replay::hash_world(&w))[..16]
    );
    println!();
    println!("  {:<38} {:>12} {:>12}", "", "median", "worst");

    // A. An empty tick with nothing dirty: this is what you pay in the ticks
    //    where the player does not build, i.e. the vast majority.
    //
    //    The **count of recomputations** across it is printed alongside,
    //    because the number on its own cannot be diagnosed: an `A` costing
    //    milliseconds with zero recomputations is a completely different fault
    //    from one costing the same with a recomputation on every tick. It is
    //    also how `J` — the fraction of ticks in which the population moved —
    //    is read off directly instead of estimated (slot 18.5).
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
        let _ = sim_core::coverage::compute_from_scratch(&w);
    });
    let per_provider = g.median / w.building_count().max(1) as u128;
    row(
        "G. compute_from_scratch only (step 3)",
        &g,
        Some(format!("{per_provider} ns/provider")),
    );

    Ok(())
}

// --- building the city ------------------------------------------------------

/// How many houses and how many providers of each kind, and where to put them.
///
/// The lattice has a pitch of 4: blocks of 3x3 tiles, each with five 1x1 cells
/// on the ring (all facing a road) and a 2x2 square in the middle. The central
/// cell touches no road, so on its own it would be useless: the 2x2 square
/// exists precisely to make use of it.
struct Layout {
    house: BuildingKindId,
    house_count: u32,
    /// 1x1 providers, on the ring: (kind, how many).
    small: Vec<(BuildingKindId, u32)>,
    /// 2x2 providers, in the middle of the block.
    large: Vec<(BuildingKindId, u32)>,
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
        let house_count = residents.div_ceil(per_house);
        // The residents actually settled: `residents` rounded up to the last
        // full house. Both the providers' capacity and the food consumption are
        // counted against this, not against `house_count`.
        let population = house_count * per_house;
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
            let mut how_many = population.div_ceil(capacity);
            if let Some(p) = def.production.as_ref() {
                let per_provider = i64::from(p.output_per_tick.to_millis()) / per_resident.max(1);
                if per_provider > 0 {
                    let sustainable = u32::try_from(per_provider).unwrap_or(u32::MAX);
                    how_many = how_many.max(population.div_ceil(sustainable));
                }
            }
            match def.size {
                (1, 1) => small.push((kind, how_many)),
                (2, 2) => large.push((kind, how_many)),
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
        let on_ring = house_count + small.iter().map(|(_, q)| *q).sum::<u32>();
        let for_large = large.iter().map(|(_, q)| *q).sum::<u32>();
        let city_blocks = on_ring.div_ceil(RING_SLOTS).max(for_large).max(1);
        // The blocks for D and E come after the city.
        let total_blocks = city_blocks + TEST_SLOTS.div_ceil(RING_SLOTS);
        let blocks_per_side = (1..).find(|n| n * n >= total_blocks).unwrap_or(1);

        // The lattice only covers the part of the map in use, with one closing
        // road tile: a city that does not fill the map is the normal case, and
        // laying roads everywhere would inflate the measure.
        let side_used = blocks_per_side * 4 + 1;
        if side_used > u32::from(side) {
            return Err(format!(
                "{side_used} tiles of side are needed for {residents} residents, the map has {side}"
            ));
        }

        let mut layout = Self {
            house,
            house_count,
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
    let mut w = World::new(grid, Arc::clone(data), 42, difficulty);

    // The roads first, all in one tick: the topology has to be there before the
    // buildings go looking for an entrance.
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
    apply(&mut w, &roads)?;

    let mut buildings = Vec::new();
    let mut small: Vec<(BuildingKindId, Spread)> = layout
        .small
        .iter()
        .map(|(k, q)| (*k, Spread::new(*q, layout.city_blocks * RING_SLOTS)))
        .collect();
    let mut large: Vec<(BuildingKindId, Spread)> = layout
        .large
        .iter()
        .map(|(k, q)| (*k, Spread::new(*q, layout.city_blocks)))
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
    apply(&mut w, &buildings)?;
    Ok(w)
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
/// What `--zero-demographics` measures on, and it is the term slot 18.5 needs:
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
        "  {:<38} {:>12} {:>12}  {}",
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
