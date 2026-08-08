use std::{
    env,
    fs::File,
    io::BufReader,
    path::{Path, PathBuf},
    process::ExitCode,
};

use watchmind_recommendation::{
    CandidateRequest, DiversificationConfig, FullEvaluationConfig, OfflineDataset,
    RecommendationEngine, RecommendationKind, TasteProfile, TasteProfileConfig, WorkId,
    build_taste_profile, engine_name, evaluate_baselines, evaluate_full,
};

fn main() -> ExitCode {
    match run(env::args().skip(1)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::from(2)
        }
    }
}

fn run(args: impl IntoIterator<Item = String>) -> Result<(), String> {
    let args = args.into_iter().collect::<Vec<_>>();
    let Some((command, arguments)) = args.split_first() else {
        println!("{} - moteur offline prêt", engine_name());
        print_help();
        return Ok(());
    };

    match command.as_str() {
        "import-csv" => {
            let parsed = DatasetArguments::parse(arguments, false, usage(command))?;
            import_csv(&parsed.ratings, &parsed.catalog)
        }
        "build-profile" => {
            let parsed = DatasetArguments::parse(arguments, false, usage(command))?;
            build_profile_command(&parsed)
        }
        "show-poles" => {
            let parsed = DatasetArguments::parse(arguments, false, usage(command))?;
            show_poles(&parsed)
        }
        "recommend" => {
            let parsed = DatasetArguments::parse(arguments, false, usage(command))?;
            recommend(&parsed)
        }
        "explain" => explain(arguments),
        "evaluate" => {
            let parsed = DatasetArguments::parse(arguments, true, usage(command))?;
            evaluate(&parsed)
        }
        "leave-one-out" => {
            let parsed = DatasetArguments::parse(arguments, true, usage(command))?;
            leave_one_out(&parsed)
        }
        "compare-baselines" => {
            let parsed = DatasetArguments::parse(arguments, false, usage(command))?;
            compare_baselines(&parsed)
        }
        "help" | "--help" | "-h" => {
            print_help();
            Ok(())
        }
        _ => Err(format!("unknown command {command:?}\n\n{}", help_text())),
    }
}

#[derive(Clone, Copy)]
enum OutputFormat {
    Text,
    Json,
}

struct DatasetArguments {
    ratings: PathBuf,
    catalog: PathBuf,
    config: Option<PathBuf>,
    output: OutputFormat,
}

impl DatasetArguments {
    fn parse(arguments: &[String], allow_config: bool, usage: &str) -> Result<Self, String> {
        let Some(ratings) = arguments.first() else {
            return Err(usage.to_owned());
        };
        if ratings.starts_with('-') {
            return Err(usage.to_owned());
        }
        let mut catalog = None;
        let mut config = None;
        let mut output = OutputFormat::Text;
        let mut index = 1;
        while index < arguments.len() {
            match arguments[index].as_str() {
                "--catalog" => {
                    let value = arguments.get(index + 1).ok_or_else(|| usage.to_owned())?;
                    catalog = Some(PathBuf::from(value));
                    index += 2;
                }
                "--config" if allow_config => {
                    let value = arguments.get(index + 1).ok_or_else(|| usage.to_owned())?;
                    config = Some(PathBuf::from(value));
                    index += 2;
                }
                "--json" => {
                    output = OutputFormat::Json;
                    index += 1;
                }
                _ => return Err(usage.to_owned()),
            }
        }
        Ok(Self {
            ratings: PathBuf::from(ratings),
            catalog: catalog.ok_or_else(|| usage.to_owned())?,
            config,
            output,
        })
    }
}

fn import_csv(ratings_path: &Path, catalog_path: &Path) -> Result<(), String> {
    let dataset = load_dataset(ratings_path, catalog_path)?;
    println!("{}", dataset.summary());
    Ok(())
}

fn build_profile_command(arguments: &DatasetArguments) -> Result<(), String> {
    let (_, profile) = load_profile(arguments)?;
    match arguments.output {
        OutputFormat::Json => println!(
            "{}",
            serde_json::to_string_pretty(&profile).map_err(|error| error.to_string())?
        ),
        OutputFormat::Text => println!(
            "profile: history={} confidence={:.3} mode={:?} tags={} poles={} axes={:?}",
            profile.history_size(),
            profile.confidence().get(),
            profile.mode(),
            profile.tag_affinities().len(),
            profile.poles().len(),
            profile.axes().source(),
        ),
    }
    Ok(())
}

fn show_poles(arguments: &DatasetArguments) -> Result<(), String> {
    let (dataset, profile) = load_profile(arguments)?;
    match arguments.output {
        OutputFormat::Json => println!(
            "{}",
            serde_json::to_string_pretty(profile.poles()).map_err(|error| error.to_string())?
        ),
        OutputFormat::Text => {
            println!("taste poles: {}", profile.poles().len());
            for pole in profile.poles() {
                let tags = pole
                    .dominant_tags()
                    .iter()
                    .map(|tag| format!("{} ({:.3})", tag.name(), tag.weight()))
                    .collect::<Vec<_>>()
                    .join(", ");
                let representatives = pole
                    .representative_work_ids()
                    .iter()
                    .map(|work_id| {
                        dataset
                            .catalog()
                            .iter()
                            .find(|work| work.id() == *work_id)
                            .map_or_else(
                                || work_id.get().to_string(),
                                |work| format!("{} [{}]", work.title(), work_id.get()),
                            )
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                println!(
                    "{}. members={} tags=[{}] representatives=[{}]",
                    pole.ordinal() + 1,
                    pole.member_count(),
                    tags,
                    representatives
                );
            }
        }
    }
    Ok(())
}

fn recommend(arguments: &DatasetArguments) -> Result<(), String> {
    let (dataset, profile) = load_profile(arguments)?;
    let engine = RecommendationEngine::default();
    let candidates = engine.generate_candidates(&dataset, &CandidateRequest::default());
    let recommendations = engine
        .recommend(&profile, &candidates, &DiversificationConfig::default())
        .map_err(|error| error.to_string())?;

    match arguments.output {
        OutputFormat::Text => {
            println!("{}", candidates.report());
            for (index, recommendation) in recommendations.recommendations().iter().enumerate() {
                let scored = recommendation.scored();
                let kind = match recommendation.kind() {
                    RecommendationKind::Safe => "sûre",
                    RecommendationKind::Exploration => "pari",
                };
                println!(
                    "\n{}. {} [id={}] score={:.6} type={kind}",
                    index + 1,
                    scored.title(),
                    scored.work_id().get(),
                    scored.score().total()
                );
                if let Some(exploration) = recommendation.exploration() {
                    println!("{}", exploration.text());
                }
                println!("{}", scored.explanation());
            }
        }
        OutputFormat::Json => println!(
            "{}",
            serde_json::to_string(&serde_json::json!({
                "candidate_report": candidates.report(),
                "selection": {
                    "safe_count": recommendations.safe_count(),
                    "exploration_count": recommendations.exploration_count(),
                },
                "recommendations": recommendations.recommendations(),
            }))
            .map_err(|error| error.to_string())?
        ),
    }
    Ok(())
}

fn explain(arguments: &[String]) -> Result<(), String> {
    let Some(raw_work_id) = arguments.first() else {
        return Err(usage("explain").to_owned());
    };
    let work_id = raw_work_id
        .parse::<u32>()
        .ok()
        .and_then(|value| WorkId::new(value).ok())
        .ok_or_else(|| "work id must be a positive integer".to_owned())?;
    let parsed = DatasetArguments::parse(&arguments[1..], false, usage("explain"))?;
    let (dataset, profile) = load_profile(&parsed)?;
    let work = dataset
        .catalog()
        .iter()
        .find(|work| work.id() == work_id)
        .ok_or_else(|| format!("catalog does not contain work {}", work_id.get()))?;
    let scored = RecommendationEngine::default()
        .score_candidates(&profile, std::slice::from_ref(work))
        .map_err(|error| error.to_string())?
        .pop()
        .expect("one candidate produces one score");
    match parsed.output {
        OutputFormat::Text => {
            println!(
                "{} [id={}] score={:.6}",
                scored.title(),
                scored.work_id().get(),
                scored.score().total()
            );
            println!("{}", scored.explanation());
        }
        OutputFormat::Json => println!(
            "{}",
            serde_json::to_string_pretty(&scored).map_err(|error| error.to_string())?
        ),
    }
    Ok(())
}

fn evaluate(arguments: &DatasetArguments) -> Result<(), String> {
    let dataset = load_dataset(&arguments.ratings, &arguments.catalog)?;
    let config = load_evaluation_config(arguments.config.as_deref())?;
    let report = evaluate_full(&dataset, &config).map_err(|error| error.to_string())?;
    match arguments.output {
        OutputFormat::Text => print!("{}", report.to_markdown()),
        OutputFormat::Json => println!("{}", report.to_json().map_err(|error| error.to_string())?),
    }
    if report.passed() {
        Ok(())
    } else {
        Err(format!(
            "evaluation gate failed: {}",
            report.failures().join("; ")
        ))
    }
}

fn leave_one_out(arguments: &DatasetArguments) -> Result<(), String> {
    let dataset = load_dataset(&arguments.ratings, &arguments.catalog)?;
    let config = load_evaluation_config(arguments.config.as_deref())?;
    let report = evaluate_full(&dataset, &config).map_err(|error| error.to_string())?;
    match arguments.output {
        OutputFormat::Text => {
            let metrics = report.engine().metrics();
            println!(
                "leave-one-out: cases={} median_rank={:.3} recall@10={:.3} recall@20={:.3} mrr={:.3}",
                report.engine().target_ranks().len(),
                metrics.median_rank(),
                metrics.recall_at_10(),
                metrics.recall_at_20(),
                metrics.mean_reciprocal_rank(),
            );
            for target in report.engine().target_ranks() {
                println!(
                    "- work_id={} rank={}",
                    target.work_id().get(),
                    target.rank()
                );
            }
        }
        OutputFormat::Json => println!(
            "{}",
            serde_json::to_string_pretty(report.engine()).map_err(|error| error.to_string())?
        ),
    }
    Ok(())
}

fn compare_baselines(arguments: &DatasetArguments) -> Result<(), String> {
    let dataset = load_dataset(&arguments.ratings, &arguments.catalog)?;
    let report = evaluate_baselines(&dataset).map_err(|error| error.to_string())?;
    match arguments.output {
        OutputFormat::Text => print!("{report}"),
        OutputFormat::Json => println!("{}", report.to_json().map_err(|error| error.to_string())?),
    }
    Ok(())
}

fn load_profile(arguments: &DatasetArguments) -> Result<(OfflineDataset, TasteProfile), String> {
    let dataset = load_dataset(&arguments.ratings, &arguments.catalog)?;
    let profile = build_taste_profile(&dataset, &TasteProfileConfig::default())
        .map_err(|error| error.to_string())?;
    Ok((dataset, profile))
}

fn load_evaluation_config(path: Option<&Path>) -> Result<FullEvaluationConfig, String> {
    let Some(path) = path else {
        return Ok(FullEvaluationConfig::default());
    };
    serde_json::from_reader(BufReader::new(open_file(path, "evaluation config")?))
        .map_err(|error| format!("invalid evaluation config {}: {error}", path.display()))
}

fn load_dataset(ratings_path: &Path, catalog_path: &Path) -> Result<OfflineDataset, String> {
    let ratings = open_file(ratings_path, "ratings CSV")?;
    let catalog = open_file(catalog_path, "catalog JSON")?;
    OfflineDataset::import(BufReader::new(ratings), BufReader::new(catalog))
        .map_err(|error| error.to_string())
}

fn open_file(path: &Path, label: &str) -> Result<File, String> {
    File::open(path).map_err(|error| format!("cannot open {label} {}: {error}", path.display()))
}

fn usage(command: &str) -> &'static str {
    match command {
        "import-csv" => "usage: watchmind-cli import-csv <ratings.csv> --catalog <catalog.json>",
        "build-profile" => {
            "usage: watchmind-cli build-profile <ratings.csv> --catalog <catalog.json> [--json]"
        }
        "show-poles" => {
            "usage: watchmind-cli show-poles <ratings.csv> --catalog <catalog.json> [--json]"
        }
        "recommend" => {
            "usage: watchmind-cli recommend <ratings.csv> --catalog <catalog.json> [--json]"
        }
        "explain" => {
            "usage: watchmind-cli explain <work-id> <ratings.csv> --catalog <catalog.json> [--json]"
        }
        "evaluate" => {
            "usage: watchmind-cli evaluate <ratings.csv> --catalog <catalog.json> [--config <evaluation.json>] [--json]"
        }
        "leave-one-out" => {
            "usage: watchmind-cli leave-one-out <ratings.csv> --catalog <catalog.json> [--config <evaluation.json>] [--json]"
        }
        "compare-baselines" => {
            "usage: watchmind-cli compare-baselines <ratings.csv> --catalog <catalog.json> [--json]"
        }
        _ => "usage: watchmind-cli <command> --help",
    }
}

fn help_text() -> &'static str {
    "Commands: import-csv, build-profile, show-poles, recommend, explain, evaluate, leave-one-out, compare-baselines"
}

fn print_help() {
    println!("{}", help_text());
    for command in [
        "import-csv",
        "build-profile",
        "show-poles",
        "recommend",
        "explain",
        "evaluate",
        "leave-one-out",
        "compare-baselines",
    ] {
        println!("{}", usage(command));
    }
}
