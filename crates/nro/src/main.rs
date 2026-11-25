//! nro: Nightreign orchestrator CLI. Every command prints JSON (the same the MCP server returns).

use nro::{ops, session};
use std::collections::BTreeMap;

const HELP: &str = "nro - launch and drive an offline nightreign-pvp developer game

  nro env                                pins, relevant processes, current session
  nro preflight [--mod-dist DIR]         stage a run folder and run every safety check (no launch)
  nro launch [--mod-dist DIR] [--world] [--set key=value ...]
                                         preflight, then launch through me3; --world also presses through the title to the Roundtable
  nro status                             session state + the mod's `status`
  nro cmd <console line...>              one mod console command (e.g. nro cmd players)
  nro keys \"ENTER; E*3; wait 2s\"         keyboard input to the game window (focuses it)
  nro navigate title_to_world
  nro shot [name]                        screenshot of the game window into the run folder
  nro log [n] [filter]                   tail of the run's nrpvp.log
  nro stop [--all]                       kill the session's game and me3 (--all: every emulated instance)
  nro launch --emulated N                emulated-Steam instance N (1..4), Steam closed; any command takes --i N
  nro run <scenario.toml> [--mod-dist DIR]  full scenario: stage, launch, steps, report.json
  nro scenarios [--tag X]                list scenarios/*.toml (tags: smoke rules hud mi slow lab)
  nro puppets <n>                        n AFK puppet players (1..5) in the running session (offline mode, in a world)
  nro puppet_ring [radius]               put the puppets on a circle around the player (default 1.3 m)
  nro puppet_front [dist]                put the puppets side by side in front of the player (default 1.2 m)
  nro win <pid|exe name> shot <png>       research: screenshot of any program's biggest window (needs `nro desk`); with several, give the pid
  nro win <pid|exe name> keys \"<seq>\"     research: keys and `click fx fy` (fractions of the client area) into that window
  nro desk                               serve keys/screenshots for other nro processes (run it in a normal terminal)";

fn main() {
    nro::sys::dpi_aware();
    let mut a: Vec<String> = std::env::args().skip(1).collect();
    // `--i N`: act on emulated instance N (default: the one staged last).
    if let Some(p) = a.iter().position(|x| x == "--i") {
        if let Some(n) = a.get(p + 1).cloned() {
            std::env::set_var("NRO_I", n);
            a.drain(p..=p + 1);
        }
    }
    let r = dispatch(&a);
    match r {
        Ok(v) => println!("{}", serde_json::to_string_pretty(&v).unwrap()),
        Err(e) => {
            eprintln!("error: {e}");
            std::process::exit(1);
        }
    }
}

fn flag(a: &[String], name: &str) -> Option<String> {
    a.iter()
        .position(|x| x == name)
        .and_then(|i| a.get(i + 1).cloned())
}

fn sets(a: &[String]) -> Result<BTreeMap<String, toml::Value>, String> {
    let mut out = BTreeMap::new();
    let mut i = 0;
    while i < a.len() {
        if a[i] == "--set" {
            let kv = a.get(i + 1).ok_or("--set needs key=value")?;
            let (k, v) = kv.split_once('=').ok_or("--set needs key=value")?;
            let val: toml::Value = toml::from_str::<toml::Table>(&format!("x = {v}"))
                .ok()
                .and_then(|t| t.get("x").cloned())
                .unwrap_or_else(|| toml::Value::String(v.to_string()));
            out.insert(k.to_string(), val);
            i += 2;
        } else {
            i += 1;
        }
    }
    Ok(out)
}

fn dispatch(a: &[String]) -> ops::R {
    let rest = || a[1..].join(" ");
    match a.first().map(String::as_str) {
        None | Some("help" | "-h" | "--help") => Ok(serde_json::json!(HELP)),
        Some("env") => ops::env(),
        Some("preflight") => ops::preflight(
            flag(a, "--mod-dist"),
            flag(a, "--emulated").and_then(|n| n.parse().ok()),
        ),
        Some("launch") => ops::launch(
            flag(a, "--mod-dist"),
            sets(a)?,
            a.iter().any(|x| x == "--world"),
            flag(a, "--emulated").and_then(|n| n.parse().ok()),
        ),
        Some("status") => ops::status(),
        Some("cmd") => ops::console(&rest()),
        Some("keys") => ops::keys(&rest()),
        Some("navigate") => ops::navigate(a.get(1).map(String::as_str).unwrap_or("title_to_world")),
        Some("shot") => ops::screenshot(a.get(1).map(String::as_str).unwrap_or("shot")),
        Some("log") => ops::log_tail(
            a.get(1).and_then(|n| n.parse().ok()).unwrap_or(40),
            a.get(2).map(String::as_str),
        ),
        Some("stop") if a.iter().any(|x| x == "--all") => ops::stop_all(),
        Some("stop") => ops::stop(),
        Some("run") => ops::run_scenario(
            a.get(1).ok_or("nro run <scenario.toml>")?,
            flag(a, "--mod-dist"),
        ),
        Some("scenarios") => ops::scenarios(flag(a, "--tag").as_deref()),
        Some("puppets") => ops::puppets(a.get(1).and_then(|n| n.parse().ok()).unwrap_or(2)),
        Some("puppet_front") => {
            ops::puppet_front(a.get(1).and_then(|n| n.parse().ok()).unwrap_or(1.2))
        }
        Some("puppet_ring") => {
            ops::puppet_ring(a.get(1).and_then(|n| n.parse().ok()).unwrap_or(1.3))
        }
        Some("win") => ops::window(&a[1..]),
        Some("desk") => nro::desk::serve().map(|_| serde_json::json!("done")),
        Some("watch") => session::watch(std::path::Path::new(a.get(1).ok_or("watch <run dir>")?))
            .map(|_| serde_json::json!("done")),
        Some(other) => Err(format!("unknown command {other:?}\n{HELP}")),
    }
}
