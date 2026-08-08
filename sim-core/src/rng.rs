//! RNG deterministico, uno stream per dominio (D4).
//!
//! Il requisito difficile non e' seedare l'RNG: e' che il giorno in cui si
//! aggiunge un dominio nuovo i golden replay degli scenari esistenti restino
//! verdi. Per questo il seed di ogni stream deriva dal **nome** del dominio e
//! non dalla sua posizione nell'enum.

use rand::{RngCore, SeedableRng};
use rand_pcg::Pcg64;
use serde::{Deserialize, Serialize};

/// Domini RNG indipendenti.
///
/// Aggiungere una variante non deve alterare le sequenze delle varianti
/// esistenti: se lo fa, la derivazione e' per posizione ed e' un bug, non un
/// golden da rigenerare.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Serialize, Deserialize)]
pub enum RngDomain {
    Events,
    Migration,
    Production,
}

impl RngDomain {
    /// Tutte le varianti, in ordine di dichiarazione. L'ordine conta solo per
    /// l'hash canonico (fase 08), non per la derivazione dei seed.
    pub const TUTTI: [RngDomain; 3] = [
        RngDomain::Events,
        RngDomain::Migration,
        RngDomain::Production,
    ];

    pub const COUNT: usize = Self::TUTTI.len();

    /// Sale stabile del dominio.
    ///
    /// Non rinominare una variante senza rigenerare i golden: il nome e' parte
    /// del contratto di determinismo, non un dettaglio estetico.
    const fn salt(self) -> &'static str {
        match self {
            Self::Events => "brando/rng/v1/events",
            Self::Migration => "brando/rng/v1/migration",
            Self::Production => "brando/rng/v1/production",
        }
    }

    /// Slot occupato dal dominio in [`RngSet`].
    ///
    /// Assegnato per variante e non per posizione in [`RngDomain::TUTTI`]:
    /// e' cio' che permette di aggiungere un dominio in testa all'enum senza
    /// spostare gli stream esistenti.
    const fn index(self) -> usize {
        match self {
            Self::Events => 0,
            Self::Migration => 1,
            Self::Production => 2,
        }
    }

    /// Inversa di [`RngDomain::index`]. La biiezione e' verificata da
    /// `index_e_una_biiezione`.
    const fn from_index(i: usize) -> Option<Self> {
        match i {
            0 => Some(Self::Events),
            1 => Some(Self::Migration),
            2 => Some(Self::Production),
            _ => None,
        }
    }
}

/// Uno stream RNG con il conteggio delle estrazioni.
///
/// Il conteggio non serve al gioco: serve all'hash canonico (fase 08). A
/// parita' di seed, il numero di estrazioni determina univocamente lo stato
/// del generatore, quindi hashare `(seed, draws)` copre lo stato dell'RNG
/// senza dover serializzare i suoi interni.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct Stream {
    rng: Pcg64,
    draws: u64,
}

impl Stream {
    fn from_seed_bytes(bytes: [u8; 32]) -> Self {
        Self {
            rng: Pcg64::from_seed(bytes),
            draws: 0,
        }
    }

    /// Quante estrazioni sono state fatte da questo stream.
    pub const fn draws(&self) -> u64 {
        self.draws
    }

    const fn conta(&mut self) {
        // wrapping e non `+=`: il core non panica. In pratica non ci si
        // arriva mai, 2^64 estrazioni non sono raggiungibili in una partita.
        self.draws = self.draws.wrapping_add(1);
    }
}

impl RngCore for Stream {
    fn next_u32(&mut self) -> u32 {
        self.conta();
        self.rng.next_u32()
    }

    fn next_u64(&mut self) -> u64 {
        self.conta();
        self.rng.next_u64()
    }

    fn fill_bytes(&mut self, dst: &mut [u8]) {
        self.conta();
        self.rng.fill_bytes(dst);
    }
}

/// L'insieme degli stream, uno per dominio.
///
/// I campi non sono pubblici di proposito: se due sistemi potessero estrarre
/// dallo stesso stream nello stesso tick, l'ordine di estrazione diventerebbe
/// un contratto implicito. [`RngSet::get`] presta un dominio alla volta.
#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub struct RngSet {
    streams: [Stream; RngDomain::COUNT],
}

impl RngSet {
    /// Deriva uno stream per dominio a partire dal seed della partita.
    ///
    /// `blake3(seed_le || salt)` in modalita' XOF riempie per intero i 32 byte
    /// di seed di `Pcg64`: `seed_from_u64` scarterebbe entropia e renderebbe
    /// piu' facile una collisione tra domini.
    ///
    /// Gli stream si riempiono per **slot** (`index()`), non scorrendo
    /// `TUTTI`: altrimenti aggiungere una variante in testa all'enum
    /// sposterebbe gli stream esistenti e sfaserebbe tutti i golden.
    pub fn from_seed(seed: u64) -> Self {
        let streams = std::array::from_fn(|i| {
            // Invariante dimostrabile: `from_index` e' l'inversa di `index`
            // su 0..COUNT, presidiato da `index_e_una_biiezione`.
            let d = RngDomain::from_index(i).expect("slot < COUNT ha sempre un dominio");
            Stream::from_seed_bytes(derive_seed(seed, d))
        });
        Self { streams }
    }

    /// Presta in scrittura lo stream di un dominio.
    pub fn get(&mut self, domain: RngDomain) -> &mut Stream {
        &mut self.streams[domain.index()]
    }

    /// Posizione dello stream, per l'hash canonico dello stato (fase 08).
    pub fn draws(&self, domain: RngDomain) -> u64 {
        self.streams[domain.index()].draws()
    }
}

fn derive_seed(seed: u64, domain: RngDomain) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(&seed.to_le_bytes());
    hasher.update(domain.salt().as_bytes());
    let mut out = [0u8; 32];
    hasher.finalize_xof().fill(&mut out);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sequenza(seed: u64, domain: RngDomain, n: usize) -> Vec<u64> {
        let mut set = RngSet::from_seed(seed);
        let s = set.get(domain);
        (0..n).map(|_| s.next_u64()).collect()
    }

    /// Valori attesi scritti a mano: confrontare due istanze tra loro
    /// passerebbe anche con una derivazione sbagliata.
    ///
    /// Se questo test rompe dopo aver **aggiunto** una variante a
    /// `RngDomain`, la derivazione e' per posizione e non per nome: e' un bug,
    /// non un golden da rigenerare.
    ///
    /// Se rompe dopo aver **rinominato** una variante o cambiato un sale, e'
    /// atteso: il nome del dominio e' parte del contratto (vanno rigenerati
    /// anche i golden replay).
    #[test]
    fn sequenze_riproducibili_con_valori_attesi() {
        assert_eq!(
            sequenza(42, RngDomain::Events, 8),
            [
                16_951_895_464_066_535_839,
                10_528_375_347_967_641_558,
                6_732_969_908_935_899_607,
                15_963_220_654_581_270_673,
                3_540_804_277_719_876_773,
                15_145_133_130_928_511_643,
                8_827_474_317_410_546_936,
                2_537_525_544_625_458_919,
            ]
        );
        assert_eq!(
            sequenza(42, RngDomain::Migration, 4),
            [
                6_744_713_315_132_201_080,
                14_951_314_113_004_524_121,
                14_001_924_983_284_815_708,
                9_329_562_633_619_259_286,
            ]
        );
        assert_eq!(
            sequenza(42, RngDomain::Production, 4),
            [
                7_953_230_224_566_493_169,
                13_149_629_979_718_932_555,
                474_551_565_481_971_545,
                1_458_220_722_024_169_394,
            ]
        );
    }

    /// `index` e `from_index` sono l'una l'inversa dell'altra su `0..COUNT`.
    /// E' l'invariante che autorizza l'`expect` in `RngSet::from_seed`.
    #[test]
    fn index_e_una_biiezione() {
        for d in RngDomain::TUTTI {
            assert!(d.index() < RngDomain::COUNT, "{d:?} fuori dagli slot");
            assert_eq!(RngDomain::from_index(d.index()), Some(d));
        }
        for i in 0..RngDomain::COUNT {
            let d = RngDomain::from_index(i).expect("slot coperto");
            assert_eq!(d.index(), i);
        }
        assert_eq!(RngDomain::from_index(RngDomain::COUNT), None);
    }

    /// Coglie il copia-incolla in cui due domini condividono il sale.
    #[test]
    fn domini_indipendenti() {
        let e = sequenza(42, RngDomain::Events, 8);
        let m = sequenza(42, RngDomain::Migration, 8);
        let p = sequenza(42, RngDomain::Production, 8);
        assert_ne!(e, m);
        assert_ne!(e, p);
        assert_ne!(m, p);
    }

    /// L'ordine con cui i sistemi chiamano i domini non deve influire sulle
    /// rispettive sequenze: A,B,A,B e A,A,B,B danno gli stessi risultati.
    #[test]
    fn ordine_di_chiamata_irrilevante() {
        let mut alternato = RngSet::from_seed(7);
        let mut a1 = Vec::new();
        let mut b1 = Vec::new();
        for _ in 0..4 {
            a1.push(alternato.get(RngDomain::Events).next_u64());
            b1.push(alternato.get(RngDomain::Production).next_u64());
        }

        let mut raggruppato = RngSet::from_seed(7);
        let a2: Vec<_> = (0..4)
            .map(|_| raggruppato.get(RngDomain::Events).next_u64())
            .collect();
        let b2: Vec<_> = (0..4)
            .map(|_| raggruppato.get(RngDomain::Production).next_u64())
            .collect();

        assert_eq!(a1, a2);
        assert_eq!(b1, b2);
        assert_eq!(alternato, raggruppato, "anche lo stato finale coincide");
    }

    #[test]
    fn seed_diversi_sequenze_diverse() {
        for d in RngDomain::TUTTI {
            assert_ne!(sequenza(42, d, 8), sequenza(43, d, 8), "dominio {d:?}");
        }
    }

    /// Il conteggio delle estrazioni e' cio' che l'hash canonico usera' per
    /// distinguere "consumati 5 valori" da "consumati 6" (fase 08).
    #[test]
    fn le_estrazioni_si_contano_per_dominio() {
        let mut set = RngSet::from_seed(1);
        assert_eq!(set.draws(RngDomain::Events), 0);
        for _ in 0..5 {
            set.get(RngDomain::Events).next_u64();
        }
        set.get(RngDomain::Migration).next_u32();
        assert_eq!(set.draws(RngDomain::Events), 5);
        assert_eq!(set.draws(RngDomain::Migration), 1);
        assert_eq!(set.draws(RngDomain::Production), 0);
    }

    /// Stato uguale sse stesso seed e stesse estrazioni: e' l'assunzione che
    /// rende sufficiente hashare (seed, draws) invece degli interni di Pcg64.
    #[test]
    fn stato_determinato_da_seed_e_conteggio() {
        let mut a = RngSet::from_seed(9);
        let mut b = RngSet::from_seed(9);
        for _ in 0..3 {
            a.get(RngDomain::Events).next_u64();
        }
        assert_ne!(a, b);
        for _ in 0..3 {
            b.get(RngDomain::Events).next_u64();
        }
        assert_eq!(a, b);
    }
}
