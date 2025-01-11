use anyhow::{anyhow, Ok, Result};
use reqwest;
use reqwest::header::{HeaderMap, HeaderValue, ACCEPT};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::env;
use std::fs::{self, create_dir_all, remove_dir_all, File};
use std::io::copy;
use std::path::Path;

pub struct Config {
    pub api_key: String,
    pub base_dir: String,
    pub mods_dir: String,
    pub mods_file: String,
}

#[derive(Serialize, Deserialize)]
struct Mod {
    authors: Vec<String>,
    filename: String,
    name: String,
    url: String,
    version: String,
}

pub fn run(config: Config) -> Result<()> {
    return sync_mods(
        &config.mods_dir,
        &config.base_dir,
        &config.mods_file,
        &config.api_key,
    );
}

fn sync_mods(mods_dir: &String, path: &String, mods_file: &String, api_key: &String) -> Result<()> {
    let _ = stage_dir(&mods_dir);
    let contents = fs::read_to_string(format!("{}/{}", path, mods_file))
        .expect("Should have been able to read the file");
    let mods: Vec<Mod> = serde_json::from_str(contents.as_str())
        .expect("Should have received correctly formatted json file");
    for m in mods.into_iter() {
        let url_parts = m.url.split("/");
        let project_id = url_parts
            .last()
            .expect("expected project_id to not be empty");
        let file_id = get_file_id(project_id, &m.filename, &api_key)
            .expect("expected to receive a valid file id");
        println!("file id is {}", file_id);
        download_file(project_id, file_id, m.filename, mods_dir.clone(), &api_key)
            .expect("expected to download the jar file")
    }
    return Ok(());
}

fn get_file_id(project_id: &str, filename: &String, api_key: &String) -> Result<u64> {
    println!("attempting to find file {}", filename);
    let client = reqwest::blocking::Client::new();
    let mut headers = HeaderMap::new();
    headers.insert(ACCEPT, HeaderValue::from_static("application/json"));
    headers.insert("X-Api-Token", HeaderValue::from_str(&api_key)?);

    let url = format!(
        "https://www.curseforge.com/api/v1/mods/{}/files?pageIndex=0&pageSize=100&sort=dateCreated&sortDescending=true",
        project_id
    );

    let body = client.get(&url).headers(headers).send()?.json::<Value>();

    for f in body?["data"].as_array().unwrap().into_iter() {
        if f.get("fileName").unwrap().as_str().unwrap() == filename.as_str() {
            println!(" -----> matching file found, will now attempt to download mod file");
            return Ok(f
                .get("id")
                .unwrap()
                .as_u64()
                .expect("expected to find a valid file id"));
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
    filename: String,
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
        .send()
        .expect(format!("request to get file {} failed", file_id).as_str());
    let mut out = File::create(format!("{}/{}", dir, filename)).expect("failed to create file");
    let content = resp.bytes().expect("expected to receive bytes to write");
    copy(&mut content.as_ref(), &mut out)?;

    println!(" -----> successfully downloaded {}", filename);
    return Ok(());
}

fn stage_dir(dir: &str) -> Result<()> {
    if Path::new(dir).exists() {
        remove_dir_all(dir).unwrap();
    }
    create_dir_all(dir).unwrap();
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
        let mods_dir = format!("{}/minecraft/mods", base_dir);

        Ok(Config {
            api_key,
            base_dir,
            mods_dir,
            mods_file,
        })
    }
}
