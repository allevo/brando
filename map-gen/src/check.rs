//! The checks a [`Config`] has to pass before any pass runs.

use sim_core::{Grid, GridError, Terrain, Tile};

use crate::{Config, ConfigError, SEA_LEVEL};

/// Refuses an unusable config with its first fault, or hands back the grid
/// every pass reads its geometry from.
pub(crate) fn run(config: &Config) -> Result<Grid, ConfigError> {
    let (width, height) = (config.width, config.height);
    let (min_height, max_height) = (config.min_height, config.max_height);

    // The size limit is asked of the grid rather than restated here, so
    // there is only ever one answer to how big a map may be.
    let grid = match Grid::new(width, height, Terrain::Plain) {
        Ok(g) => g,
        Err(GridError::InvalidSize { width, height, max }) => {
            return Err(ConfigError::UnusableSize { width, height, max });
        }
    };

    let cap = Tile::MAX_GROUND_HEIGHT;
    if min_height > max_height || max_height > cap {
        return Err(ConfigError::InvalidHeightRange {
            min_height,
            max_height,
            cap,
        });
    }
    if !(min_height <= SEA_LEVEL && SEA_LEVEL <= max_height) {
        return Err(ConfigError::HeightRangeExcludesSeaLevel {
            min_height,
            max_height,
            sea_level: SEA_LEVEL,
        });
    }

    let sources = &config.sources;
    if sources.is_empty() {
        return Err(ConfigError::NoSources);
    }
    for (index, s) in sources.iter().enumerate() {
        if !grid.in_bounds(s.point) {
            return Err(ConfigError::SourceOutOfBounds {
                index,
                x: s.point.x,
                y: s.point.y,
                width,
                height,
            });
        }
    }
    for i in 0..sources.len() {
        for j in (i + 1)..sources.len() {
            if sources[i].point == sources[j].point {
                return Err(ConfigError::DuplicateSourceOrigin {
                    first: i,
                    second: j,
                    x: sources[i].point.x,
                    y: sources[i].point.y,
                });
            }
        }
    }

    Ok(grid)
}
