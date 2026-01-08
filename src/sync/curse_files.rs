use reqwest::Result;
use reqwest::header::{HeaderMap, HeaderValue, ACCEPT};
use serde::Deserialize;

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct ApiResponse {
    data: Vec<ModFile>,
    pagination: PaginationMeta,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ModFile {
    pub id: u64,
    pub file_name: String,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct PaginationMeta {
    total_count: u64,
}

pub struct CurseFile {
    project_id: String,
    api_key: String,
    client: reqwest::blocking::Client,
    page: u32,
    per_page: u32,
    files: <Vec<ModFile> as IntoIterator>::IntoIter,
    total: u64,
}

impl CurseFile {
    pub fn of(project_id: &str, api_key: &str) -> Result<Self> {
        Ok(CurseFile {
            project_id: project_id.to_owned(),
            api_key: api_key.to_owned(),
            client: reqwest::blocking::Client::new(),
            files: vec![].into_iter(),
            page: 0,
            per_page: 50,
            total: 0,
        })
    }

    fn try_next(&mut self) -> Result<Option<ModFile>> {
        if let Some(dep) = self.files.next() {
            return Ok(Some(dep));
        }

        if self.page > 0 && u64::from(self.page * self.per_page) >= self.total {
            return Ok(None);
        }

        let url = format!("https://www.curseforge.com/api/v1/mods/{}/files?pageIndex={}&pageSize={}&sort=dateCreated&sortDescending=true&removeAlphas=false", 
            self.project_id, 
            self.page, 
            self.per_page);

        let mut headers = HeaderMap::new();
        headers.insert(ACCEPT, HeaderValue::from_static("application/json"));
        headers.insert("X-Api-Token", HeaderValue::from_str(&self.api_key).unwrap());
        
        let response = self.client
            .get(&url)
            .headers(headers)
            .send()?
            .json::<ApiResponse>()?;
        
        self.page += 1;
        self.files = response.data.into_iter();
        self.total = response.pagination.total_count;
        Ok(self.files.next())
    }
    
}

impl Iterator for CurseFile {
    type Item = Result<ModFile>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.try_next() {
            Ok(Some(dep)) => Some(Ok(dep)),
            Ok(None) => None,
            Err(err) => Some(Err(err)),
        }
    }
}