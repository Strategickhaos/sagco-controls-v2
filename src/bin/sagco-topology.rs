/// sagco-topology — Opcode graph: reads all ledgers, builds node/edge map
/// Shows how every opcode connects to every other through seals and artifacts.
/// Outputs DOT graph + JSON topology report.
/// USE: sagco-topology [--scope all|agent|gcp|k8s|observe] [--dot] [--json]
use sha2::{Digest, Sha256};
use chrono::Utc;
use serde_json::json;
use std::{env, fs, io::Write, collections::HashMap};

#[derive(serde::Serialize, Clone)]
struct TopoNode {
    id:        String,
    opcode:    String,
    timestamp: String,
    status:    String,
    seal:      String,
    ledger:    String,
}

#[derive(serde::Serialize)]
struct TopoEdge {
    from:     String,
    to:       String,
    relation: String,
}

fn load_ledger(path: &str, ledger_name: &str) -> Vec<TopoNode> {
    let content = fs::read_to_string(path).unwrap_or_default();
    content.lines()
        .filter(|l| !l.trim().is_empty())
        .filter_map(|l| serde_json::from_str::<serde_json::Value>(l).ok())
        .enumerate()
        .map(|(i, v)| {
            let opcode    = v["opcode"].as_str().unwrap_or("UNKNOWN").to_string();
            let timestamp = v["timestamp"].as_str().unwrap_or("").to_string();
            let status    = v["status"].as_str().unwrap_or("").to_string();
            let seal      = v["seal"].as_str().unwrap_or("").to_string();
            let id        = format!("{}:{}:{}", ledger_name, opcode, i);
            TopoNode { id, opcode, timestamp, status, seal, ledger: ledger_name.to_string() }
        })
        .collect()
}

fn build_edges(nodes: &[TopoNode]) -> Vec<TopoEdge> {
    // Build seal → node index for chaining
    let seal_map: HashMap<&str, &str> = nodes.iter()
        .filter(|n| !n.seal.is_empty())
        .map(|n| (n.seal.as_str(), n.id.as_str()))
        .collect();

    let mut edges = Vec::new();

    // Chain edges: seal of node N → id of node N+1 (if prev_seal relationship exists)
    // Also: AGENT → its child opcodes by opcode name
    for (i, node) in nodes.iter().enumerate() {
        // Sequential chain within same ledger
        if i > 0 && nodes[i-1].ledger == node.ledger {
            edges.push(TopoEdge {
                from:     nodes[i-1].id.clone(),
                to:       node.id.clone(),
                relation: "NEXT_IN_CHAIN".to_string(),
            });
        }

        // AGENT orchestrates other opcodes
        if node.opcode == "AGENT" {
            for other in nodes.iter() {
                if other.ledger != node.ledger &&
                   node.timestamp <= other.timestamp &&
                   (other.opcode == "OBSERVE" || other.opcode == "FUZZ" ||
                    other.opcode == "FORECAST" || other.opcode == "CHAINVERIFY" ||
                    other.opcode == "GCP_OBSERVE" || other.opcode == "K8S_OBSERVE")
                {
                    edges.push(TopoEdge {
                        from:     node.id.clone(),
                        to:       other.id.clone(),
                        relation: "ORCHESTRATED".to_string(),
                    });
                }
            }
        }

        // Seal cross-reference
        if let Some(target_id) = seal_map.get(node.seal.as_str()) {
            if *target_id != node.id.as_str() {
                edges.push(TopoEdge {
                    from:     node.id.clone(),
                    to:       target_id.to_string(),
                    relation: "SEAL_MATCH".to_string(),
                });
            }
        }
    }

    edges
}

fn to_dot(nodes: &[TopoNode], edges: &[TopoEdge]) -> String {
    let mut dot = String::from("digraph SAGCO_TOPOLOGY {\n");
    dot.push_str("  rankdir=LR;\n");
    dot.push_str("  node [shape=box, style=filled];\n\n");

    // Node colors by opcode family
    for node in nodes {
        let color = match node.opcode.as_str() {
            "FUZZ"|"BINSCAN"|"EXTRACT"|"HUNT"|"CREEP_WATCH" => "#ffcccc",
            "GUARD"|"OBSERVE"|"FSWALK"|"CHAINVERIFY"|"BASELINE"|"K8S_OBSERVE" => "#cce0ff",
            "TOKENIZE"|"FORECAST"|"VERIFY"|"TIMELINE"|"TOPOOPT" => "#e8ccff",
            "AGENT"    => "#fffacc",
            "GCP_OBSERVE" => "#ccffec",
            "STEPPER"  => "#ffd9b3",
            _          => "#f0f0f0",
        };
        let label = format!("{}\\n{}",
            node.opcode,
            if node.timestamp.len() > 19 { &node.timestamp[..19] } else { &node.timestamp }
        );
        dot.push_str(&format!(
            "  \"{}\" [label=\"{}\", fillcolor=\"{}\"];\n",
            node.id.replace('"', "'"), label, color
        ));
    }

    dot.push('\n');

    // Edges
    for edge in edges {
        let style = match edge.relation.as_str() {
            "NEXT_IN_CHAIN" => "style=solid",
            "ORCHESTRATED"  => "style=dashed, color=gray",
            "SEAL_MATCH"    => "style=dotted, color=blue",
            _               => "style=solid",
        };
        dot.push_str(&format!(
            "  \"{}\" -> \"{}\" [label=\"{}\", {}];\n",
            edge.from.replace('"', "'"),
            edge.to.replace('"', "'"),
            edge.relation,
            style
        ));
    }

    dot.push_str("}\n");
    dot
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let scope    = args.windows(2).find(|w| w[0] == "--scope").map(|w| w[1].as_str()).unwrap_or("all");
    let dot_mode  = args.iter().any(|a| a == "--dot");
    let json_mode = args.iter().any(|a| a == "--json");

    println!("=== SAGCO-TOPOLOGY v1 ===");
    println!("SCOPE={}", scope);

    // Discover ledgers
    let ledger_files: Vec<(String, String)> = match fs::read_dir("data") {
        Ok(entries) => entries.flatten()
            .filter(|e| e.path().extension().map(|x| x == "jsonl").unwrap_or(false))
            .map(|e| {
                let path = e.path().to_string_lossy().to_string();
                let name = e.file_name().to_string_lossy()
                    .trim_end_matches(".jsonl").to_string();
                (path, name)
            })
            .filter(|(_, name)| {
                scope == "all" ||
                (scope == "agent"   && name.contains("agent"))   ||
                (scope == "gcp"     && name.contains("gcp"))     ||
                (scope == "k8s"     && name.contains("k8s"))     ||
                (scope == "observe" && name.contains("observe"))
            })
            .collect(),
        Err(_) => {
            println!("ANTIBODY=NO_DATA_DIR_ANTIBODY");
            println!("STATUS=SAGCO_TOPOLOGY_NO_DATA");
            std::process::exit(2);
        }
    };

    if ledger_files.is_empty() {
        println!("ANTIBODY=NO_LEDGERS_ANTIBODY");
        println!("STATUS=SAGCO_TOPOLOGY_EMPTY");
        std::process::exit(2);
    }

    // Load all nodes
    let mut all_nodes: Vec<TopoNode> = Vec::new();
    for (path, name) in &ledger_files {
        let nodes = load_ledger(path, name);
        println!("LEDGER={} NODES={}", name, nodes.len());
        all_nodes.extend(nodes);
    }

    // Sort by timestamp
    all_nodes.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));

    // Build edges
    let edges = build_edges(&all_nodes);

    println!("TOTAL_NODES={}", all_nodes.len());
    println!("TOTAL_EDGES={}", edges.len());

    let timestamp = Utc::now().to_rfc3339();
    let seal_input = format!("{}{}", all_nodes.len(), edges.len());
    let seal = format!("{:x}", Sha256::digest(seal_input.as_bytes()));

    fs::create_dir_all("reports/topology").ok();
    fs::create_dir_all("data").ok();
    let ts = Utc::now().format("%Y%m%d_%H%M%S");

    // DOT output
    if dot_mode {
        let dot = to_dot(&all_nodes, &edges);
        let dot_path = format!("reports/topology/topology_{}.dot", ts);
        fs::write(&dot_path, &dot).ok();
        println!("DOT_REPORT={}", dot_path);
        println!("RENDER=dot -Tpng {} -o topology.png", dot_path);
    }

    if json_mode {
        let report = json!({
            "opcode":      "TOPOLOGY",
            "timestamp":   timestamp,
            "scope":       scope,
            "ledgers":     ledger_files.len(),
            "node_count":  all_nodes.len(),
            "edge_count":  edges.len(),
            "nodes":       all_nodes,
            "edges":       edges,
            "seal":        seal,
            "status":      "SAGCO_TOPOLOGY_PASS",
        });
        let report_text = serde_json::to_string_pretty(&report).unwrap();
        let json_path = format!("reports/topology/topology_{}.json", ts);
        fs::write(&json_path, &report_text).ok();
        println!("REPORT={}", json_path);
    }

    // Ledger entry
    let ledger_line = serde_json::to_string(&json!({
        "opcode":    "TOPOLOGY",
        "timestamp": Utc::now().to_rfc3339(),
        "scope":     scope,
        "nodes":     all_nodes.len(),
        "edges":     edges.len(),
        "seal":      seal,
        "status":    "SAGCO_TOPOLOGY_PASS",
    })).unwrap() + "\n";
    if let Ok(mut f) = fs::OpenOptions::new()
        .create(true).append(true).open("data/topology_ledger.jsonl")
    {
        let _ = f.write_all(ledger_line.as_bytes());
    }

    println!("SEAL={}", seal);
    println!("STATUS=SAGCO_TOPOLOGY_PASS");
}
