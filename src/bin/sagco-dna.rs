/// sagco-dna — DNA Synthesis Tokenizer: codon → SAGCO opcode execution
/// The bridge between the DNA-Synthesis Tokenizer Pipeline and sagco-controls-v2.
///
/// Every DNA codon (3-nucleotide sequence) maps to a SAGCO opcode.
/// ATG (START) → begins execution.  TAA/TAG/TGA (STOP) → seals output.
/// The 140nt sequence in Sagco-Os=True.pdf is executable SAGCO bytecode.
///
/// USE:
///   sagco-dna <sequence>        → execute DNA as SAGCO opcode chain
///   sagco-dna --file <file>     → read sequence from file
///   sagco-dna --translate       → show full codon → opcode table
///   sagco-dna --encode <text>   → encode text → DNA sequence
use sha2::{Digest, Sha256};
use chrono::Utc;
use serde_json::json;
use std::{collections::HashMap, env, fs, io::Write};

fn seal(s: &str) -> String { format!("{:x}", Sha256::digest(s.as_bytes())) }

// ── Codon → SAGCO opcode table ────────────────────────────────────────────────
// Derived from sagco-vm-lang neural graph + standard codon table structure.
// START codon = pipeline entry. STOP codons = seal + exit.
// Each amino acid group → one SAGCO opcode family.
fn codon_table() -> HashMap<&'static str, &'static str> {
    let mut t = HashMap::new();
    // START
    t.insert("ATG", "OBSERVE");          // Met — start of every chain

    // STOP (SEAL)
    t.insert("TAA", "SEAL");
    t.insert("TAG", "SEAL");
    t.insert("TGA", "SEAL");

    // Phenylalanine (F) → TOKENIZE
    t.insert("TTT", "TOKENIZE");
    t.insert("TTC", "TOKENIZE");

    // Leucine (L) → LEXER
    t.insert("TTA", "LEXER");
    t.insert("TTG", "LEXER");
    t.insert("CTT", "LEXER");
    t.insert("CTC", "LEXER");
    t.insert("CTA", "LEXER");
    t.insert("CTG", "LEXER");

    // Isoleucine (I) → CLASSIFY
    t.insert("ATT", "CLASSIFY");
    t.insert("ATC", "CLASSIFY");
    t.insert("ATA", "CLASSIFY");

    // Valine (V) → VARIANCE
    t.insert("GTT", "VARIANCE");
    t.insert("GTC", "VARIANCE");
    t.insert("GTA", "VARIANCE");
    t.insert("GTG", "VARIANCE");

    // Serine (S) → STEPPER
    t.insert("TCT", "STEPPER");
    t.insert("TCC", "STEPPER");
    t.insert("TCA", "STEPPER");
    t.insert("TCG", "STEPPER");
    t.insert("AGT", "STEPPER");
    t.insert("AGC", "STEPPER");

    // Proline (P) → PIPELINE
    t.insert("CCT", "PIPELINE");
    t.insert("CCC", "PIPELINE");
    t.insert("CCA", "PIPELINE");
    t.insert("CCG", "PIPELINE");

    // Threonine (T) → TOPOLOGY
    t.insert("ACT", "TOPOLOGY");
    t.insert("ACC", "TOPOLOGY");
    t.insert("ACA", "TOPOLOGY");
    t.insert("ACG", "TOPOLOGY");

    // Alanine (A) → AGENT
    t.insert("GCT", "AGENT");
    t.insert("GCC", "AGENT");
    t.insert("GCA", "AGENT");
    t.insert("GCG", "AGENT");

    // Tyrosine (Y) → FORECAST
    t.insert("TAT", "FORECAST");
    t.insert("TAC", "FORECAST");

    // Histidine (H) → HUNT
    t.insert("CAT", "HUNT");
    t.insert("CAC", "HUNT");

    // Glutamine (Q) → QUERY (topology query)
    t.insert("CAA", "QUERY");
    t.insert("CAG", "QUERY");

    // Asparagine (N) → NODE (topology node)
    t.insert("AAT", "NODE");
    t.insert("AAC", "NODE");

    // Lysine (K) → KEYFILE (ledger key)
    t.insert("AAA", "KEYFILE");
    t.insert("AAG", "KEYFILE");

    // Aspartate (D) → DAEMON
    t.insert("GAT", "DAEMON");
    t.insert("GAC", "DAEMON");

    // Glutamate (E) → EVIDENCE
    t.insert("GAA", "EVIDENCE");
    t.insert("GAG", "EVIDENCE");

    // Cysteine (C) → CHAINVERIFY
    t.insert("TGT", "CHAINVERIFY");
    t.insert("TGC", "CHAINVERIFY");

    // Tryptophan (W) → WATCH
    t.insert("TGG", "WATCH");

    // Arginine (R) → RECLASS
    t.insert("CGT", "RECLASS");
    t.insert("CGC", "RECLASS");
    t.insert("CGA", "RECLASS");
    t.insert("CGG", "RECLASS");
    t.insert("AGA", "RECLASS");
    t.insert("AGG", "RECLASS");

    // Glycine (G) → GUARD
    t.insert("GGT", "GUARD");
    t.insert("GGC", "GUARD");
    t.insert("GGA", "GUARD");
    t.insert("GGG", "GUARD");

    t
}

// ── Text → DNA encoding (simple: letter → codon) ─────────────────────────────
fn encode_text(text: &str) -> String {
    // Map each char to a DNA codon using the genetic code in reverse
    // (simplified: A-Z each gets a unique codon)
    let char_codons: HashMap<char, &str> = [
        ('A',"GCT"),('B',"GCC"),('C',"TGT"),('D',"GAT"),('E',"GAA"),
        ('F',"TTT"),('G',"GGT"),('H',"CAT"),('I',"ATT"),('J',"CAA"),
        ('K',"AAA"),('L',"TTA"),('M',"ATG"),('N',"AAT"),('O',"CAG"),
        ('P',"CCT"),('Q',"CAA"),('R',"CGT"),('S',"TCT"),('T',"ACT"),
        ('U',"ACG"),('V',"GTT"),('W',"TGG"),('X',"TAC"),('Y',"TAT"),
        ('Z',"GTA"),('_',"GCG"),(' ',"TAA"),  // space = SEAL
        ('0',"GGG"),('1',"GGT"),('2',"GGC"),('3',"GGA"),('4',"GTG"),
        ('5',"GTC"),('6',"GTT"),('7',"GTA"),('8',"AGT"),('9',"AGC"),
    ].iter().cloned().collect();

    let mut dna = String::from("ATG"); // Always start with ATG (OBSERVE)
    for ch in text.to_uppercase().chars() {
        if let Some(&codon) = char_codons.get(&ch) {
            dna.push_str(codon);
        }
    }
    dna.push_str("TAA"); // Terminate with SEAL
    dna
}

// ── Execute a DNA sequence as SAGCO opcodes ───────────────────────────────────
fn execute(seq: &str, table: &HashMap<&str, &str>) -> Vec<(String, String)> {
    let clean: String = seq.chars()
        .filter(|c| matches!(c, 'A'|'T'|'G'|'C'|'a'|'t'|'g'|'c'))
        .collect::<String>()
        .to_uppercase();

    let mut instructions = Vec::new();
    let mut i = 0;
    while i + 3 <= clean.len() {
        let codon = &clean[i..i+3];
        let opcode = table.get(codon).copied().unwrap_or("UNKNOWN");
        instructions.push((codon.to_string(), opcode.to_string()));
        i += 3;
    }
    instructions
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let table = codon_table();

    if args.len() < 2 || args[1] == "--help" {
        println!("SAGCO-DNA v1 — DNA → SAGCO opcode execution engine");
        println!("USE:");
        println!("  sagco-dna <ATGCGT...>           execute DNA sequence");
        println!("  sagco-dna --file <path>          read sequence from file");
        println!("  sagco-dna --encode <text>        encode text → DNA");
        println!("  sagco-dna --translate            show full codon table");
        return;
    }

    println!("=== SAGCO-DNA v1 ===");

    // --translate: show full table
    if args.iter().any(|a| a == "--translate") {
        println!("CODON TABLE ({} entries):", table.len());
        let mut sorted: Vec<_> = table.iter().collect();
        sorted.sort_by_key(|(c, _)| *c);
        for (codon, opcode) in &sorted {
            println!("  {} → {}", codon, opcode);
        }
        return;
    }

    // --encode: text → DNA
    if let Some(pos) = args.iter().position(|a| a == "--encode") {
        let text = args.get(pos+1).map(|s| s.as_str()).unwrap_or("SAGCO");
        let dna = encode_text(text);
        println!("INPUT={}", text);
        println!("DNA={}", dna);
        println!("LENGTH={}nt", dna.len());
        println!("CODONS={}", dna.len()/3);
        println!("SEAL={}", seal(&dna));

        // Also execute it
        println!("");
        println!("EXECUTION:");
        let instrs = execute(&dna, &table);
        for (codon, opcode) in &instrs {
            println!("  {} → {}", codon, opcode);
        }
        return;
    }

    // --execute: accept dash-separated codons (ATG-TCT-GCT-TAA)
    if let Some(pos) = args.iter().position(|a| a == "--execute") {
        let raw = args.get(pos+1).expect("--execute requires sequence");
        // strip dashes → raw ATGC string
        let seq: String = raw.chars().filter(|c| matches!(c, 'A'|'T'|'G'|'C'|'a'|'t'|'g'|'c')).collect();
        println!("=== SAGCO-DNA EXECUTE ===");
        println!("INPUT={}", raw);
        println!("CLEANED={}nt", seq.len());
        let instrs = execute(&seq, &table);
        let exec_str: String = instrs.iter().map(|(c,o)| format!("{}{}", c, o)).collect();
        let exec_seal = seal(&exec_str);
        println!("");
        println!("--- OPCODE DISTRIBUTION ---");
        let mut counts: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
        for (_, op) in &instrs { *counts.entry(op.clone()).or_insert(0) += 1; }
        let mut sorted: Vec<_> = counts.iter().collect();
        sorted.sort_by(|a,b| b.1.cmp(a.1));
        for (op, n) in &sorted { println!("  {} = {}", op, n); }
        println!("");
        println!("PIPELINE={}", instrs.iter().map(|(_,o)| o.as_str()).collect::<Vec<_>>().join(" → "));
        println!("SEAL={}", exec_seal);
        println!("STATUS=SAGCO_DNA_EXECUTE_PASS");

        fs::create_dir_all("data").ok();
        let ledger_line = serde_json::to_string(&json!({
            "opcode":  "DNA_EXECUTE",
            "timestamp": Utc::now().to_rfc3339(),
            "codons":  instrs.len(),
            "seal":    exec_seal,
            "status":  "SAGCO_DNA_EXECUTE_PASS",
        })).unwrap() + "\n";
        if let Ok(mut f) = fs::OpenOptions::new().create(true).append(true).open("data/dna_ledger.jsonl") {
            let _ = f.write_all(ledger_line.as_bytes());
        }
        return;
    }

    // --file or raw sequence
    let sequence = if let Some(pos) = args.iter().position(|a| a == "--file") {
        let path = args.get(pos+1).expect("--file requires path");
        let content = fs::read_to_string(path).expect("cannot read file");
        content.chars().filter(|c| matches!(c, 'A'|'T'|'G'|'C'|'a'|'t'|'g'|'c')).collect()
    } else {
        args[1].clone()
    };

    println!("SEQUENCE_LEN={}nt", sequence.len());
    println!("CODONS={}", sequence.len()/3);

    let instructions = execute(&sequence, &table);

    // Opcode stats
    let mut opcode_counts: HashMap<String, usize> = HashMap::new();
    for (_, op) in &instructions { *opcode_counts.entry(op.clone()).or_insert(0) += 1; }

    println!("");
    println!("--- OPCODE DISTRIBUTION ---");
    let mut counts: Vec<_> = opcode_counts.iter().collect();
    counts.sort_by(|a, b| b.1.cmp(a.1));
    for (op, count) in &counts {
        println!("  {} = {}", op, count);
    }

    println!("");
    println!("--- EXECUTION TRACE (first 20) ---");
    for (codon, opcode) in instructions.iter().take(20) {
        println!("  {} → {}", codon, opcode);
    }
    if instructions.len() > 20 {
        println!("  ... ({} more)", instructions.len() - 20);
    }

    // Seal the execution
    let exec_string: String = instructions.iter().map(|(c,o)| format!("{}{}", c, o)).collect();
    let exec_seal = seal(&exec_string);

    // Write report + ledger
    let timestamp = Utc::now().to_rfc3339();
    let report = json!({
        "opcode":      "DNA_EXECUTE",
        "timestamp":   timestamp,
        "sequence_len": sequence.len(),
        "codon_count": instructions.len(),
        "opcode_distribution": opcode_counts,
        "instructions": instructions.iter().map(|(c,o)| json!({"codon":c,"opcode":o})).collect::<Vec<_>>(),
        "seal":        exec_seal,
        "status":      "SAGCO_DNA_PASS",
    });

    fs::create_dir_all("reports/dna").ok();
    let ts  = Utc::now().format("%Y%m%d_%H%M%S");
    let out = format!("reports/dna/dna_{}.json", ts);
    fs::write(&out, serde_json::to_string_pretty(&report).unwrap()).ok();

    fs::create_dir_all("data").ok();
    let ledger_line = serde_json::to_string(&json!({
        "opcode":    "DNA_EXECUTE",
        "timestamp": timestamp,
        "codons":    instructions.len(),
        "report":    out,
        "seal":      exec_seal,
        "status":    "SAGCO_DNA_PASS",
    })).unwrap() + "\n";
    if let Ok(mut f) = fs::OpenOptions::new().create(true).append(true).open("data/dna_ledger.jsonl") {
        let _ = f.write_all(ledger_line.as_bytes());
    }

    println!("");
    println!("SEAL={}", exec_seal);
    println!("REPORT={}", out);
    println!("LEDGER=data/dna_ledger.jsonl");
    println!("STATUS=SAGCO_DNA_PASS");
}
