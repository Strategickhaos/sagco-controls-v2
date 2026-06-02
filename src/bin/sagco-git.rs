/// sagco-git — Git + GitHub agent: create repos, clone, push, status
/// Wraps GitHub REST API via minreq + local git operations.
/// USE:
///   sagco-git create  <repo-name> [--private] [--token TOKEN]
///   sagco-git push    [--token TOKEN] [--remote origin]
///   sagco-git clone   <owner/repo> [--token TOKEN]
///   sagco-git status
///   sagco-git list    [--token TOKEN]
use std::{env, process::Command};
use sha2::{Digest, Sha256};
use chrono::Utc;
use serde_json::json;

fn seal(s: &str) -> String { format!("{:x}", Sha256::digest(s.as_bytes())) }

fn token_from_args(args: &[String]) -> String {
    args.windows(2)
        .find(|w| w[0] == "--token")
        .map(|w| w[1].clone())
        .or_else(|| std::env::var("GITHUB_TOKEN").ok())
        .or_else(|| std::env::var("GH_TOKEN").ok())
        .unwrap_or_default()
}

fn git(args: &[&str]) -> (i32, String) {
    match Command::new("git").args(args).output() {
        Ok(o) => {
            let out = String::from_utf8_lossy(&o.stdout).to_string()
                + &String::from_utf8_lossy(&o.stderr);
            (o.status.code().unwrap_or(-1), out.trim().to_string())
        }
        Err(e) => (-1, format!("GIT_NOT_FOUND={}", e)),
    }
}

fn github_post(path: &str, body: &str, token: &str) -> (u16, String) {
    match minreq::post(format!("https://api.github.com{}", path))
        .with_header("Authorization", format!("Bearer {}", token))
        .with_header("Accept", "application/vnd.github+json")
        .with_header("X-GitHub-Api-Version", "2022-11-28")
        .with_header("User-Agent", "sagco-git/1.0")
        .with_header("Content-Type", "application/json")
        .with_body(body)
        .with_timeout(15)
        .send()
    {
        Ok(r) => (r.status_code as u16, r.as_str().unwrap_or("").to_string()),
        Err(e) => (0, format!("REQUEST_ERROR={:?}", e)),
    }
}

fn github_get(path: &str, token: &str) -> (u16, String) {
    match minreq::get(format!("https://api.github.com{}", path))
        .with_header("Authorization", format!("Bearer {}", token))
        .with_header("Accept", "application/vnd.github+json")
        .with_header("X-GitHub-Api-Version", "2022-11-28")
        .with_header("User-Agent", "sagco-git/1.0")
        .with_timeout(15)
        .send()
    {
        Ok(r) => (r.status_code as u16, r.as_str().unwrap_or("").to_string()),
        Err(e) => (0, format!("REQUEST_ERROR={:?}", e)),
    }
}

fn cmd_create(args: &[String]) {
    let name     = args.get(2).cloned().unwrap_or_else(|| "sagco-repo".to_string());
    let private  = args.iter().any(|a| a == "--private");
    let token    = token_from_args(args);

    println!("=== SAGCO-GIT CREATE ===");
    println!("REPO={}", name);
    println!("PRIVATE={}", private);

    if token.is_empty() {
        println!("ANTIBODY=NO_TOKEN_ANTIBODY");
        println!("FIX=set env GITHUB_TOKEN or pass --token YOUR_PAT");
        std::process::exit(2);
    }

    let body = serde_json::to_string(&json!({
        "name":        name,
        "description": "SAGCO: Sovereign Agent Grid Compute Runtime",
        "private":     private,
        "auto_init":   false,
    })).unwrap();

    let (code, resp) = github_post("/user/repos", &body, &token);
    let v: serde_json::Value = serde_json::from_str(&resp).unwrap_or(json!({}));

    if code == 201 {
        let clone_url = v["clone_url"].as_str().unwrap_or("");
        let html_url  = v["html_url"].as_str().unwrap_or("");
        println!("REPO_CREATED={}", html_url);
        println!("CLONE_URL={}", clone_url);
        println!("SSH_URL={}", v["ssh_url"].as_str().unwrap_or(""));

        // Auto-set remote if we're in a git repo
        let (rc, _) = git(&["remote", "get-url", "origin"]);
        if rc != 0 {
            let (rr, ro) = git(&["remote", "add", "origin", clone_url]);
            println!("REMOTE_ADDED={} {}", rr, ro);
        } else {
            println!("REMOTE_EXISTS=origin (use --force to overwrite)");
        }

        let seal_v = seal(&format!("{}{}", name, Utc::now().to_rfc3339()));
        println!("SEAL={}", seal_v);
        println!("STATUS=SAGCO_GIT_CREATE_PASS");
    } else {
        let msg = v["message"].as_str().unwrap_or(&resp);
        println!("HTTP={}", code);
        println!("ERROR={}", msg);
        println!("ANTIBODY=GITHUB_CREATE_ANTIBODY");
        println!("STATUS=SAGCO_GIT_CREATE_FAIL");
        std::process::exit(3);
    }
}

fn cmd_push(args: &[String]) {
    let remote  = args.windows(2).find(|w| w[0] == "--remote").map(|w| w[1].as_str()).unwrap_or("origin");
    let branch  = args.windows(2).find(|w| w[0] == "--branch").map(|w| w[1].as_str()).unwrap_or("master");

    println!("=== SAGCO-GIT PUSH ===");
    println!("REMOTE={} BRANCH={}", remote, branch);

    // Seal the current HEAD commit
    let (_, head) = git(&["rev-parse", "HEAD"]);
    println!("HEAD={}", head.chars().take(12).collect::<String>());

    let (code, out) = git(&["push", "-u", remote, branch]);
    println!("{}", out);

    if code == 0 {
        println!("SEAL={}", seal(&head));
        println!("STATUS=SAGCO_GIT_PUSH_PASS");
    } else {
        println!("ANTIBODY=GIT_PUSH_ANTIBODY");
        println!("STATUS=SAGCO_GIT_PUSH_FAIL");
        std::process::exit(3);
    }
}

fn cmd_clone(args: &[String]) {
    let repo = args.get(2).cloned().unwrap_or_default();
    let token = token_from_args(args);

    println!("=== SAGCO-GIT CLONE ===");
    println!("REPO={}", repo);

    let url = if token.is_empty() {
        format!("https://github.com/{}.git", repo)
    } else {
        format!("https://{}@github.com/{}.git", token, repo)
    };

    let dir = repo.split('/').last().unwrap_or(&repo).to_string();
    let (code, out) = git(&["clone", &url, &dir]);
    println!("{}", out);

    if code == 0 {
        println!("CLONED={}", dir);
        println!("STATUS=SAGCO_GIT_CLONE_PASS");
    } else {
        println!("ANTIBODY=GIT_CLONE_ANTIBODY");
        println!("STATUS=SAGCO_GIT_CLONE_FAIL");
        std::process::exit(3);
    }
}

fn cmd_status() {
    println!("=== SAGCO-GIT STATUS ===");

    let (_, branch) = git(&["branch", "--show-current"]);
    let (_, head)   = git(&["rev-parse", "--short", "HEAD"]);
    let (_, remote) = git(&["remote", "-v"]);
    let (_, log)    = git(&["log", "--oneline", "-5"]);
    let (_, status) = git(&["status", "--short"]);

    println!("BRANCH={}", branch);
    println!("HEAD={}", head);
    println!("");
    println!("--- REMOTES ---");
    println!("{}", remote);
    println!("");
    println!("--- LAST 5 COMMITS ---");
    println!("{}", log);
    println!("");
    println!("--- WORKING TREE ---");
    if status.is_empty() { println!("CLEAN=true") } else { println!("{}", status) }
    println!("");
    println!("SEAL={}", seal(&head));
    println!("STATUS=SAGCO_GIT_STATUS_PASS");
}

fn cmd_list(args: &[String]) {
    let token = token_from_args(args);
    println!("=== SAGCO-GIT LIST (your repos) ===");

    if token.is_empty() {
        println!("ANTIBODY=NO_TOKEN_ANTIBODY");
        std::process::exit(2);
    }

    let (code, resp) = github_get("/user/repos?per_page=30&sort=updated", &token);
    if code == 200 {
        let repos: Vec<serde_json::Value> = serde_json::from_str(&resp).unwrap_or_default();
        for r in &repos {
            println!("REPO={} PRIVATE={} LANG={}",
                r["full_name"].as_str().unwrap_or("?"),
                r["private"].as_bool().unwrap_or(false),
                r["language"].as_str().unwrap_or("?"),
            );
        }
        println!("TOTAL={}", repos.len());
        println!("STATUS=SAGCO_GIT_LIST_PASS");
    } else {
        println!("HTTP={} ERROR={}", code, resp);
        println!("STATUS=SAGCO_GIT_LIST_FAIL");
        std::process::exit(3);
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();
    let sub = args.get(1).map(|s| s.as_str()).unwrap_or("status");

    match sub {
        "create" => cmd_create(&args),
        "push"   => cmd_push(&args),
        "clone"  => cmd_clone(&args),
        "status" => cmd_status(),
        "list"   => cmd_list(&args),
        _ => {
            println!("SAGCO-GIT v1");
            println!("USE:");
            println!("  sagco-git create  <name>  [--private] [--token TOKEN]");
            println!("  sagco-git push    [--remote origin] [--branch master]");
            println!("  sagco-git clone   <owner/repo> [--token TOKEN]");
            println!("  sagco-git list    [--token TOKEN]");
            println!("  sagco-git status");
            println!("");
            println!("TOKEN: set env GITHUB_TOKEN or pass --token");
        }
    }
}
