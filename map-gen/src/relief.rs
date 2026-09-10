//! Step 2: mountains and seas shape their own ground.
//!
//! Every tile starts at [`crate::SEA_LEVEL`]. Only mountain and
//! sea sources act here; a tile a `Land` source claimed keeps that height
//! for good. Each participating source picks one tile of its own `A`
//! uniformly at random as its origin, and keeps a growing set `C`, seeded
//! with that one tile. The same turn schedule step 1 uses governs when a
//! source acts, with the round counter restarted at 0.
//!
//! On its turn, a source first rolls, independently for every tile
//! currently in `C`: with probability exactly `1/(d+1)`, `d` the Manhattan
//! distance from that tile to the source's origin, the tile's height moves
//! one step towards the source's extreme, never past the caller's
//! `max_height`/`min_height`. Then — regardless of whether any roll
//! succeeded — the source adds to `C` every neighbour of `C`'s tiles that
//! belongs to its own `A` and is not already in `C`. A source stops acting
//! once `C` equals `A`: growth, not height convergence, is what ends its
//! turn.
//!
//! This terminates for the same reason step 1 does: `A` is connected, `C`
//! only ever grows and stays a subset of `A`, and as long as `C` is a
//! strict subset some tile of it neighbours a tile of `A` it has not yet
//! reached — so every act grows `C` by at least one tile, unconditionally,
//! until `C = A`.

use sim_core::{Grid, TileIndex, TilePos};

use crate::assign::{AssignResult, SourceId};
use crate::{SEA_LEVEL, Source, SourceKind, dice};

pub(crate) fn run(
    seed: u64,
    sources: &[Source],
    assigned: &AssignResult,
    grid: &Grid,
    max_strength: u16,
    min_height: u8,
    max_height: u8,
) -> Vec<u8> {
    let n = grid.len() as usize;
    let mut height = vec![SEA_LEVEL; n];

    let participants: Vec<usize> = sources
        .iter()
        .enumerate()
        .filter(|(_, s)| s.kind != SourceKind::Land)
        .map(|(i, _)| i)
        .collect();
    if participants.is_empty() {
        return height;
    }

    let mut rng = dice::stream(seed, "relief");

    let mut origin_pos: Vec<TilePos> = Vec::with_capacity(participants.len());
    let mut c: Vec<Vec<TileIndex>> = Vec::with_capacity(participants.len());
    let mut in_c: Vec<bool> = vec![false; n];
    for &i in &participants {
        let a_i = &assigned.claimed[i];
        let k = dice::below(&mut rng, a_i.len() as u64) as usize;
        let origin = a_i[k];
        origin_pos.push(grid.pos(origin).expect("claimed tile is on the grid"));
        in_c[usize::from(origin.get())] = true;
        c.push(vec![origin]);
    }

    let round_cap = (n as u64).saturating_mul(u64::from(max_strength) + 1) + 1;
    let mut round: u64 = 0;
    loop {
        let all_done = participants
            .iter()
            .enumerate()
            .all(|(p, &i)| c[p].len() == assigned.claimed[i].len());
        if all_done {
            break;
        }

        for (p, &i) in participants.iter().enumerate() {
            let a_len = assigned.claimed[i].len();
            if c[p].len() == a_len {
                continue;
            }
            let period = u64::from(max_strength - sources[i].strength) + 1;
            if !round.is_multiple_of(period) {
                continue;
            }

            // A snapshot of C before this act: a tile grown into C during
            // this same act has not yet had a turn to roll, and does not
            // get one until the source's next act.
            let snapshot_len = c[p].len();

            for &t in &c[p][..snapshot_len] {
                let here = grid.pos(t).expect("tile in C is on the grid");
                let d = u64::from(here.manhattan(origin_pos[p]));
                if dice::below(&mut rng, d + 1) == 0 {
                    let ti = usize::from(t.get());
                    let h = height[ti];
                    match sources[i].kind {
                        SourceKind::Mountain if h < max_height => height[ti] = h + 1,
                        SourceKind::Sea if h > min_height => height[ti] = h - 1,
                        SourceKind::Mountain | SourceKind::Sea => {}
                        SourceKind::Land => unreachable!("land sources never join `participants`"),
                    }
                }
            }

            for k in 0..snapshot_len {
                let t = c[p][k];
                for nb in grid.neighbors4(t) {
                    let ni = usize::from(nb.get());
                    if assigned.owner[ni] == Some(SourceId::new(i)) && !in_c[ni] {
                        in_c[ni] = true;
                        c[p].push(nb);
                    }
                }
            }
        }
        round += 1;
        debug_assert!(
            round < round_cap,
            "step 2 did not converge — a bug, not a seed"
        );
    }

    height
}

#[cfg(test)]
mod tests {
    use sim_core::{Grid, Terrain, TilePos};

    use super::*;
    use crate::assign;

    fn grid(w: u16, h: u16) -> Grid {
        Grid::new(w, h, Terrain::Plain).expect("valid size")
    }

    #[test]
    fn relief_only_ever_touches_tiles_owned_by_a_mountain_or_a_sea_source() {
        let g = grid(10, 10);
        let sources = vec![
            Source {
                point: TilePos::new(0, 0),
                kind: SourceKind::Mountain,
                strength: 1,
            },
            Source {
                point: TilePos::new(9, 9),
                kind: SourceKind::Land,
                strength: 1,
            },
        ];
        let assigned = assign::run(1, &sources, &g, 1);
        let height = run(1, &sources, &assigned, &g, 1, 0, 31);
        for (i, owner) in assigned.owner.iter().enumerate() {
            if owner == &Some(crate::assign::SourceId::new(1)) {
                assert_eq!(height[i], SEA_LEVEL, "land-owned tile moved");
            }
        }
    }

    #[test]
    fn relief_never_pushes_a_height_past_the_callers_min_or_max() {
        let g = grid(12, 12);
        let sources = vec![
            Source {
                point: TilePos::new(0, 0),
                kind: SourceKind::Mountain,
                strength: 1,
            },
            Source {
                point: TilePos::new(11, 11),
                kind: SourceKind::Sea,
                strength: 1,
            },
        ];
        let assigned = assign::run(2, &sources, &g, 1);
        let height = run(2, &sources, &assigned, &g, 1, 6, 10);
        assert!(height.iter().all(|&h| (6..=10).contains(&h)));
    }

    #[test]
    fn a_lone_mountain_source_never_lowers_a_tile_below_sea_level() {
        let g = grid(10, 10);
        let sources = vec![Source {
            point: TilePos::new(5, 5),
            kind: SourceKind::Mountain,
            strength: 1,
        }];
        let assigned = assign::run(3, &sources, &g, 1);
        let height = run(3, &sources, &assigned, &g, 1, 0, 31);
        assert!(height.iter().all(|&h| h >= SEA_LEVEL));
    }

    #[test]
    fn a_lone_sea_source_never_raises_a_tile_above_sea_level() {
        let g = grid(10, 10);
        let sources = vec![Source {
            point: TilePos::new(5, 5),
            kind: SourceKind::Sea,
            strength: 1,
        }];
        let assigned = assign::run(4, &sources, &g, 1);
        let height = run(4, &sources, &assigned, &g, 1, 0, 31);
        assert!(height.iter().all(|&h| h <= SEA_LEVEL));
    }

    #[test]
    fn a_source_whose_claimed_set_is_a_single_tile_never_acts_at_all() {
        let g = grid(1, 1);
        let sources = vec![Source {
            point: TilePos::new(0, 0),
            kind: SourceKind::Mountain,
            strength: 1,
        }];
        let assigned = assign::run(5, &sources, &g, 1);
        let height = run(5, &sources, &assigned, &g, 1, 0, 31);
        assert_eq!(height, vec![SEA_LEVEL]);
    }

    #[test]
    fn relief_terminates_with_every_participating_sources_c_equal_to_its_a() {
        // If the loop above ever returned without every participant's C
        // reaching A, this would simply hang — proptest-free but pinned:
        // this test finishing at all is the assertion.
        let g = grid(16, 16);
        let sources = vec![
            Source {
                point: TilePos::new(0, 0),
                kind: SourceKind::Mountain,
                strength: 3,
            },
            Source {
                point: TilePos::new(15, 15),
                kind: SourceKind::Sea,
                strength: 1,
            },
            Source {
                point: TilePos::new(0, 15),
                kind: SourceKind::Land,
                strength: 2,
            },
        ];
        let assigned = assign::run(6, &sources, &g, 3);
        let _height = run(6, &sources, &assigned, &g, 3, 0, 31);
    }
}
