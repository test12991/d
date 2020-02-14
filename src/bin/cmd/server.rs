// Copyright 2020 The EPIC Developers
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

/// EPIC server commands processing
use std::process::exit;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use clap::ArgMatches;
use ctrlc;

use crate::config::GlobalConfig;
use crate::core::global;
use crate::p2p::{PeerAddr, Seeding};
use crate::servers;
use crate::tui::ui;
use epic_util::logger::LogEntry;
use std::sync::mpsc;

/// wrap below to allow UI to clean up on stop
pub fn start_server(config: servers::ServerConfig, logs_rx: Option<mpsc::Receiver<LogEntry>>) {
	start_server_tui(config, logs_rx);
	// Just kill process for now, otherwise the process
	// hangs around until sigint because the API server
	// currently has no shutdown facility
	exit(0);
}

fn start_server_tui(config: servers::ServerConfig, logs_rx: Option<mpsc::Receiver<LogEntry>>) {
	// Run the UI controller.. here for now for simplicity to access
	// everything it might need
	if config.run_tui.unwrap_or(false) {
		warn!("Starting EPIC in UI mode...");
		servers::Server::start(
			config,
			logs_rx,
			|serv: servers::Server, logs_rx: Option<mpsc::Receiver<LogEntry>>| {
				let mut controller = ui::Controller::new(logs_rx.unwrap()).unwrap_or_else(|e| {
					panic!("Error loading UI controller: {}", e);
				});
				controller.run(serv);
			},
		)
		.unwrap();
	} else {
		warn!("Starting EPIC w/o UI...");
		servers::Server::start(
			config,
			logs_rx,
			|serv: servers::Server, _: Option<mpsc::Receiver<LogEntry>>| {
				let running = Arc::new(AtomicBool::new(true));
				let r = running.clone();
				ctrlc::set_handler(move || {
					r.store(false, Ordering::SeqCst);
				})
				.expect("Error setting handler for both SIGINT (Ctrl+C) and SIGTERM (kill)");
				while running.load(Ordering::SeqCst) {
					thread::sleep(Duration::from_secs(1));
				}
				warn!("Received SIGINT (Ctrl+C) or SIGTERM (kill).");
				serv.stop();
			},
		)
		.unwrap();
	}
}

/// Handles the server part of the command line, mostly running, starting and
/// stopping the EPIC blockchain server. Processes all the command line
/// arguments to build a proper configuration and runs EPIC with that
/// configuration.
pub fn server_command(
	server_args: Option<&ArgMatches<'_>>,
	mut global_config: GlobalConfig,
	logs_rx: Option<mpsc::Receiver<LogEntry>>,
) -> i32 {
	global::set_mining_mode(
		global_config
			.members
			.as_mut()
			.unwrap()
			.server
			.clone()
			.chain_type,
	);

	// just get defaults from the global config
	let mut server_config = global_config.members.as_ref().unwrap().server.clone();

	if let Some(a) = server_args {
		if let Some(port) = a.value_of("port") {
			server_config.p2p_config.port = port.parse().unwrap();
		}

		if let Some(api_port) = a.value_of("api_port") {
			let default_ip = "0.0.0.0";
			server_config.api_http_addr = format!("{}:{}", default_ip, api_port);
		}

		if let Some(wallet_url) = a.value_of("wallet_url") {
			server_config
				.stratum_mining_config
				.as_mut()
				.unwrap()
				.wallet_listener_url = wallet_url.to_string();
		}

		if let Some(seeds) = a.values_of("seed") {
			let seed_addrs = seeds
				.filter_map(|x| x.parse().ok())
				.map(|x| PeerAddr(x))
				.collect();
			server_config.p2p_config.seeding_type = Seeding::List;
			server_config.p2p_config.seeds = Some(seed_addrs);
		}
	}

	if let Some(a) = server_args {
		match a.subcommand() {
			("run", _) => {
				start_server(server_config, logs_rx);
			}
			("", _) => {
				println!("Subcommand required, use 'EPIC help server' for details");
			}
			(cmd, _) => {
				println!(":: {:?}", server_args);
				panic!(
					"Unknown server command '{}', use 'EPIC help server' for details",
					cmd
				);
			}
		}
	} else {
		start_server(server_config, logs_rx);
	}
	0
}
