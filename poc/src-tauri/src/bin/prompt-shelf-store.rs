use prompt_shelf_poc::{paths::DataPaths, store::Store};
use std::path::PathBuf;

fn run() -> Result<(), String> {
    let mut args = std::env::args_os().skip(1);
    let mut directory = None;
    let mut manifest = None;
    while let Some(arg) = args.next() {
        match arg.to_str(){
            Some("--data-dir") if directory.is_none()=>directory=Some(PathBuf::from(args.next().ok_or("missing_data_dir")?)),
            Some("--import-manifest") if manifest.is_none()=>manifest=Some(PathBuf::from(args.next().ok_or("missing_manifest")?)),
            Some("--check")=>{},
            _=>return Err("usage: prompt-shelf-store --data-dir <isolated-directory> [--import-manifest <manifest.json>] [--check]".into()),
        }
    }
    let directory = directory.ok_or("offline_cli_requires_explicit_data_dir")?;
    let store = Store::open(DataPaths::new(&directory)?)?;
    if let Some(path) = manifest {
        store.import_manifest(&path)?;
    }
    println!("{}", store.check()?);
    Ok(())
}
fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1)
    }
}
