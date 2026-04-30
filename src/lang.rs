pub struct Strings {
    // Picker
    pub which_browser: &'static str,
    pub cancel: &'static str,
    // Update
    pub update_suffix: &'static str,
    pub update_error_prefix: &'static str,
    pub up_to_date: &'static str,
    pub btn_download: &'static str,
    pub downloading: &'static str,
    pub btn_restart: &'static str,
    pub dl_complete: &'static str,
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
    update_suffix: "公開中",
    update_error_prefix: "更新エラー: ",
    up_to_date: "最新バージョン",
    btn_download: "ダウンロード",
    downloading: "DL中...",
    btn_restart: "再起動",
    dl_complete: "DL完了",
    window_title_settings: "brows - 設定",
    subtitle: "ブラウザ選択ツール for Windows 11",
    registered: "登録済み",
    not_registered: "未登録",
    btn_register: "登録",
    btn_unregister: "解除",
    register_success_hint: "設定 → アプリ → 既定のアプリ から brows を選択してください。",
    unregister_success: "登録を解除しました。",
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
    update_suffix: "available",
    update_error_prefix: "Update error: ",
    up_to_date: "Up to date",
    btn_download: "Download",
    downloading: "Downloading...",
    btn_restart: "Restart",
    dl_complete: "Ready to restart",
    window_title_settings: "brows - Settings",
    subtitle: "Browser picker for Windows 11",
    registered: "Registered",
    not_registered: "Not registered",
    btn_register: "Register",
    btn_unregister: "Unregister",
    register_success_hint: "Go to Settings → Apps → Default apps and select brows.",
    unregister_success: "Unregistered successfully.",
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
    if is_japanese() { &JA } else { &EN }
}

fn is_japanese() -> bool {
    extern "system" { fn GetUserDefaultUILanguage() -> u16; }
    let primary = unsafe { GetUserDefaultUILanguage() } & 0x3FF;
    primary == 0x11
}
