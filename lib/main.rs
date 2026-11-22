//! OctaTree CLI.

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::thread::sleep;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use clap::{Args, Parser, Subcommand};
use octatree::import::Imported;
use octatree::scan::FsReader;
use octatree::settings::{STARTER_TOML, Settings};
use octatree::watch::{Control, Watcher};
use octatree::{emit, explain, generate, migrate};

#[derive(Parser)]
#[command(
    name = "octatree",
    version,
    about = "Generate a Rojo project tree from your Luau sources"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Generate default.project.json
    Generate(GenerateArgs),
    /// Keep default.project.json up to date while you work
    Watch(WatchArgs),
    /// Stop a running `octatree watch` (run it from another terminal)
    Stop(StopArgs),
    /// Show how a path is routed and which rules decide it
    Explain(ExplainArgs),
    /// Convert an old JSON config into octatree.toml
    Migrate(MigrateArgs),
    /// Write a commented starter octatree.toml
    Init(InitArgs),
}

#[derive(Args)]
struct CommonArgs {
    /// Project root (contains the source folder)
    #[arg(long, default_value = ".")]
    root: PathBuf,
    /// Config file (default: <root>/octatree.toml)
    #[arg(long)]
    config: Option<PathBuf>,
}

#[derive(Args)]
struct GenerateArgs {
    #[command(flatten)]
    common: CommonArgs,
    /// Output file (default: the config's `output`, usually <root>/default.project.json)
    #[arg(long)]
    out: Option<PathBuf>,
    /// Print the JSON to stdout instead of writing a file
    #[arg(long, conflicts_with = "check")]
    dry_run: bool,
    /// Exit with status 1 if the output file is missing or out of date
    #[arg(long)]
    check: bool,
    /// Treat warnings as errors
    #[arg(long)]
    strict: bool,
}

#[derive(Args)]
struct WatchArgs {
    #[command(flatten)]
    common: CommonArgs,
    /// Output file (default: the config's `output`, usually <root>/default.project.json)
    #[arg(long)]
    out: Option<PathBuf>,
    /// How often to check for changes, in milliseconds (minimum 100)
    #[arg(long, default_value_t = 500)]
    interval_ms: u64,
}

#[derive(Args)]
struct StopArgs {
    /// Project root of the watcher to stop
    #[arg(long, default_value = ".")]
    root: PathBuf,
}

#[derive(Args)]
struct ExplainArgs {
    #[command(flatten)]
    common: CommonArgs,
    /// Project-relative path to explain
    path: String,
}

#[derive(Args)]
struct MigrateArgs {
    /// Path to the old JSON config file to convert
    path: PathBuf,
    /// Project root where octatree.toml is written
    #[arg(long, default_value = ".")]
    root: PathBuf,
    /// Where to write the new config (default: <root>/octatree.toml)
    #[arg(long)]
    out: Option<PathBuf>,
    /// Overwrite an existing file
    #[arg(long)]
    force: bool,
}

#[derive(Args)]
struct InitArgs {
    /// Project root
    #[arg(long, default_value = ".")]
    root: PathBuf,
    /// Overwrite an existing octatree.toml
    #[arg(long)]
    force: bool,
}

/// Resolves and checks the config path (`<root>/octatree.toml` by default).
fn config_path(common: &CommonArgs) -> Result<PathBuf> {
    let path = common
        .config
        .clone()
        .unwrap_or_else(|| common.root.join("octatree.toml"));
    if path
        .extension()
        .is_none_or(|e| !e.eq_ignore_ascii_case("toml"))
    {
        bail!(
            "{} is not a .toml file; convert an old config with `octatree migrate <path>`",
            path.display()
        );
    }
    if !path.is_file() {
        bail!(
            "{} not found; create it with `octatree init` (or pass --config)",
            path.display()
        );
    }
    Ok(path)
}

fn load_settings(common: &CommonArgs) -> Result<Settings> {
    Ok(Settings::load(&config_path(common)?)?)
}

fn watch_cmd(args: &WatchArgs) -> Result<bool> {
    let root = &args.common.root;
    let mut watcher = Watcher::new(root.clone(), config_path(&args.common)?, args.out.clone())?;
    let control = Control::for_root(root);
    if control.is_active() {
        bail!("a watcher is already running for this project; stop it with `octatree stop`");
    }
    let _guard = control.start()?;
    let interval = Duration::from_millis(args.interval_ms.max(100));

    eprintln!(
        "Watching {} (checking every {} ms), writing {}",
        root.display(),
        interval.as_millis(),
        watcher.output_path().display()
    );
    eprintln!("To stop it, run `octatree stop` in another terminal (Ctrl+C also works).");

    let mut last_beat = Instant::now();
    loop {
        if control.stop_requested() {
            break;
        }
        if last_beat.elapsed() >= Duration::from_secs(1) {
            control.beat()?;
            last_beat = Instant::now();
        }
        watcher.tick(&mut |message| eprintln!("{message}"));
        sleep(interval);
    }
    eprintln!("Watcher stopped.");
    Ok(true)
}

fn stop_cmd(args: &StopArgs) -> Result<bool> {
    let control = Control::for_root(&args.root);
    if !control.is_active() {
        eprintln!("No watcher is running for {}.", args.root.display());
        return Ok(true);
    }
    control.request_stop()?;
    for _ in 0..30 {
        sleep(Duration::from_millis(100));
        if !control.is_active() {
            eprintln!("Watcher stopped.");
            return Ok(true);
        }
    }
    eprintln!("Stop requested, but the watcher has not confirmed yet; it may be busy.");
    Ok(true)
}

fn write_new(path: &Path, content: &str, force: bool) -> Result<()> {
    if path.exists() && !force {
        bail!(
            "{} already exists (use --force to overwrite)",
            path.display()
        );
    }
    std::fs::write(path, content).with_context(|| format!("writing {}", path.display()))
}

fn generate_cmd(args: GenerateArgs) -> Result<bool> {
    let settings = load_settings(&args.common)?;
    let root = &args.common.root;
    let out_path = args
        .out
        .clone()
        .unwrap_or_else(|| root.join(&settings.output));

    let mut reader = FsReader::new(root);
    let generated = generate(&settings, &mut reader).context("generation failed")?;

    for w in &generated.warnings {
        eprintln!("warning: {w}");
    }
    if args.strict && !generated.warnings.is_empty() {
        bail!("{} warning(s) in --strict mode", generated.warnings.len());
    }

    if args.dry_run {
        println!("{}", generated.json);
        return Ok(true);
    }
    if args.check {
        let ok = emit::is_up_to_date(&out_path, &generated.json)?;
        if !ok {
            eprintln!("{} is out of date", out_path.display());
        }
        return Ok(ok);
    }
    if emit::write_if_changed(&out_path, &generated.json)? {
        eprintln!("Generated {}", out_path.display());
    } else {
        eprintln!("{} is already up to date", out_path.display());
    }
    Ok(true)
}

fn explain_cmd(args: &ExplainArgs) -> Result<bool> {
    let settings = load_settings(&args.common)?;
    let mut reader = FsReader::new(&args.common.root);
    print!("{}", explain::explain(&settings, &args.path, &mut reader)?);
    Ok(true)
}

fn migrate_cmd(args: &MigrateArgs) -> Result<bool> {
    let imported = Imported::load(&args.path)?;
    let toml_text = migrate::to_toml(&imported);
    let out_path = args
        .out
        .clone()
        .unwrap_or_else(|| args.root.join("octatree.toml"));

    // Make sure what we are about to write is a valid config.
    Settings::from_toml(&toml_text, &out_path).context("converted config is invalid")?;
    write_new(&out_path, &toml_text, args.force)?;

    eprintln!(
        "Wrote {} ({} rule(s) from {})",
        out_path.display(),
        imported.duplicated.len() + imported.no_server.len() + imported.server_only.len(),
        args.path.display()
    );
    for key in &imported.unknown_keys {
        eprintln!("warning: unknown key `{key}` in the old config was not converted");
    }
    eprintln!(
        "Next: list your top-level folders in `modules`, then run `octatree explain <path>` to check the routing."
    );
    Ok(true)
}

fn init_cmd(args: &InitArgs) -> Result<bool> {
    let out_path = args.root.join("octatree.toml");
    write_new(&out_path, STARTER_TOML, args.force)?;
    eprintln!("Wrote {}", out_path.display());
    Ok(true)
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let result = match cli.command {
        Command::Generate(a) => generate_cmd(a),
        Command::Watch(a) => watch_cmd(&a),
        Command::Stop(a) => stop_cmd(&a),
        Command::Explain(a) => explain_cmd(&a),
        Command::Migrate(a) => migrate_cmd(&a),
        Command::Init(a) => init_cmd(&a),
    };
    match result {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::from(1),
        Err(e) => {
            eprintln!("error: {e:#}");
            ExitCode::from(2)
        }
    }
}
