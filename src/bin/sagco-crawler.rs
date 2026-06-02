/// sagco-crawler — Tier-2 HTTP stepper agent (rivals tshark / curl pipelines)
/// Stepwise crawler: fetches a URL, extracts links + metadata, seals response.
/// USE: sagco-crawler <url> [--depth 1] [--json]
use std::fs;
use std::collections::{HashSet, VecDeque};
use sha2::{Digest, Sha256};
use serde_json::json;
use regex::Regex;
use chrono::Utc;

#[derive(serde::Serialize)]
struct CrawlStep {
    url:         String,
    status_code: u16,
    body_bytes:  usize,
    links_found: usize,
    seal:        String,
    tick:        usize,
}

fn extract_links(body: &str, base: &str) -> Vec<String> {
    let re = Regex::new(r#"href=["']([^"']+)["']"#).unwrap();
    re.captures_iter(body)
        .filter_map(|c| c.get(1))
        .map(|m| {
            let href = m.as_str();
            if href.starts_with("http") {
                href.to_string()
            } else if href.starts_with('/') {
                // Resolve against base origin
                let parts: Vec<&str> = base.splitn(4, '/').collect();
                format!("{}//{}{}", parts.get(0).unwrap_or(&""), parts.get(2).unwrap_or(&""), href)
            } else {
                href.to_string()
            }
        })
        .filter(|u| u.starts_with("http"))
        .collect()
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        println!("USE: sagco-crawler <url> [--depth 1] [--json]");
        std::process::exit(1);
    }
    let seed = &args[1];
    let depth: usize = args.windows(2)
        .find(|w| w[0] == "--depth")
        .and_then(|w| w[1].parse().ok())
        .unwrap_or(1);
    let json_mode = args.iter().any(|a| a == "--json");

    println!("=== SAGCO-CRAWLER v1 ===");
    println!("SEED={}", seed);
    println!("MAX_DEPTH={}", depth);

    let mut visited: HashSet<String> = HashSet::new();
    let mut queue: VecDeque<(String, usize)> = VecDeque::new();
    queue.push_back((seed.clone(), 0));

    let mut steps: Vec<CrawlStep> = Vec::new();
    let mut tick = 0usize;

    while let Some((url, current_depth)) = queue.pop_front() {
        if visited.contains(&url) || current_depth > depth { continue; }
        visited.insert(url.clone());
        tick += 1;

        println!("TICK={} URL={} DEPTH={}", tick, url, current_depth);

        let response = minreq::get(&url)
            .with_header("User-Agent", "sagco-crawler/1.0")
            .with_timeout(10)
            .send();

        match response {
            Ok(resp) => {
                let code = resp.status_code as u16;
                let body = resp.as_str().unwrap_or("").to_string();
                let seal = format!("{:x}", Sha256::digest(body.as_bytes()));
                let links = extract_links(&body, &url);
                let link_count = links.len();

                if current_depth < depth {
                    for link in links { queue.push_back((link, current_depth + 1)); }
                }

                steps.push(CrawlStep {
                    url: url.clone(),
                    status_code: code,
                    body_bytes: body.len(),
                    links_found: link_count,
                    seal,
                    tick,
                });
            }
            Err(e) => {
                println!("ANTIBODY=CRAWL_IO_ANTIBODY URL={} ERROR={:?}", url, e);
                steps.push(CrawlStep {
                    url: url.clone(),
                    status_code: 0,
                    body_bytes: 0,
                    links_found: 0,
                    seal: "NONE".to_string(),
                    tick,
                });
            }
        }
    }

    // Chain seal over all step seals
    let chain_input: String = steps.iter().map(|s| s.seal.as_str()).collect::<Vec<_>>().join("");
    let chain_seal = format!("{:x}", Sha256::digest(chain_input.as_bytes()));

    if json_mode {
        let out = json!({
            "sagco_command": "sagco-crawler",
            "timestamp": Utc::now().to_rfc3339(),
            "seed": seed,
            "depth": depth,
            "pages_crawled": steps.len(),
            "chain_seal": chain_seal,
            "steps": steps,
        });
        fs::create_dir_all("reports").ok();
        let ts = Utc::now().format("%Y%m%d_%H%M%S");
        let p = format!("reports/crawler_{}.json", ts);
        fs::write(&p, serde_json::to_string_pretty(&out).unwrap()).ok();
        println!("REPORT={}", p);
    }

    println!("PAGES_CRAWLED={}", steps.len());
    println!("CHAIN_SEAL={}", chain_seal);
    println!("STATUS=SAGCO_CRAWLER_PASS");
}
