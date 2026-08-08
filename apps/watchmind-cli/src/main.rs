use std::{
    env,
    fs::File,
    io::BufReader,
    path::{Path, PathBuf},
    process::ExitCode,
};

use watchmind_recommendation::{
    CandidateRequest, DiversificationConfig, OfflineDataset, RecommendationEngine,
    RecommendationKind, TasteProfileConfig, build_taste_profile, engine_name, evaluate_baselines,
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
    if args.is_empty() {
        println!(
            "{engine_name} — moteur offline prêt",
            engine_name = engine_name()
        );
        return Ok(());
    }

    match args.as_slice() {
        [] => unreachable!("empty arguments are handled above"),
        [command, ratings, flag, catalog] if command == "import-csv" && flag == "--catalog" => {
            import_csv(Path::new(ratings), Path::new(catalog))
        }
        [command, ..] if command == "import-csv" => {
            Err("usage: watchmind-cli import-csv <ratings.csv> --catalog <catalog.json>".to_owned())
        }
        [command, ratings, flag, catalog]
            if command == "compare-baselines" && flag == "--catalog" =>
        {
            compare_baselines(Path::new(ratings), Path::new(catalog), OutputFormat::Text)
        }
        [command, ratings, flag, catalog, format]
            if command == "compare-baselines" && flag == "--catalog" && format == "--json" =>
        {
            compare_baselines(Path::new(ratings), Path::new(catalog), OutputFormat::Json)
        }
        [command, ..] if command == "compare-baselines" => Err(
            "usage: watchmind-cli compare-baselines <ratings.csv> --catalog <catalog.json> [--json]"
                .to_owned(),
        ),
        [command, ratings, flag, catalog] if command == "recommend" && flag == "--catalog" => {
            recommend(Path::new(ratings), Path::new(catalog), OutputFormat::Text)
        }
        [command, ratings, flag, catalog, format]
            if command == "recommend" && flag == "--catalog" && format == "--json" =>
        {
            recommend(Path::new(ratings), Path::new(catalog), OutputFormat::Json)
        }
        [command, ..] if command == "recommend" => Err(
            "usage: watchmind-cli recommend <ratings.csv> --catalog <catalog.json> [--json]"
                .to_owned(),
        ),
        [command, ..] => Err(format!("unknown command {command:?}")),
    }
}

fn recommend(
    ratings_path: &Path,
    catalog_path: &Path,
    output_format: OutputFormat,
) -> Result<(), String> {
    let dataset = load_dataset(ratings_path, catalog_path)?;
    let profile = build_taste_profile(&dataset, &TasteProfileConfig::default())
        .map_err(|error| error.to_string())?;
    let engine = RecommendationEngine::default();
    let candidates = engine.generate_candidates(&dataset, &CandidateRequest::default());
    let recommendations = engine
        .recommend(&profile, &candidates, &DiversificationConfig::default())
        .map_err(|error| error.to_string())?;

    match output_format {
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

fn import_csv(ratings_path: &Path, catalog_path: &Path) -> Result<(), String> {
    let dataset = load_dataset(ratings_path, catalog_path)?;
    println!("{}", dataset.summary());
    Ok(())
}

#[derive(Clone, Copy)]
enum OutputFormat {
    Text,
    Json,
}

fn compare_baselines(
    ratings_path: &Path,
    catalog_path: &Path,
    output_format: OutputFormat,
) -> Result<(), String> {
    let dataset = load_dataset(ratings_path, catalog_path)?;
    let report = evaluate_baselines(&dataset).map_err(|error| error.to_string())?;
    match output_format {
        OutputFormat::Text => print!("{report}"),
        OutputFormat::Json => println!("{}", report.to_json().map_err(|error| error.to_string())?),
    }
    Ok(())
}

fn load_dataset(ratings_path: &Path, catalog_path: &Path) -> Result<OfflineDataset, String> {
    let ratings = open_file(ratings_path, "ratings CSV")?;
    let catalog = open_file(catalog_path, "catalog JSON")?;
    OfflineDataset::import(BufReader::new(ratings), BufReader::new(catalog))
        .map_err(|error| error.to_string())
}

fn open_file(path: &Path, label: &str) -> Result<File, String> {
    File::open(path).map_err(|error| {
        let path = PathBuf::from(path);
        format!("cannot open {label} {}: {error}", path.display())
    })
}
