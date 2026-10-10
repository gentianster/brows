pub struct Strings {
    // Picker
    pub which_browser: &'static str,
    pub cancel: &'static str,
    pub settings: &'static str,
    // Tray
    #[cfg(windows)]
    pub tray_settings: &'static str,
    // Update
    pub update_suffix: &'static str,
    #[cfg(windows)]
    pub update_error_prefix: &'static str,
    #[cfg(windows)]
    pub up_to_date: &'static str,
    #[cfg(windows)]
    pub btn_check_update: &'static str,
    #[cfg(windows)]
    pub checking: &'static str,
    #[cfg(windows)]
    pub check_failed: &'static str,
    #[cfg(windows)]
    pub btn_download: &'static str,
    #[cfg(windows)]
    pub downloading: &'static str,
    #[cfg(windows)]
    pub btn_restart: &'static str,
    #[cfg(windows)]
    pub dl_complete: &'static str,
    #[cfg(target_os = "linux")]
    pub update_disabled: &'static str,
    // Settings header
    pub window_title_settings: &'static str,
    pub subtitle: &'static str,
    // Registration
    pub registered: &'static str,
    pub not_registered: &'static str,
    pub btn_register: &'static str,
    pub btn_unregister: &'static str,
    pub register_success_hint: &'static str,
    pub unregister_success: &'static str,
    // Startup
    pub startup_checkbox: &'static str,
    pub startup_hint: &'static str,
    // URL Rules
    pub section_url_rules: &'static str,
    pub btn_open_config: &'static str,
    pub no_rules: &'static str,
    pub search_hint: &'static str,
    pub pattern_hint: &'static str,
    pub btn_add: &'static str,
    // Browsers
    pub section_browsers: &'static str,
    pub no_browsers: &'static str,
}

static JA: Strings = Strings {
    which_browser: "どのブラウザで開きますか？",
    cancel: "キャンセル",
    settings: "設定",
    #[cfg(windows)]
    tray_settings: "設定を開く",
    update_suffix: "公開中",
    #[cfg(windows)]
    update_error_prefix: "更新エラー: ",
    #[cfg(windows)]
    up_to_date: "最新バージョン",
    #[cfg(windows)]
    btn_check_update: "更新を確認",
    #[cfg(windows)]
    checking: "確認中...",
    #[cfg(windows)]
    check_failed: "最新版を取得できませんでした",
    #[cfg(windows)]
    btn_download: "ダウンロード",
    #[cfg(windows)]
    downloading: "DL中...",
    #[cfg(windows)]
    btn_restart: "再起動",
    #[cfg(windows)]
    dl_complete: "DL完了",
    #[cfg(target_os = "linux")]
    update_disabled: "Linux では自動更新は未対応",
    window_title_settings: "brows - 設定",
    subtitle: if cfg!(windows) {
        "ブラウザ選択ツール for Windows 11"
    } else {
        "ブラウザ選択ツール for Linux"
    },
    registered: "登録済み",
    not_registered: "未登録",
    btn_register: "登録",
    btn_unregister: "解除",
    register_success_hint: if cfg!(windows) {
        "設定 → アプリ → 既定のアプリ から brows を選択してください。"
    } else {
        "システム設定の既定のブラウザーに brows を選択してください。"
    },
    unregister_success: "登録を解除しました。",
    startup_checkbox: if cfg!(windows) {
        "Windows 起動時に常駐する"
    } else {
        "ログイン時に常駐する"
    },
    startup_hint: "ログオン直後からピッカーを即時表示できます",
    section_url_rules: "URL ルール",
    btn_open_config: "設定ファイルを開く",
    no_rules: "ルールなし",
    search_hint: "検索...",
    pattern_hint: "パターン (例: github.com)",
    btn_add: "追加",
    section_browsers: "検出済みブラウザ",
    no_browsers: "ブラウザが見つかりませんでした",
};

static EN: Strings = Strings {
    which_browser: "Open with which browser?",
    cancel: "Cancel",
    settings: "Settings",
    #[cfg(windows)]
    tray_settings: "Open settings",
    update_suffix: "available",
    #[cfg(windows)]
    update_error_prefix: "Update error: ",
    #[cfg(windows)]
    up_to_date: "Up to date",
    #[cfg(windows)]
    btn_check_update: "Check for updates",
    #[cfg(windows)]
    checking: "Checking...",
    #[cfg(windows)]
    check_failed: "could not fetch the latest release",
    #[cfg(windows)]
    btn_download: "Download",
    #[cfg(windows)]
    downloading: "Downloading...",
    #[cfg(windows)]
    btn_restart: "Restart",
    #[cfg(windows)]
    dl_complete: "Ready to restart",
    #[cfg(target_os = "linux")]
    update_disabled: "Automatic updates are not available on Linux",
    window_title_settings: "brows - Settings",
    subtitle: if cfg!(windows) {
        "Browser picker for Windows 11"
    } else {
        "Browser picker for Linux"
    },
    registered: "Registered",
    not_registered: "Not registered",
    btn_register: "Register",
    btn_unregister: "Unregister",
    register_success_hint: if cfg!(windows) {
        "Go to Settings → Apps → Default apps and select brows."
    } else {
        "Select brows as the default browser in system settings."
    },
    unregister_success: "Unregistered successfully.",
    startup_checkbox: if cfg!(windows) {
        "Start with Windows"
    } else {
        "Start at login"
    },
    startup_hint: "Shows the picker instantly right after logon",
    section_url_rules: "URL Rules",
    btn_open_config: "Open config file",
    no_rules: "No rules",
    search_hint: "Search...",
    pattern_hint: "Pattern (e.g. github.com)",
    btn_add: "Add",
    section_browsers: "Detected browsers",
    no_browsers: "No browsers found",
};

pub fn get() -> &'static Strings {
    if is_japanese() {
        &JA
    } else {
        &EN
    }
}

#[cfg(windows)]
fn is_japanese() -> bool {
    extern "system" {
        fn GetUserDefaultUILanguage() -> u16;
    }
    let primary = unsafe { GetUserDefaultUILanguage() } & 0x3FF;
    primary == 0x11
}

#[cfg(target_os = "linux")]
fn is_japanese() -> bool {
    ["LC_ALL", "LC_MESSAGES", "LANG"]
        .into_iter()
        .filter_map(|name| std::env::var(name).ok())
        .find(|value| !value.is_empty())
        .is_some_and(|value| locale_is_japanese(&value))
}

#[cfg(target_os = "linux")]
fn locale_is_japanese(locale: &str) -> bool {
    locale
        .split(['_', '-', '.', '@'])
        .next()
        .is_some_and(|language| language.eq_ignore_ascii_case("ja"))
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::locale_is_japanese;

    #[test]
    fn japanese_locales_include_region_and_encoding() {
        for locale in ["ja", "ja_JP.UTF-8", "ja-JP", "JA_JP@modifier"] {
            assert!(locale_is_japanese(locale));
        }
        for locale in ["C", "en_US.UTF-8", "javanese", ""] {
            assert!(!locale_is_japanese(locale));
        }
    }
}
