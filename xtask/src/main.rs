//! Runner headless: partite singole, batch, rigenerazione dei golden.
//!
//! E' il posto dove il gioco gira senza alcuna dipendenza grafica: `sim-core`
//! e basta. Se qui servisse Bevy, sarebbe un errore architetturale.

mod bench;
mod golden;
mod scenario;

use std::process::ExitCode;
use std::sync::Arc;

use sim_core::{ServiceKind, World};

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let comando = args.first().map(String::as_str);

    match comando {
        Some("run") => esito(run(&args[1..])),
        Some("record") => esito(record(&args[1..])),
        Some("regen-golden") => esito(regen_golden(&args[1..])),
        Some("bench") => esito(bench::bench(&args[1..])),
        _ => {
            uso();
            ExitCode::FAILURE
        }
    }
}

fn esito(r: Result<(), String>) -> ExitCode {
    match r {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("errore: {e}");
            ExitCode::FAILURE
        }
    }
}

fn uso() {
    eprintln!("uso: cargo xtask <comando>");
    eprintln!();
    eprintln!("  run --scenario <nome> --ticks <n> [--dump-every <n>]");
    eprintln!("  record --scenario <nome> --out <file.ron>");
    eprintln!("  regen-golden [--check]");
    eprintln!("  bench [--lato <n>] [--abitanti <n>] [--ripetizioni <n>]   (usare --release)");
    eprintln!();
    eprintln!("scenari: {}", scenario::NOMI.join(", "));
}

/// Registra uno scenario in un `.ron` a scelta.
///
/// I golden degli scenari noti li scrive `regen-golden`; questo serve a
/// tirarne fuori uno in un percorso qualunque, per ispezionarlo o per
/// costruirci sopra un caso nuovo.
fn record(args: &[String]) -> Result<(), String> {
    let nome = opzione(args, "--scenario").ok_or("serve --scenario <nome>")?;
    let out = opzione(args, "--out").ok_or("serve --out <file.ron>")?;

    let data = Arc::new(sim_data::load_default().map_err(|e| format!("tabelle: {e}"))?);
    let sc =
        scenario::per_nome(&nome, &data).ok_or_else(|| format!("scenario sconosciuto: {nome}"))?;
    let rec = golden::registra(&sc, &data);

    let path = std::path::Path::new(&out);
    rec.save(path).map_err(|e| format!("{out}: {e}"))?;
    println!("scritto {out} ({} comandi)", rec.commands.len());
    Ok(())
}

/// Rigenera i golden, o verifica che non ci sarebbe niente da rigenerare.
fn regen_golden(args: &[String]) -> Result<(), String> {
    let check = args.iter().any(|a| a == "--check");
    let data = Arc::new(sim_data::load_default().map_err(|e| format!("tabelle: {e}"))?);
    let differenze = golden::regen(&data, check)?;

    if differenze.is_empty() {
        println!("golden aggiornati, niente da fare");
        return Ok(());
    }
    if check {
        eprintln!("questi golden sarebbero cambiati:");
        for d in &differenze {
            eprintln!("  - {d}");
        }
        return Err(
            "i golden non sono aggiornati. Se il cambiamento e' voluto, esegui \n  \
             cargo xtask regen-golden\n\
             e committa il diff; se non lo e', c'e' una fonte di non-determinismo da trovare"
                .to_string(),
        );
    }
    println!("rigenerati:");
    for d in &differenze {
        println!("  - {d}");
    }
    Ok(())
}

fn run(args: &[String]) -> Result<(), String> {
    let nome = opzione(args, "--scenario").unwrap_or_else(|| "minimo".to_string());
    let ticks: u32 = numero(args, "--ticks")?.unwrap_or(120);
    let dump_every: u32 = numero(args, "--dump-every")?.unwrap_or(30);

    let data = Arc::new(sim_data::load_default().map_err(|e| format!("tabelle: {e}"))?);
    let sc =
        scenario::per_nome(&nome, &data).ok_or_else(|| format!("scenario sconosciuto: {nome}"))?;

    println!("scenario '{}' — {}", sc.nome, sc.descrizione);
    println!(
        "seed {}, griglia {}x{}, dataset {}",
        sc.seed,
        sc.lato,
        sc.lato,
        &data.hash_hex()[..16]
    );
    println!();
    intestazione();

    let mut w = sc.mondo(Arc::clone(&data));
    let mut rifiutati = 0usize;
    for t in 0..ticks {
        let cmds = sc.comandi_al_tick(t);
        let r = sim_core::step(&mut w, &cmds);
        rifiutati += r.rejected.len();
        for (i, e) in &r.rejected {
            eprintln!("  tick {t}: comando {i} rifiutato: {e}");
        }
        if dump_every > 0 && (t + 1) % dump_every == 0 {
            riga(&w);
        }
    }

    println!();
    println!("{} comandi rifiutati in totale", rifiutati);
    Ok(())
}

fn intestazione() {
    println!(
        "{:>6}  {:>5}  {:>4}  {:>5}  {:>9}  {:>7}  {:>7}  {:>7}",
        "tick", "mesi", "case", "abit.", "giacenza", "acqua", "cibo", "tesoro"
    );
}

fn riga(w: &World) {
    let mesi = w.tick() / w.data().rules.tick_per_mese;
    let con_acqua = w
        .houses()
        .filter(|(_, h)| h.servita.get(ServiceKind::Acqua))
        .count();
    let con_cibo = w
        .houses()
        .filter(|(_, h)| h.servita.get(ServiceKind::Cibo))
        .count();
    let case = w.n_case();
    println!(
        "{:>6}  {:>5}  {:>4}  {:>5}  {:>9}  {:>3}/{:<3}  {:>3}/{:<3}  {:>7}",
        w.tick(),
        mesi,
        case,
        w.popolazione(),
        milli(w.giacenza_totale()),
        con_acqua,
        case,
        con_cibo,
        case,
        w.economy().tesoro
    );
}

/// Formatta millesimi come `12.500`, come fa `Milli::Display`.
fn milli(v: i64) -> String {
    let segno = if v < 0 { "-" } else { "" };
    let a = v.unsigned_abs();
    format!("{segno}{}.{:03}", a / 1000, a % 1000)
}

fn opzione(args: &[String], nome: &str) -> Option<String> {
    let i = args.iter().position(|a| a == nome)?;
    args.get(i + 1).cloned()
}

fn numero(args: &[String], nome: &str) -> Result<Option<u32>, String> {
    match opzione(args, nome) {
        None => Ok(None),
        Some(v) => v
            .parse()
            .map(Some)
            .map_err(|_| format!("{nome} vuole un numero, trovato '{v}'")),
    }
}
