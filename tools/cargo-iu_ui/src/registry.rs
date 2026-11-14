use serde::Deserialize;
use std::collections::BTreeMap;

const GITHUB_RAW_BASE_URL: &str = "https://raw.githubusercontent.com";

#[derive(Deserialize, Debug, Clone)]
pub struct Registry {
    pub name: String,
    pub updated_at: String,
    #[serde(default)]
    pub assets: BTreeMap<String, AssetEntry>,
    #[serde(default)]
    pub components: BTreeMap<String, ComponentEntry>,
    #[serde(default)]
    pub themes: BTreeMap<String, SimpleEntry>,
    #[serde(default)]
    pub tags: BTreeMap<String, SimpleEntry>,
    #[serde(default)]
    pub utils: BTreeMap<String, SimpleEntry>,
}

#[derive(Deserialize, Debug, Clone, PartialEq)]
pub struct AssetEntry {
    pub path: String,
    pub hash: String,
}

#[derive(Deserialize, Debug, Clone, PartialEq)]
pub struct ComponentEntry {
    #[serde(rename = "type")]
    pub comp_type: String, // "file" or "dir"
    pub version: String,
    pub target_name: Option<String>,
    pub target_dir: Option<String>,
    pub root_dir: Option<String>,
    pub files: Vec<String>,
    #[serde(default)]
    pub assets: Vec<String>,
}

#[derive(Deserialize, Debug, Clone, PartialEq)]
pub struct SimpleEntry {
    pub version: String,
    pub target_name: String,
    pub files: Vec<String>,
}

impl Registry {
    /// Download and parse `registry.json` from GitHub Raw
    pub fn fetch(repo: &str, branch: &str) -> Result<Self, Box<dyn std::error::Error>> {
        Self::fetch_with_base(GITHUB_RAW_BASE_URL, repo, branch)
    }

    fn fetch_with_base(
        base_url: &str,
        repo: &str,
        branch: &str,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let url = format!("{base_url}/{repo}/{branch}/registry.json");

        let mut response = ureq::get(&url)
            .header("User-Agent", "cargo-iu_ui")
            .call()
            .map_err(|e| format!("Failed to fetch registry from {url}: {e}"))?;

        let registry: Registry = response.body_mut().read_json()?;
        Ok(registry)
    }

    /// Retrieving the raw text of a specific file from GitHub Raw
    pub fn fetch_raw_file(
        repo: &str,
        branch: &str,
        file_path: &str,
    ) -> Result<String, Box<dyn std::error::Error>> {
        Self::fetch_raw_file_with_base(GITHUB_RAW_BASE_URL, repo, branch, file_path)
    }

    fn fetch_raw_file_with_base(
        base_url: &str,
        repo: &str,
        branch: &str,
        file_path: &str,
    ) -> Result<String, Box<dyn std::error::Error>> {
        let url = format!("{base_url}/{repo}/{branch}/{file_path}");

        let mut response = ureq::get(&url)
            .header("User-Agent", "cargo-iu_ui")
            .call()
            .map_err(|e| format!("Failed to fetch file from {url}: {e}"))?;

        Ok(response.body_mut().read_to_string()?)
    }

    /// Retrieving Binary Files from GitHub Raw
    pub fn fetch_raw_bytes(
        repo: &str,
        branch: &str,
        file_path: &str,
    ) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        Self::fetch_raw_bytes_with_base(GITHUB_RAW_BASE_URL, repo, branch, file_path)
    }

    fn fetch_raw_bytes_with_base(
        base_url: &str,
        repo: &str,
        branch: &str,
        file_path: &str,
    ) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        let url = format!("{base_url}/{repo}/{branch}/{file_path}");

        let response = ureq::get(&url)
            .header("User-Agent", "cargo-iu_ui")
            .call()
            .map_err(|e| format!("Failed to fetch asset from {url}: {e}"))?;

        let mut reader = response.into_body().into_reader();
        let mut bytes = Vec::new();

        std::io::copy(&mut reader, &mut bytes)?;
        Ok(bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io::{BufRead, BufReader, Write},
        net::TcpListener,
        thread,
    };

    // 1リクエストだけ受け取って指定レスポンスを返す簡易モック HTTP サーバー
    fn spawn_mock_server(
        status: &str,
        content_type: &str,
        body: Vec<u8>,
    ) -> (String, thread::JoinHandle<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("failed to bind mock server");
        let addr = listener.local_addr().unwrap();
        let status = status.to_string();
        let content_type = content_type.to_string();

        let handle = thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("failed to accept");
            let mut reader = BufReader::new(stream.try_clone().unwrap());

            // リクエストライン＋ヘッダを空行まで読み切る
            let mut raw_request = String::new();
            loop {
                let mut line = String::new();
                let n = reader.read_line(&mut line).unwrap();
                if n == 0 || line == "\r\n" || line == "\n" {
                    break;
                }
                raw_request.push_str(&line);
            }

            let header = format!(
                "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            );
            stream.write_all(header.as_bytes()).unwrap();
            stream.write_all(&body).unwrap();
            stream.flush().unwrap();

            raw_request
        });

        (format!("http://{addr}"), handle)
    }

    #[test]
    fn test_deserialize_minimal_registry_uses_defaults() {
        let json = r#"{
            "name": "iu_ui",
            "updated_at": "2026-11-14T11:00:00+09:00"
        }"#;

        let reg: Registry = serde_json::from_str(json).unwrap();
        assert_eq!(reg.name, "iu_ui");
        assert_eq!(reg.updated_at, "2026-11-14T11:00:00+09:00");
        assert!(reg.assets.is_empty());
        assert!(reg.components.is_empty());
        assert!(reg.themes.is_empty());
        assert!(reg.tags.is_empty());
        assert!(reg.utils.is_empty());
    }

    #[test]
    fn test_deserialize_full_registry() {
        let json = r#"{
            "name": "iu_ui",
            "updated_at": "2026-11-14T11:00:00+09:00",
            "assets": {
                "logo.png": {
                    "path": "iu_ui/src/assets/logo.png",
                    "hash": "abc123"
                }
            },
            "components": {
                "button": {
                    "type": "file",
                    "version": "20260101-1111111",
                    "target_name": "button.rs",
                    "files": ["iu_ui/src/components/button.rs"]
                },
                "tabs": {
                    "type": "dir",
                    "version": "20260102-2222222",
                    "root_dir": "iu_ui/src/components/tabs",
                    "target_dir": "tabs",
                    "files": ["mod.rs", "tab_item.rs"],
                    "assets": ["logo.png"]
                }
            },
            "themes": {
                "dark": {
                    "version": "20260103-3333333",
                    "target_name": "dark.rs",
                    "files": ["iu_ui/src/theme/dark.rs"]
                }
            },
            "tags": {
                "badge": {
                    "version": "20260104-4444444",
                    "target_name": "badge.rs",
                    "files": ["iu_ui/src/tag/badge.rs"]
                }
            },
            "utils": {
                "cn": {
                    "version": "20260105-5555555",
                    "target_name": "cn.rs",
                    "files": ["iu_ui/src/utils/cn.rs"]
                }
            }
        }"#;

        let reg: Registry = serde_json::from_str(json).unwrap();

        assert_eq!(
            reg.assets.get("logo.png"),
            Some(&AssetEntry {
                path: "iu_ui/src/assets/logo.png".to_string(),
                hash: "abc123".to_string(),
            })
        );

        let button = reg.components.get("button").unwrap();
        assert_eq!(button.comp_type, "file");
        assert_eq!(button.target_name.as_deref(), Some("button.rs"));
        assert_eq!(button.target_dir, None);
        assert_eq!(button.root_dir, None);
        assert!(button.assets.is_empty());

        let tabs = reg.components.get("tabs").unwrap();
        assert_eq!(tabs.comp_type, "dir");
        assert_eq!(tabs.target_name, None);
        assert_eq!(tabs.target_dir.as_deref(), Some("tabs"));
        assert_eq!(tabs.root_dir.as_deref(), Some("iu_ui/src/components/tabs"));
        assert_eq!(tabs.files, vec!["mod.rs", "tab_item.rs"]);
        assert_eq!(tabs.assets, vec!["logo.png"]);

        assert_eq!(reg.themes["dark"].target_name, "dark.rs");
        assert_eq!(reg.tags["badge"].target_name, "badge.rs");
        assert_eq!(reg.utils["cn"].target_name, "cn.rs");
    }

    #[test]
    fn test_deserialize_invalid_registry_returns_err() {
        // 必須フィールド updated_at 欠落
        let missing_field = r#"{"name": "iu_ui"}"#;
        assert!(serde_json::from_str::<Registry>(missing_field).is_err());

        // ComponentEntry で "type" ではなく "comp_type" になっている
        let wrong_rename = r#"{
            "name": "iu_ui",
            "updated_at": "2026-11-14",
            "components": {
                "button": {
                    "comp_type": "file",
                    "version": "1",
                    "files": []
                }
            }
        }"#;
        assert!(serde_json::from_str::<Registry>(wrong_rename).is_err());
    }

    #[test]
    fn test_fetch_with_base_success_and_request_headers() {
        let body = br#"{
            "name": "iu_ui",
            "updated_at": "2026-11-14T11:00:00+09:00",
            "components": {}
        }"#
        .to_vec();

        let (base_url, handle) = spawn_mock_server("200 OK", "application/json", body);

        let reg = Registry::fetch_with_base(&base_url, "eto0202/michiu", "main").unwrap();
        assert_eq!(reg.name, "iu_ui");

        let req = handle.join().unwrap();
        assert!(req.starts_with("GET /eto0202/michiu/main/registry.json HTTP/1.1"));
        assert!(req.to_ascii_lowercase().contains("user-agent: cargo-iu_ui"));
    }

    #[test]
    fn test_fetch_with_base_http_404_error() {
        let (base_url, handle) =
            spawn_mock_server("404 Not Found", "text/plain", b"Not Found".to_vec());

        let err = Registry::fetch_with_base(&base_url, "eto0202/michiu", "main").unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("Failed to fetch registry from"));
        assert!(msg.contains("/eto0202/michiu/main/registry.json"));

        handle.join().unwrap();
    }

    #[test]
    fn test_fetch_with_base_invalid_json_returns_err() {
        let (base_url, handle) =
            spawn_mock_server("200 OK", "application/json", b"{invalid json".to_vec());

        let err = Registry::fetch_with_base(&base_url, "eto0202/michiu", "main").unwrap_err();
        assert!(!err.to_string().is_empty());

        handle.join().unwrap();
    }

    #[test]
    fn test_fetch_raw_file_with_base() {
        let code = "pub fn button() {}\n";
        let (base_url, handle) = spawn_mock_server(
            "200 OK",
            "text/plain; charset=utf-8",
            code.as_bytes().to_vec(),
        );

        let result = Registry::fetch_raw_file_with_base(
            &base_url,
            "eto0202/michiu",
            "main",
            "iu_ui/src/components/button.rs",
        )
        .unwrap();
        assert_eq!(result, code);

        let req = handle.join().unwrap();
        assert!(
            req.starts_with("GET /eto0202/michiu/main/iu_ui/src/components/button.rs HTTP/1.1")
        );
    }

    #[test]
    fn test_fetch_raw_file_with_base_error() {
        let (base_url, handle) =
            spawn_mock_server("404 Not Found", "text/plain", b"Not Found".to_vec());

        let err =
            Registry::fetch_raw_file_with_base(&base_url, "eto0202/michiu", "main", "missing.rs")
                .unwrap_err();
        assert!(err.to_string().contains("Failed to fetch file from"));

        handle.join().unwrap();
    }

    #[test]
    fn test_fetch_raw_bytes_with_base() {
        // UTF-8 として不正なバイト列を含むバイナリデータもそのまま取得できること
        let binary = vec![0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0xFF, 0x00];
        let (base_url, handle) =
            spawn_mock_server("200 OK", "application/octet-stream", binary.clone());

        let result = Registry::fetch_raw_bytes_with_base(
            &base_url,
            "eto0202/michiu",
            "main",
            "iu_ui/src/assets/logo.png",
        )
        .unwrap();
        assert_eq!(result, binary);

        let req = handle.join().unwrap();
        assert!(req.starts_with("GET /eto0202/michiu/main/iu_ui/src/assets/logo.png HTTP/1.1"));
    }

    #[test]
    fn test_fetch_raw_bytes_with_base_error() {
        let (base_url, handle) =
            spawn_mock_server("500 Internal Server Error", "text/plain", b"err".to_vec());

        let err = Registry::fetch_raw_bytes_with_base(
            &base_url,
            "eto0202/michiu",
            "main",
            "iu_ui/src/assets/logo.png",
        )
        .unwrap_err();
        assert!(err.to_string().contains("Failed to fetch asset from"));

        handle.join().unwrap();
    }
}
