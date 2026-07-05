use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct FetchedModel {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub owned_by: Option<String>,
}

pub async fn fetch_models(base_url: &str, api_key: &str) -> Result<Vec<FetchedModel>, String> {
    let url = format!("{}/v1/models", base_url.trim_end_matches('/'));
    let client = reqwest::Client::new();
    let resp = client
        .get(&url)
        .bearer_auth(api_key)
        .send()
        .await
        .map_err(|e| format!("Network error: {e}"))?;

    #[derive(Deserialize)]
    struct ModelsResponse {
        data: Vec<FetchedModel>,
    }

    let body = resp
        .json::<ModelsResponse>()
        .await
        .map_err(|e| format!("Parse error: {e}"))?;
    Ok(body.data)
}
