use std::{
    env,
    fs::File,
    io::BufReader,
    path::{Path, PathBuf},
    process::ExitCode,
};

use watchmind_recommendation::{OfflineDataset, engine_name};

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
        [command, ..] => Err(format!("unknown command {command:?}")),
    }
}

fn import_csv(ratings_path: &Path, catalog_path: &Path) -> Result<(), String> {
    let ratings = open_file(ratings_path, "ratings CSV")?;
    let catalog = open_file(catalog_path, "catalog JSON")?;
    let dataset = OfflineDataset::import(BufReader::new(ratings), BufReader::new(catalog))
        .map_err(|error| error.to_string())?;
    println!("{}", dataset.summary());
    Ok(())
}

fn open_file(path: &Path, label: &str) -> Result<File, String> {
    File::open(path).map_err(|error| {
        let path = PathBuf::from(path);
        format!("cannot open {label} {}: {error}", path.display())
    })
}
