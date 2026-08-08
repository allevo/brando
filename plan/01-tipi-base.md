# Fase 01 — Tipi fondamentali e griglia

**Goal:** esistono `Milli`, `Coins`, gli id newtype e la `Grid`; `Tile` sta nel budget di
memoria e la conversione posizione↔indice è biiettiva su tutta la mappa.
**Dipende da:** 00.
**Dimensione:** M.
**Decisioni coinvolte:** D4 (niente float), D5 (scala), A1, A4.

## Perché adesso

Questi tipi entrano nella firma di tutto il resto. Cambiare `Milli` in i64 o `TileIdx` in u32
quando ci sono già cinque sistemi sopra è un refactoring che tocca ogni file. Cambiarlo adesso
tocca solo i test di questa fase.

## Cosa si costruisce

### `Milli(i32)` — quantità frazionarie

```rust
/// Quantità in millesimi di unità. Niente float nello stato (D4).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default, Serialize, Deserialize)]
pub struct Milli(i32);
```

API minima, solo ciò che serve nelle fasi 04–07:

- `from_units(i32) -> Option<Milli>` (checked: 3.000.000 unità non ci stanno)
- `const fn from_millis(i32) -> Milli`, `fn to_millis(self) -> i32`
- `checked_add`, `checked_sub`, `checked_mul_int(i32)`
- `saturating_add`, `saturating_sub` — ammesse solo dove la saturazione è la semantica di gioco
  voluta, con un commento che lo dica
- `div_int(self, d: i32) -> Option<Milli>`, **troncamento verso zero, documentato**
- `Display` che stampa `12.500` (per gli snapshot e la mappa ASCII)

Nessun `impl Add`/`Sub` nudo: l'operatore invita a ignorare l'overflow, e in un core che non
può panicare (convenzioni: niente `unwrap` fuori dagli invarianti impossibili) l'overflow
silenzioso è peggio del rumore visivo di `checked_add`.

`Coins(i32)` per il denaro, stessa forma senza millesimi (vedi [A1](decisioni-aperte.md)).

### Id newtype

```rust
slotmap::new_key_type! { pub struct BuildingId; pub struct HouseId; }

/// Indice lineare di tile: y * width + x. Mappa max 256×256 (A4).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TileIdx(u16);

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct TilePos { pub x: u8, pub y: u8 }
```

`usize` nudo non compare in nessuna firma pubblica (convenzioni).

Nota su `slotmap`: l'iterazione è per indice di slot, quindi deterministica a parità di
sequenza di inserimenti e rimozioni — che è garantita dal log dei comandi (D4). Va scritto come
doc comment sul campo `buildings`, perché è un invariante non ovvio da cui dipende l'hash.

### `Grid` e `Tile`

```rust
pub struct Grid {
    width: u8,
    height: u8,
    tiles: Vec<Tile>,          // len = width * height
}

/// Budget: 4 byte. 40.000 tile ⇒ 160 KB, sta in L2 (D: Tile deve restare piccolo).
#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub struct Tile {
    terrain: Terrain,          // enum #[repr(u8)]
    flags: TileFlags,          // bitflag u8: HAS_ROAD, ...
    occupant: OccupantSlot,    // u16: indice compatto, non Option<Box<_>>
}
```

`Grid` espone: `new(width, height, Terrain) -> Result<Grid, GridError>` (rifiuta 0 e >256),
`idx(TilePos) -> TileIdx`, `pos(TileIdx) -> TilePos`, `get`/`get_mut`, `in_bounds`,
`neighbors4(TileIdx) -> impl Iterator<Item = TileIdx>` **senza wraparound** — il bug classico
è che il vicino "a destra" di x=width-1 finisce sulla riga sotto.

`OccupantSlot` merita attenzione: serve a sapere, da un tile, quale edificio o casa lo occupa,
senza `Option<Box<...>>`. Forma consigliata: `u16` con sentinella `u16::MAX` = vuoto, più un
bit in `flags` che dice se l'occupante è casa o edificio. La mappa da slot a `BuildingId` sta
nel `World`, non nel tile.

## Fuori scope

Niente `World`, niente `Building`, niente `House` (fase 04). Niente pathfinding (05).
Niente terreni con proprietà di gioco: `Terrain` in questa fase è solo un enum, i numeri
associati (costruibile? costo?) arrivano dalle tabelle in fase 03.

## Test

Property test (`proptest`), perché qui gli invarianti sono aritmetici e le proprietà valgono
davvero:

1. **Biiezione posizione↔indice**: per ogni `width`, `height` in 1..=256 e ogni pos valida,
   `pos(idx(p)) == p`. E per ogni idx valido, `idx(pos(i)) == i`.
2. **`neighbors4` non esce e non wrappa**: ogni vicino restituito è in bounds e ha
   distanza di Manhattan 1 dal centro. I tile d'angolo hanno esattamente 2 vicini, quelli di
   bordo 3, gli interni 4.
3. **`Milli` non panica mai**: per ogni coppia di `i32`, le operazioni `checked_*` restituiscono
   `None` o un risultato corretto, mai un panic. Confronto contro `i64` come oracolo.
4. **`div_int` tronca verso zero** anche per operandi negativi: `-1500 / 2 == -750`,
   `-1501 / 2 == -750` (non `-751`). Test tabellare, non property.
5. **Budget di memoria**, test statico:
   ```rust
   #[test]
   fn tile_sta_nel_budget() {
       assert_eq!(size_of::<Tile>(), 4, "Tile è cresciuto: 40.000 tile devono stare in cache");
   }
   ```
6. `Grid::new` rifiuta `0` e `257` con l'errore giusto.

## Verifica

```sh
cargo test -p sim-core
cargo clippy --workspace --all-targets -- -D warnings
```

**Fatto quando:** i sei gruppi di test sono verdi e `size_of::<Tile>() == 4`. Se `Tile` è
uscito dal budget, la fase non è chiusa: si torna su `OccupantSlot` prima di costruirci sopra.
