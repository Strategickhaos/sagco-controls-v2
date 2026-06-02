/// sagco-sentinel-dream — Act VI: The Sentinel Dreams
/// Compares what was EXPECTED to what ACTUALLY HAPPENED.
/// Reads the ledger of past runs, computes expected opcode distribution
/// from the stable human codon table, then measures divergence from observed.
///
/// This is the honest version of the DNA analysis:
///   expected = human codon frequency × sequence length
///   actual   = what sagco-dna produced
///   variance = |expected - actual| per opcode
///   alert    = if variance > threshold → SAGCO_DREAM_VARIANCE_ALERT
///
/// USE: sagco-sentinel-dream <sagco-dna-report.json> [--threshold 5.0]
use sha2::{Digest, Sha256};
use chrono::Utc;
use serde_json::{json, Value};
use std::{collections::HashMap, env, fs, io::Write};

fn seal(s: &str) -> String { format!("{:x}", Sha256::digest(s.as_bytes())) }

/// Human codon frequency per 1000 codons (Kazusa DB)
fn human_codon_freq() -> HashMap<&'static str, f64> {
    [
        ("ATG",22.0),("TAA",1.0),("TAG",0.8),("TGA",1.6),
        ("TTT",17.6),("TTC",20.3),("TTA",7.7),("TTG",12.9),
        ("CTT",13.2),("CTC",19.6),("CTA",7.2),("CTG",39.6),
        ("ATT",15.9),("ATC",20.8),("ATA",7.5),
        ("GTT",11.0),("GTC",14.5),("GTA",7.1),("GTG",28.1),
        ("TCT",15.2),("TCC",17.7),("TCA",12.2),("TCG",4.4),("AGT",15.2),("AGC",19.5),
        ("CCT",17.5),("CCC",19.8),("CCA",16.9),("CCG",6.9),
        ("ACT",13.1),("ACC",18.9),("ACA",15.1),("ACG",6.1),
        ("GCT",18.4),("GCC",27.7),("GCA",15.8),("GCG",6.2),
        ("TAT",12.2),("TAC",15.3),
        ("CAT",10.9),("CAC",15.1),
        ("CAA",12.3),("CAG",34.2),
        ("AAT",17.0),("AAC",19.1),
        ("AAA",24.4),("AAG",31.9),
        ("GAT",21.8),("GAC",25.1),
        ("GAA",29.0),("GAG",39.6),
        ("TGT",10.6),("TGC",12.6),
        ("TGG",13.2),
        ("CGT",4.5),("CGC",10.4),("CGA",6.2),("CGG",11.4),("AGA",12.2),("AGG",12.0),
        ("GGT",10.8),("GGC",22.2),("GGA",16.5),("GGG",16.5),
    ].iter().cloned().collect()
}

fn codon_to_opcode() -> HashMap<&'static str, &'static str> {
    [
        ("ATG","OBSERVE"),("TAA","SEAL"),("TAG","SEAL"),("TGA","SEAL"),
        ("TTT","TOKENIZE"),("TTC","TOKENIZE"),
        ("TTA","LEXER"),("TTG","LEXER"),("CTT","LEXER"),("CTC","LEXER"),("CTA","LEXER"),("CTG","LEXER"),
        ("ATT","CLASSIFY"),("ATC","CLASSIFY"),("ATA","CLASSIFY"),
        ("GTT","VARIANCE"),("GTC","VARIANCE"),("GTA","VARIANCE"),("GTG","VARIANCE"),
        ("TCT","STEPPER"),("TCC","STEPPER"),("TCA","STEPPER"),("TCG","STEPPER"),("AGT","STEPPER"),("AGC","STEPPER"),
        ("CCT","PIPELINE"),("CCC","PIPELINE"),("CCA","PIPELINE"),("CCG","PIPELINE"),
        ("ACT","TOPOLOGY"),("ACC","TOPOLOGY"),("ACA","TOPOLOGY"),("ACG","TOPOLOGY"),
        ("GCT","AGENT"),("GCC","AGENT"),("GCA","AGENT"),("GCG","AGENT"),
        ("TAT","FORECAST"),("TAC","FORECAST"),
        ("CAT","HUNT"),("CAC","HUNT"),
        ("CAA","QUERY"),("CAG","QUERY"),
        ("AAT","NODE"),("AAC","NODE"),
        ("AAA","KEYFILE"),("AAG","KEYFILE"),
        ("GAT","DAEMON"),("GAC","DAEMON"),
        ("GAA","EVIDENCE"),("GAG","EVIDENCE"),
        ("TGT","CHAINVERIFY"),("TGC","CHAINVERIFY"),
        ("TGG","WATCH"),
        ("CGT","RECLASS"),("CGC","RECLASS"),("CGA","RECLASS"),("CGG","RECLASS"),("AGA","RECLASS"),("AGG","RECLASS"),
        ("GGT","GUARD"),("GGC","GUARD"),("GGA","GUARD"),("GGG","GUARD"),
    ].iter().cloned().collect()
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let report_path = args.get(1).cloned().unwrap_or_else(|| {
        // default: use latest DNA report
        fs::read_dir("reports/dna").ok()
            .and_then(|mut d| d.next())
            .and_then(|e| e.ok())
            .map(|e| e.path().to_string_lossy().to_string())
            .unwrap_or_else(|| { eprintln!("no DNA report found"); std::process::exit(1); })
    });
    let threshold: f64 = args.windows(2)
        .find(|w| w[0] == "--threshold")
        .and_then(|w| w[1].parse().ok())
        .unwrap_or(5.0);

    println!("=== SAGCO-SENTINEL-DREAM v1 ===");
    println!("REPORT={}", report_path);
    println!("THRESHOLD={}", threshold);
    println!("SOURCE=Kazusa_DB_Homo_sapiens");
    println!("");

    // Load DNA execution report
    let raw = fs::read_to_string(&report_path).expect("cannot read report");
    let v: Value = serde_json::from_str(&raw).expect("invalid JSON");

    let total_codons = v["codon_count"].as_u64().unwrap_or(0) as f64;
    let actual_dist = v["opcode_distribution"].as_object()
        .map(|o| o.iter().map(|(k, v)| (k.clone(), v.as_f64().unwrap_or(0.0))).collect::<HashMap<_,_>>())
        .unwrap_or_default();

    // Compute expected opcode distribution from human codon frequency
    let codon_freq  = human_codon_freq();
    let codon_opcode = codon_to_opcode();
    let mut expected: HashMap<String, f64> = HashMap::new();

    for (codon, freq) in &codon_freq {
        if let Some(&opcode) = codon_opcode.get(codon) {
            let expected_count = (freq / 1000.0) * total_codons;
            *expected.entry(opcode.to_string()).or_insert(0.0) += expected_count;
        }
    }

    println!("--- DREAM ANALYSIS (expected vs actual) ---");
    println!("{:<16} {:>8} {:>8} {:>8}  SIGNAL", "OPCODE", "EXPECT", "ACTUAL", "DELTA");
    println!("{}", "-".repeat(60));

    let mut alerts: Vec<String> = Vec::new();
    let mut variances: Vec<(String, f64, f64, f64)> = Vec::new();

    let mut all_opcodes: std::collections::HashSet<String> = expected.keys().cloned().collect();
    all_opcodes.extend(actual_dist.keys().cloned());
    let mut sorted: Vec<String> = all_opcodes.into_iter().collect();
    sorted.sort();

    for op in &sorted {
        let exp = expected.get(op).copied().unwrap_or(0.0);
        let act = actual_dist.get(op).copied().unwrap_or(0.0);
        let delta = act - exp;
        let signal = if delta.abs() < threshold { "stable" }
                     else if delta > 0.0 { "OVEREXPRESSED" }
                     else { "underexpressed" };

        println!("{:<16} {:>8.1} {:>8.1} {:>+8.1}  {}", op, exp, act, delta, signal);

        if delta.abs() >= threshold {
            alerts.push(format!("{}={:+.1}", op, delta));
        }
        variances.push((op.clone(), exp, act, delta));
    }

    let timestamp = Utc::now().to_rfc3339();
    let dream_status = if alerts.is_empty() {
        "SAGCO_DREAM_STABLE"
    } else {
        "SAGCO_DREAM_VARIANCE_DETECTED"
    };

    println!("");
    if alerts.is_empty() {
        println!("STATUS=SAGCO_DREAM_STABLE");
        println!("VERDICT=sequence behaves as expected for human genome projection");
    } else {
        println!("ALERTS={}", alerts.len());
        for a in &alerts { println!("  ALERT={}", a); }
        println!("STATUS=SAGCO_DREAM_VARIANCE_DETECTED");
        println!("VERDICT=sequence deviates from human baseline — projection is informative");
    }

    let seal_v = seal(&format!("{}{:.4}", report_path, total_codons));

    // Write report
    let report = json!({
        "opcode":        "SENTINEL_DREAM",
        "timestamp":     timestamp,
        "source_report": report_path,
        "total_codons":  total_codons,
        "threshold":     threshold,
        "codon_table":   "Kazusa_DB_Homo_sapiens",
        "variances":     variances.iter().map(|(op,e,a,d)| json!({"opcode":op,"expected":e,"actual":a,"delta":d})).collect::<Vec<_>>(),
        "alerts":        alerts,
        "seal":          seal_v,
        "status":        dream_status,
    });

    fs::create_dir_all("reports").ok();
    let ts  = Utc::now().format("%Y%m%d_%H%M%S");
    let out = format!("reports/dream_{}.json", ts);
    fs::write(&out, serde_json::to_string_pretty(&report).unwrap()).ok();

    fs::create_dir_all("data").ok();
    if let Ok(mut f) = fs::OpenOptions::new().create(true).append(true).open("data/dream_ledger.jsonl") {
        let _ = f.write_all((serde_json::to_string(&json!({
            "opcode":   "SENTINEL_DREAM",
            "timestamp": timestamp,
            "alerts":   alerts.len(),
            "report":   out,
            "seal":     seal_v,
            "status":   dream_status,
        })).unwrap() + "\n").as_bytes());
    }

    println!("SEAL={}", seal_v);
    println!("REPORT={}", out);
    println!("LEDGER=data/dream_ledger.jsonl");
}
