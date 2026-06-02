use std::process::Command;

fn run_case(name: &str, args: &[&str]) -> bool {
    println!("=== CASE: {} ===", name);

    let out = Command::new("cargo")
        .args(["run", "--quiet", "--bin", "sagco-reclass", "--"])
        .args(args)
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("failed to run sagco-reclass");

    let stdout = String::from_utf8_lossy(&out.stdout);
    println!("{}", stdout);

    stdout.contains("STATUS=")
}

fn main() {
    println!("=== SAGCO TOPOLOGY OPTIMIZATION FUZZ ===");

    let cases: Vec<(&str, Vec<&str>)> = vec![
        ("baseline_reclass",  vec!["1011.6", "68.4",  "851.6", "228.4", "4", "10"]),
        ("scope_creep",       vec!["1011.6", "68.4",  "851.6", "388.4", "4", "10"]),
        ("scope_reduction",   vec!["1011.6", "68.4",  "851.6", "100.0", "4", "10"]),
        ("no_change",         vec!["851.6",  "228.4", "851.6", "228.4", "4", "10"]),
        ("zero_capacity",     vec!["1011.6", "68.4",  "851.6", "228.4", "0", "10"]),
    ];

    let mut pass = 0usize;
    let mut fail = 0usize;

    for (name, args) in &cases {
        if run_case(name, args) {
            pass += 1;
        } else {
            fail += 1;
        }
    }

    println!("---");
    println!("TOPOLOGY_CASES_PASS={}", pass);
    println!("TOPOLOGY_CASES_FAIL={}", fail);

    // Verify the ledger chain accumulated across all cases
    let verify = Command::new("cargo")
        .args(["run", "--quiet", "--bin", "sagco-reclass", "--", "--verify-chain"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output();

    match verify {
        Ok(out) => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            println!("{}", stdout);
            if stdout.contains("SAGCO_CHAIN_VERIFIED") {
                println!("STATUS=SAGCO_TOPOFUZZ_PASS");
            } else {
                println!("STATUS=SAGCO_TOPOFUZZ_CHAIN_VARIANCE");
                std::process::exit(3);
            }
        }
        Err(e) => {
            println!("ERROR={}", e);
            println!("STATUS=SAGCO_TOPOFUZZ_VERIFY_FAIL");
            std::process::exit(2);
        }
    }
}
