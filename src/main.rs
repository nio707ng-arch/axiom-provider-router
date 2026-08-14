//! Darial CLI — agent, relay, and gateway roles.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use darial::config::Config;
use darial::telemetry;

#[derive(Parser, Debug)]
#[command(
    name = "darial",
    version,
    about = "Privacy-preserving provider-neutral LLM gateway"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Local OpenAI-compatible agent (loopback by default).
    Agent {
        #[command(subcommand)]
        command: RoleCmd,
    },
    /// Content-blind privacy relay.
    Relay {
        #[command(subcommand)]
        command: RoleCmd,
    },
    /// Auth, policy, and provider gateway.
    Gateway {
        #[command(subcommand)]
        command: GatewayCmd,
    },
}

#[derive(Subcommand, Debug)]
enum RoleCmd {
    /// Serve the role.
    Serve {
        #[arg(long)]
        config: PathBuf,
    },
    /// Validate configuration.
    CheckConfig {
        #[arg(long)]
        config: PathBuf,
    },
    /// Print effective config with secrets redacted.
    PrintEffectiveConfig {
        #[arg(long)]
        config: PathBuf,
    },
}

#[derive(Subcommand, Debug)]
enum GatewayCmd {
    /// Serve the gateway.
    Serve {
        #[arg(long)]
        config: PathBuf,
    },
    /// Validate configuration.
    CheckConfig {
        #[arg(long)]
        config: PathBuf,
    },
    /// Print effective config with secrets redacted.
    PrintEffectiveConfig {
        #[arg(long)]
        config: PathBuf,
    },
    /// Key management.
    Key {
        #[command(subcommand)]
        command: KeyCmd,
    },
}

#[derive(Subcommand, Debug)]
enum KeyCmd {
    /// Generate HPKE keypair.
    Generate {
        #[arg(long)]
        output: PathBuf,
        #[arg(long, default_value_t = false)]
        force: bool,
    },
    /// Print public key document from key dir.
    Publish {
        #[arg(long)]
        config: PathBuf,
    },
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(code) => code,
    }
}

fn run() -> Result<(), ExitCode> {
    let cli = Cli::parse();
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|_| ExitCode::from(1))?;

    rt.block_on(async move {
        match cli.command {
            Commands::Agent { command } => role("agent", command).await,
            Commands::Relay { command } => role("relay", command).await,
            Commands::Gateway { command } => gateway(command).await,
        }
    })
}

async fn role(name: &str, command: RoleCmd) -> Result<(), ExitCode> {
    match command {
        RoleCmd::CheckConfig { config } => {
            let cfg = load_cfg(&config)?;
            cfg.validate(name).map_err(config_err)?;
            println!("ok");
            Ok(())
        }
        RoleCmd::PrintEffectiveConfig { config } => {
            let cfg = load_cfg(&config)?;
            cfg.validate(name).map_err(config_err)?;
            println!(
                "{}",
                serde_json::to_string_pretty(&cfg.effective_sanitized()).unwrap()
            );
            Ok(())
        }
        RoleCmd::Serve { config } => {
            let cfg = load_cfg(&config)?;
            telemetry::init_tracing(cfg.telemetry.json_logs);
            cfg.validate(name).map_err(config_err)?;
            let result = match name {
                "agent" => darial::agent::serve(cfg).await,
                "relay" => darial::relay::serve(cfg).await,
                _ => unreachable!(),
            };
            result.map_err(runtime_err)
        }
    }
}

async fn gateway(command: GatewayCmd) -> Result<(), ExitCode> {
    match command {
        GatewayCmd::CheckConfig { config } => {
            let cfg = load_cfg(&config)?;
            cfg.validate("gateway").map_err(config_err)?;
            println!("ok");
            Ok(())
        }
        GatewayCmd::PrintEffectiveConfig { config } => {
            let cfg = load_cfg(&config)?;
            cfg.validate("gateway").map_err(config_err)?;
            println!(
                "{}",
                serde_json::to_string_pretty(&cfg.effective_sanitized()).unwrap()
            );
            Ok(())
        }
        GatewayCmd::Serve { config } => {
            let cfg = load_cfg(&config)?;
            telemetry::init_tracing(cfg.telemetry.json_logs);
            cfg.validate("gateway").map_err(config_err)?;
            let hpke = cfg
                .privacy
                .key_dir
                .as_ref()
                .map(|d| darial::gateway::load_keypair(d))
                .transpose()
                .map_err(config_err)?;
            darial::gateway::serve(cfg, hpke).await.map_err(runtime_err)
        }
        GatewayCmd::Key { command } => match command {
            KeyCmd::Generate { output, force } => {
                let doc = darial::gateway::generate_keys(&output, force).map_err(config_err)?;
                println!("{}", serde_json::to_string_pretty(&doc).unwrap());
                Ok(())
            }
            KeyCmd::Publish { config } => {
                let cfg = load_cfg(&config)?;
                let dir = cfg
                    .privacy
                    .key_dir
                    .as_ref()
                    .ok_or_else(|| config_err(darial::DarialError::Config("key_dir".into())))?;
                let kp = darial::gateway::load_keypair(dir).map_err(config_err)?;
                println!(
                    "{}",
                    serde_json::to_string_pretty(&kp.public_document()).unwrap()
                );
                Ok(())
            }
        },
    }
}

fn load_cfg(path: &Path) -> Result<Config, ExitCode> {
    Config::load_file(path).map_err(config_err)
}

fn config_err(err: darial::DarialError) -> ExitCode {
    eprintln!("error: {err}");
    match err {
        darial::DarialError::Config(_) => ExitCode::from(2),
        darial::DarialError::Unauthenticated | darial::DarialError::Unauthorized => {
            ExitCode::from(3)
        }
        _ => ExitCode::from(1),
    }
}

fn runtime_err(err: darial::DarialError) -> ExitCode {
    eprintln!("error: {err}");
    ExitCode::from(1)
}
