#[cfg(test)]
mod tests {
    use michiu_guard::{Unvalidated, Validate, Validated};
    use std::collections::HashMap;
    use std::path::PathBuf;

    // テスト用のダミー型（0より大きい値のみを許容）
    #[derive(Debug, PartialEq, Eq, serde::Deserialize)]
    struct PositiveI32(i32);

    impl Validate for PositiveI32 {
        type Error = &'static str;

        fn validate(self) -> Result<Self, Self::Error> {
            if self.0 > 0 {
                Ok(self)
            } else {
                Err("Value must be positive")
            }
        }
    }

    #[test]
    fn test_unvalidated_basic() {
        let unvalidated = Unvalidated::new(17);

        // 値が正しく取り出せるか
        assert_eq!(unvalidated.into_inner(), 17);
    }

    #[test]
    fn test_unvalidated_map() {
        let unvalidated = Unvalidated::new(10);

        // mapで中身を2倍に
        let mapped = unvalidated.map(|x| x * 2);
        assert_eq!(mapped.into_inner(), 20);
    }

    #[test]
    fn test_assume_valid() {
        let unvalidated = Unvalidated::new(153);

        // 検証をスキップして Validated にできるか
        let validated = unvalidated.assume_valid();
        assert_eq!(validated.into_inner(), 153);
    }

    #[test]
    fn test_validate_with() {
        let unvalidated = Unvalidated::new("hello");

        // クロージャを使った検証：成功 (同一型を返す)
        let res_ok =
            unvalidated.validate_with(|s| if s.len() > 3 { Ok(s) } else { Err("too short") });
        assert!(res_ok.is_ok());
        assert_eq!(res_ok.unwrap().into_inner(), "hello");

        // クロージャを使った検証：失敗
        let res_err = unvalidated.validate_with(|s| {
            if s.len() > 10 {
                Ok(s)
            } else {
                Err("too short")
            }
        });
        assert_eq!(res_err.unwrap_err(), "too short");
    }

    #[test]
    fn test_validate_with_type_conversion() {
        // String から i32 へ型をパース・検証しながら変換するテスト
        let unvalidated = Unvalidated::new("123".to_string());

        let res: Result<Validated<i32>, _> =
            unvalidated.validate_with(|s| s.parse::<i32>().map_err(|_| "not a valid number"));

        assert!(res.is_ok());
        assert_eq!(res.unwrap().into_inner(), 123);

        // 変換失敗のケース
        let unvalidated_bad = Unvalidated::new("abc".to_string());
        let res_bad: Result<Validated<i32>, _> =
            unvalidated_bad.validate_with(|s| s.parse::<i32>().map_err(|_| "not a valid number"));

        assert!(res_bad.is_err());
        assert_eq!(res_bad.unwrap_err(), "not a valid number");
    }

    #[test]
    fn test_try_validate_with_allows_fallback() {
        let raw_data = Unvalidated::new("invalid_format".to_string());

        // 1回目のバリデーション: 失敗
        let result = raw_data.try_validate_with(|s| {
            if s.starts_with("valid_") {
                Ok(s)
            } else {
                Err(("Must start with 'valid_'", s)) // 失敗時に元の値を返す
            }
        });

        assert!(result.is_err());
        let (err_msg, recovered_raw) = result.unwrap_err();
        assert_eq!(err_msg, "Must start with 'valid_'");

        // 取り戻した生データを map で修正してリトライ
        let fixed_data = recovered_raw.map(|s| format!("valid_{}", s));

        // 2回目のバリデーション: 成功
        let final_result = fixed_data.try_validate_with(|s| {
            if s.starts_with("valid_") {
                Ok(s)
            } else {
                Err(("Must start with 'valid_'", s))
            }
        });

        assert!(final_result.is_ok());
        assert_eq!(final_result.unwrap().into_inner(), "valid_invalid_format");
    }

    #[test]
    fn test_try_validate_ref_without_cloning() {
        // String をクローンせずに参照だけで検証
        let raw_data = Unvalidated::new("short".to_string());

        // 参照で検証。失敗時は元の `Unvalidated<String>` が返ってくる
        let result = raw_data.try_validate_ref(|s| {
            if s.len() >= 8 {
                Ok(())
            } else {
                Err("Too short")
            }
        });

        assert!(result.is_err());
        let (err_msg, recovered_raw) = result.unwrap_err();
        assert_eq!(err_msg, "Too short");

        // 元のデータを再利用して処理を続行
        assert_eq!(recovered_raw.into_inner(), "short");
    }

    #[test]
    fn test_validate_trait_and_try_from() {
        // 成功ケース（10は0より大きい）
        let unvalidated = Unvalidated::new(PositiveI32(10));
        let validated: Result<Validated<PositiveI32>, _> = unvalidated.try_into();
        assert!(validated.is_ok());
        assert_eq!(validated.unwrap().into_inner(), PositiveI32(10));

        // 失敗ケース（-5は0より大きくない）
        let unvalidated_err = Unvalidated::new(PositiveI32(-5));
        let validated_err: Result<Validated<PositiveI32>, _> = unvalidated_err.try_into();
        assert_eq!(validated_err.unwrap_err(), "Value must be positive");
    }

    #[test]
    fn test_validate_into() {
        // `validate_into` のテスト
        let valid = PositiveI32(5).validate_into();
        assert!(valid.is_ok());

        let invalid = PositiveI32(-5).validate_into();
        assert!(invalid.is_err());
    }

    #[test]
    fn test_deref_and_as_ref() {
        let validated = Validated::new_unchecked("rust".to_string());

        // Deref トレイトによる自動参照外し
        assert_eq!(validated.len(), 4);
        assert_eq!(*validated, "rust".to_string());

        // AsRef トレイトのテスト
        let unvalidated = Unvalidated::new(99);
        assert_eq!(unvalidated.as_ref(), &99);
        assert_eq!(validated.as_ref(), &"rust".to_string());
    }

    #[test]
    fn test_os_file_drop_validation_and_conversion() {
        // OSからドロップされたパス（String）を検証した上で PathBuf に変換して格納するケース
        let exe_path = std::env::current_exe().unwrap();
        let raw_drop_success = Unvalidated::new(exe_path.to_string_lossy().into_owned());

        let validated_success: Result<Validated<PathBuf>, _> =
            raw_drop_success.validate_with(|path_str| {
                let path = PathBuf::from(path_str);
                if path.exists() && path.is_file() {
                    Ok(path)
                } else {
                    Err("Dropped path must be an existing file")
                }
            });

        assert!(validated_success.is_ok());
        assert_eq!(validated_success.unwrap().into_inner(), exe_path);

        // 異常系: 存在しない不正なファイルパス（String）のケース
        let bad_path = "nonexistent_dummy_file_12345.txt".to_string();
        let raw_drop_failure = Unvalidated::new(bad_path);

        let validated_failure: Result<Validated<PathBuf>, _> =
            raw_drop_failure.validate_with(|path_str| {
                let path = PathBuf::from(path_str);
                if path.exists() && path.is_file() {
                    Ok(path)
                } else {
                    Err("Dropped path must be an existing file")
                }
            });

        assert!(validated_failure.is_err());
        assert_eq!(
            validated_failure.unwrap_err(),
            "Dropped path must be an existing file"
        );
    }

    #[test]
    fn test_os_clipboard_trim_and_validate() {
        // 正常系: 余計なスペースや改行が混じった、OSクリップボードからのペースト
        let raw_clipboard = Unvalidated::new("  https://michiu.org  \r\n".to_string());

        let validated_clipboard = raw_clipboard
            .map(|text| text.trim().to_string())
            .validate_with(|url| {
                if url.starts_with("https://") {
                    Ok(url)
                } else {
                    Err("Only secure HTTPS URLs are permitted")
                }
            });

        assert!(validated_clipboard.is_ok());
        assert_eq!(
            validated_clipboard.unwrap().into_inner(),
            "https://michiu.org"
        );

        // 異常系: 前処理はパスするがhttpのケース
        let raw_insecure = Unvalidated::new("  http://insecure.org  ".to_string());
        let validated_insecure = raw_insecure
            .map(|text| text.trim().to_string())
            .validate_with(|url| {
                if url.starts_with("https://") {
                    Ok(url)
                } else {
                    Err("Only secure HTTPS URLs are permitted")
                }
            });

        assert!(validated_insecure.is_err());
        assert_eq!(
            validated_insecure.unwrap_err(),
            "Only secure HTTPS URLs are permitted"
        );
    }

    #[test]
    fn test_validated_borrow_in_hashmap() {
        let mut map = HashMap::new();
        let key = Validated::new_unchecked("user_alice".to_string());
        map.insert(key, 100);

        assert_eq!(map.get("user_alice"), Some(&100));
    }

    #[test]
    fn test_display_implementation() {
        let validated = Validated::new_unchecked("safe".to_string());
        let unvalidated = Unvalidated::new("raw".to_string());

        assert_eq!(format!("{}", validated), "safe");
        assert_eq!(format!("{}", unvalidated), "raw");
    }

    #[cfg(feature = "serde")]
    #[test]
    fn test_serde_unvalidated_deserialize() {
        // `Unvalidated` は検証を行わないため
        // バリデーションに違反する値（-5）であっても正常にデシリアライズ
        let json_data = "-5";
        let res: Result<Unvalidated<PositiveI32>, serde_json::Error> =
            serde_json::from_str(json_data);

        assert!(res.is_ok());
        assert_eq!(res.unwrap().into_inner(), PositiveI32(-5));
    }

    #[cfg(feature = "serde")]
    #[test]
    fn test_serde_validated_deserialize_success() {
        // バリデーションを満たす正しい値（10）は
        // デシリアライズと同時に検証をパスし、`Validated<T>` として取得
        let json_data = "10";
        let res: Result<Validated<PositiveI32>, serde_json::Error> =
            serde_json::from_str(json_data);

        assert!(res.is_ok());
        assert_eq!(res.unwrap().into_inner(), PositiveI32(10));
    }

    #[cfg(feature = "serde")]
    #[test]
    fn test_serde_validated_deserialize_failure() {
        // バリデーションに違反する値（-5）の場合、
        // デシリアライズ自体が失敗し、定義した検証エラーが serde_json のエラーとして返る。
        let json_data = "-5";
        let res: Result<Validated<PositiveI32>, serde_json::Error> =
            serde_json::from_str(json_data);

        assert!(res.is_err());

        let err_msg = res.unwrap_err().to_string();
        assert!(err_msg.contains("Value must be positive"));
    }

    #[cfg(feature = "serde")]
    #[test]
    fn test_serde_struct_with_validated_field() {
        #[derive(Debug, serde::Deserialize)]
        struct Config {
            #[allow(dead_code)]
            id: String,
            value: Validated<PositiveI32>,
        }

        // 正常系
        let ok_json = r#"{"id": "test-id", "value": 42}"#;
        let config_ok: Result<Config, serde_json::Error> = serde_json::from_str(ok_json);

        assert!(config_ok.is_ok());
        assert_eq!(config_ok.unwrap().value.into_inner(), PositiveI32(42));

        // 異常系
        let bad_json = r#"{"id": "test-id", "value": -1}"#;
        let config_bad: Result<Config, serde_json::Error> = serde_json::from_str(bad_json);

        assert!(config_bad.is_err());
        assert!(
            config_bad
                .unwrap_err()
                .to_string()
                .contains("Value must be positive")
        );
    }
}
