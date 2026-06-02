/// sagco-hunt — Tier-4 pattern hunter (rivals YARA CLI)
/// Loads a rules file (one regex per line), scans files for matches, emits hit report.
/// USE: sagco-hunt <rules_file> <target_path> [--json]
///
/// Rules file format (one per line):
///   # comment
///   RULE_NAME: <regex_pattern>
use std::fs;
use regex::Regex;
use serde_json::json;
use sha2::{Digest, Sha256};
use chrono::Utc;

#[derive(serde::Serialize)]
struct HitRecord {
    rule:    String,
    file:    String,
    line_no: usize,
    excerpt: String,
}

struct Rule {
    name:    String,
    pattern: Regex,
}

fn load_rules(path: &str) -> Vec<Rule> {
    let text = fs::read_to_string(path).unwrap_or_default();
    text.lines()
        .filter(|l| !l.trim_start().starts_with('#') && l.contains(':'))
        .filter_map(|l| {
            let parts: Vec<&str> = l.splitn(2, ':').collect();
            if parts.len() < 2 { return None; }
            let re = Regex::new(parts[1].trim()).ok()?;
            Some(Rule { name: parts[0].trim().to_string(), pattern: re })
        })
        .collect()
}

fn scan_file(path: &str, rules: &[Rule]) -> Vec<HitRecord> {
    let text = match fs::read_to_string(path) {
        Ok(t) => t,
        Err(_) => return vec![],
    };
    let mut hits = Vec::new();
    for (line_no, line) in text.lines().enumerate() {
        for rule in rules {
            if rule.pattern.is_match(line) {
                hits.push(HitRecord {
                    rule: rule.name.clone(),
                    file: path.to_string(),
                    line_no: line_no + 1,
                    excerpt: line.chars().take(120).collect(),
                });
            }
        }
    }
    hits
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        println!("USE: sagco-hunt <rules_file> <target_path> [--json]");
        println!("Rules file: one rule per line  →  RULE_NAME: regex_pattern");
        std::process::exit(1);
    }
    let rules_path  = &args[1];
    let target_path = &args[2];
    let json_mode   = args.iter().any(|a| a == "--json");

    println!("=== SAGCO-HUNT v1 ===");
    println!("RULES={}", rules_path);
    println!("TARGET={}", target_path);

    let rules = load_rules(rules_path);
    if rules.is_empty() {
        println!("ANTIBODY=NO_RULES_ANTIBODY");
        println!("STATUS=SAGCO_HUNT_NO_RULES");
        std::process::exit(2);
    }
    println!("RULES_LOADED={}", rules.len());

    let files: Vec<String> = if std::path::Path::new(target_path).is_dir() {
        walkdir::WalkDir::new(target_path)
            .into_iter().flatten()
            .filter(|e| e.metadata().map(|m| m.is_file()).unwrap_or(false))
            .map(|e| e.path().to_string_lossy().to_string())
            .collect()
    } else {
        vec![target_path.clone()]
    };

    let mut all_hits: Vec<HitRecord> = Vec::new();
    for f in &files {
        let hits = scan_file(f, &rules);
        all_hits.extend(hits);
    }

    let seal_input: String = all_hits.iter().map(|h| format!("{}{}{}", h.rule, h.file, h.line_no)).collect();
    let seal = format!("{:x}", Sha256::digest(seal_input.as_bytes()));

    let status = if all_hits.is_empty() { "SAGCO_HUNT_CLEAN" } else { "SAGCO_HUNT_HITS_DETECTED" };

    if json_mode {
        let out = json!({
            "sagco_command": "sagco-hunt",
            "timestamp": Utc::now().to_rfc3339(),
            "rules_file": rules_path,
            "target": target_path,
            "files_scanned": files.len(),
            "total_hits": all_hits.len(),
            "seal": seal,
            "status": status,
            "hits": all_hits,
        });
        fs::create_dir_all("reports").ok();
        let ts = Utc::now().format("%Y%m%d_%H%M%S");
        let p = format!("reports/hunt_{}.json", ts);
        fs::write(&p, serde_json::to_string_pretty(&out).unwrap()).ok();
        println!("REPORT={}", p);
    } else {
        for h in &all_hits {
            println!("HIT rule={} file={} line={}", h.rule, h.file, h.line_no);
        }
    }

    println!("FILES_SCANNED={}", files.len());
    println!("TOTAL_HITS={}", all_hits.len());
    println!("SEAL={}", seal);
    println!("STATUS={}", status);
}
