mod curse_files;
use chrono::Local;
use serde::{Deserialize, Serialize};
use anyhow::{anyhow, Ok, Result};
use reqwest;
use reqwest::header::{HeaderMap, HeaderValue};
use std::env;
use std::fs::{self, create_dir_all, remove_file, File, OpenOptions};
use std::io::{copy, Write};
use std::path::Path;

pub struct Config {
    pub api_key: String,
    pub base_dir: String,
    pub mods_dir: String,
    pub mods_file: String,
}

#[derive(Serialize, Deserialize)]
struct Mod {
    filename: String,
    name: String,
    url: Option<String>,
    version: String,
}

pub fn run(config: Config) -> Result<()> {
    let _ = log_to_file("[INFO] Starting new run of modpack-sync...");
    let _ = log_to_file(&format!("[INFO]    mods_dir={}", &config.mods_dir));
    let _ = log_to_file(&format!("[INFO]    base_dir={}", &config.base_dir));
    let _ = log_to_file(&format!("[INFO]    mods_file={}", &config.mods_file));
    return sync_mods(
        &config.mods_dir,
        &config.base_dir,
        &config.mods_file,
        &config.api_key,
    );
}

fn log_to_file(message: &str) -> Result<()> {
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open("sync.log")?;

    let now = Local::now().format("%Y-%m-%d %H:%M:%S");

    writeln!(file, "[{}] {}", now, message)?;
    Ok(())
}

fn sync_mods(mods_dir: &String, path: &String, mods_file: &String, api_key: &String) -> Result<()> {
    let _ = stage_dir(&mods_dir);
    let contents = fs::read_to_string(format!("{}/{}", path, mods_file))
        .expect("Should have been able to read the file");
    let mods: Vec<Mod> = serde_json::from_str(contents.as_str())
        .expect("Should have received correctly formatted json file");
    for m in mods.into_iter() {
        if m.filename.ends_with(".disabled") {
            let _ = log_to_file(&format!("[INFO] Skipping disabled mod: {}", &m.filename));
            continue;
        }

        if !needs_update(&mods_dir, &m.filename) {
            let _ = log_to_file(&format!("[INFO] Skipping already up to date mod: {}", &m.filename));
            continue;
        }

        let path = Path::new(&mods_dir).join(&m.filename);
        if path.exists() {
            remove_file(path)?;
        }

        match &m.url {
            Some(value) => {
                let url_parts = value.split("/");
                let project_id = url_parts
                    .last()
                    .expect("expected project_id to not be empty");
                let file_id = get_file_id(project_id, &m.filename, &api_key);
                if file_id.is_err() {
                    let _ = log_to_file(&format!("[ERR!]  couldn't find file for {}. file may have been removed!", &m.filename));
                    continue;
                }
                let download_res = download_file(project_id, file_id.unwrap(), &m.filename, mods_dir.clone(), &api_key);
                if download_res.is_err() {
                    let _ = log_to_file(&format!("[ERR!]  failed to download file: {}", &m.filename));
                    let _ = log_to_file(&format!("[ERR!]  {:?}", download_res.err()));
                }
            }
            None => {
                let _ = log_to_file(&format!("[WARN] Skipping file: {} missing url! Check your modlist.json file!", &m.filename));
            }
        }
    }
    return Ok(());
}

fn needs_update(mods_dir: &str, jar_file: &str) -> bool {
    let new_path = Path::new(mods_dir).join(jar_file);

    if !new_path.exists() {
        return true;
    }

    let entries = match fs::read_dir(mods_dir) {
        std::result::Result::Ok(e) => e,
        Err(_) => return true,
    };

    for entry in entries.flatten() {
        let file_name = entry.file_name();
        let file_name = file_name.to_string_lossy();

        if !file_name.ends_with(".jar") {
            continue;
        }

        if file_name == jar_file {
            // same mod but different version
            return false;
        }
    }

    true
}

fn get_file_id(project_id: &str, filename: &String, api_key: &String) -> Result<u64> {
    let _ = log_to_file(&format!("[INFO] attempting to find file {}", filename));
    for f in curse_files::CurseFile::of(&project_id, &api_key)? {
        let file = f?;
        if file.file_name.as_str() == filename.as_str() {
            let _ = log_to_file(&format!("[INFO]  matching file found, will now attempt to download mod file"));
            return Ok(file.id);
        }
    }

    return Err(anyhow!(
        " -----> failed to find file id for file {}",
        filename
    ));
}

fn download_file(
    project_id: &str,
    file_id: u64,
    filename: &str,
    dir: String,
    api_key: &String,
) -> Result<()> {
    let client = reqwest::blocking::Client::new();
    let mut headers = HeaderMap::new();
    headers.insert("X-Api-Token", HeaderValue::from_str(&api_key)?);
    headers.insert(
        "Accept-Encoding",
        HeaderValue::from_str("gzip, deflate, br, zstd")?,
    );

    let url = format!(
        "https://www.curseforge.com/api/v1/mods/{}/files/{}/download",
        project_id, file_id
    );

    let resp = client
        .get(&url)
        .headers(headers)
        .send();
    if resp.is_err() {
        return Err(anyhow!("request to get file {} failed", file_id));
    }
    let out = File::create(format!("{}/{}", dir, filename));
    if out.is_err() {
        return Err(anyhow!("failed to create jar file"));
    }
    let content = resp?.bytes();
    if content.is_err() {
        return Err(anyhow!("no file content to write"));
    }
    copy(&mut content?.as_ref(), &mut out?)?;

    let _ = log_to_file(&format!("[INFO]  successfully downloaded {}", filename));
    return Ok(());
}

fn stage_dir(dir: &str) -> Result<()> {
    if !Path::new(dir).exists() {
        create_dir_all(dir)?;
    }
    return Ok(());
}

impl Config {
    pub fn build(args: &[String]) -> Result<Config> {
        if args.len() < 2 {
            return Err(anyhow!("expected argument containing path to modpack"));
        }

        let base_dir = args[1].clone();
        let api_key = env::var("CURSE_API_KEY").unwrap();

        let mods_file = "modlist.json".to_string();
        let mods_dir = format!("{}/.minecraft/mods", base_dir);

        Ok(Config {
            api_key,
            base_dir,
            mods_dir,
            mods_file,
        })
    }
}
