//! 常駐インスタンスとの通信。Windows の名前付きパイプを使う。
//!
//! 旧実装はループバック TCP（127.0.0.1:48693）だったが、
//! - listen ポートを開かない（ファイアウォール/EDR に優しい・netstat に出ない）
//! - ポート衝突がない
//! - パイプ名にユーザー SID を含めてユーザーごとに分離できる（TCP では困難）
//! 理由から名前付きパイプへ移行した。
//!
//! パイプ名は `\\.\pipe\brows.<SID>`。DACL を「本人 + SYSTEM のフルアクセス」に
//! 限定し、他ユーザー/他プロセスからの接続（URL 注入や転送内容の覗き見）を防ぐ。
//! シングルインスタンス判定は `FILE_FLAG_FIRST_PIPE_INSTANCE` で行う
//! （2 つ目の生成は ERROR_ACCESS_DENIED で失敗する = 既に常駐がいる）。

use windows::core::{PCWSTR, PWSTR};
use windows::Win32::Foundation::{
    CloseHandle, LocalFree, ERROR_PIPE_CONNECTED, GENERIC_READ, GENERIC_WRITE, GetLastError,
    HANDLE, HLOCAL, INVALID_HANDLE_VALUE,
};
use windows::Win32::Security::Authorization::{
    ConvertSidToStringSidW, ConvertStringSecurityDescriptorToSecurityDescriptorW, SDDL_REVISION_1,
};
use windows::Win32::Security::{
    GetTokenInformation, TokenUser, PSECURITY_DESCRIPTOR, SECURITY_ATTRIBUTES, TOKEN_QUERY,
    TOKEN_USER,
};
use windows::Win32::Storage::FileSystem::{
    CreateFileW, FlushFileBuffers, ReadFile, WriteFile, FILE_FLAGS_AND_ATTRIBUTES,
    FILE_FLAG_FIRST_PIPE_INSTANCE, FILE_SHARE_NONE, OPEN_EXISTING, PIPE_ACCESS_DUPLEX,
};
use windows::Win32::System::Pipes::{
    ConnectNamedPipe, CreateNamedPipeW, DisconnectNamedPipe, WaitNamedPipeW, PIPE_READMODE_BYTE,
    PIPE_TYPE_BYTE, PIPE_WAIT,
};
use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

const MSG_OPEN: &str = "BROWS-OPEN ";
const MSG_EXIT: &str = "BROWS-EXIT";
const MSG_ACK: &str = "BROWS-OK";
const BUF_SIZE: u32 = 8192;

pub enum Request {
    Open(String),
    Exit,
}

fn to_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// 現在のユーザーの SID を文字列（"S-1-5-..."）で返す
fn current_user_sid() -> Option<String> {
    unsafe {
        let mut token = HANDLE::default();
        OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token).ok()?;

        // 必要なバッファサイズを取得してから本取得
        let mut len = 0u32;
        let _ = GetTokenInformation(token, TokenUser, None, 0, &mut len);
        let mut buf = vec![0u8; len as usize];
        let got = GetTokenInformation(token, TokenUser, Some(buf.as_mut_ptr() as *mut _), len, &mut len);
        let _ = CloseHandle(token);
        got.ok()?;

        let tu = &*(buf.as_ptr() as *const TOKEN_USER);
        let mut pstr = PWSTR::null();
        ConvertSidToStringSidW(tu.User.Sid, &mut pstr).ok()?;
        let sid = pstr.to_string().ok();
        let _ = LocalFree(HLOCAL(pstr.0 as *mut core::ffi::c_void));
        sid
    }
}

fn pipe_name() -> String {
    match current_user_sid() {
        Some(sid) => format!(r"\\.\pipe\brows.{sid}"),
        None => r"\\.\pipe\brows".to_string(),
    }
}

/// 常駐サーバー側のパイプハンドル。bind 成功 = 自分が常駐になる。
pub struct PipeServer {
    handle: HANDLE,
}

// パイプハンドルは別スレッド（IPC サーバー）へ move して使う
unsafe impl Send for PipeServer {}

impl PipeServer {
    /// 1 クライアントの接続を待ち、リクエストを読んで ack を返す。
    /// 旧 `listener.incoming()` のループ本体に相当（プロトコル不正なら None）。
    pub fn accept(&self) -> Option<Request> {
        unsafe {
            // クライアント接続を待つ。CreateFile が先行していた場合は
            // ERROR_PIPE_CONNECTED が返るが、これは接続成功として扱う。
            if ConnectNamedPipe(self.handle, None).is_err() && GetLastError() != ERROR_PIPE_CONNECTED
            {
                let _ = DisconnectNamedPipe(self.handle);
                return None;
            }
            let req = handle_client(self.handle);
            // ack を相手が読み切るまで待ってから切断する（未読データの破棄を防ぐ）
            let _ = FlushFileBuffers(self.handle);
            let _ = DisconnectNamedPipe(self.handle);
            req
        }
    }
}

impl Drop for PipeServer {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.handle);
        }
    }
}

/// パイプを作成できたら常駐インスタンスになれる（シングルインスタンス判定を兼ねる）
pub fn try_bind() -> Option<PipeServer> {
    let name = to_wide(&pipe_name());
    unsafe {
        // 本人 + SYSTEM のみフルアクセス（GA）の DACL。取得できなければ既定の権限。
        let sddl = current_user_sid().map(|sid| to_wide(&format!("D:(A;;GA;;;{sid})(A;;GA;;;SY)")));
        let mut psd = PSECURITY_DESCRIPTOR::default();
        let mut sa = SECURITY_ATTRIBUTES::default();
        let mut have_sd = false;
        if let Some(ref sddl_w) = sddl {
            if ConvertStringSecurityDescriptorToSecurityDescriptorW(
                PCWSTR(sddl_w.as_ptr()),
                SDDL_REVISION_1,
                &mut psd,
                None,
            )
            .is_ok()
            {
                sa.nLength = std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32;
                sa.lpSecurityDescriptor = psd.0;
                have_sd = true;
            }
        }
        let lpsa = if have_sd {
            Some(&sa as *const SECURITY_ATTRIBUTES)
        } else {
            None
        };

        let h = CreateNamedPipeW(
            PCWSTR(name.as_ptr()),
            PIPE_ACCESS_DUPLEX | FILE_FLAG_FIRST_PIPE_INSTANCE,
            PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT,
            1, // 1 インスタンスだけ（FIRST_PIPE_INSTANCE と合わせてシングルトン）
            BUF_SIZE,
            BUF_SIZE,
            0,
            lpsa,
        );

        if have_sd {
            let _ = LocalFree(HLOCAL(psd.0));
        }

        if h == INVALID_HANDLE_VALUE {
            None
        } else {
            Some(PipeServer { handle: h })
        }
    }
}

/// 常駐インスタンスへ URL を転送する。ack が返れば true
pub fn send_open(url: &str) -> bool {
    send(&format!("{}{}", MSG_OPEN, url))
}

/// 常駐インスタンスを終了させる（自動更新の再起動前に呼ぶ）
pub fn send_exit() -> bool {
    send(MSG_EXIT)
}

fn send(line: &str) -> bool {
    let name = to_wide(&pipe_name());
    unsafe {
        let mut handle = open_client(&name);
        if handle.is_none() {
            // サーバーが他クライアントを処理中（BUSY）なら少し待って再試行
            if WaitNamedPipeW(PCWSTR(name.as_ptr()), 1000).as_bool() {
                handle = open_client(&name);
            }
        }
        let Some(h) = handle else {
            return false;
        };
        let ok = write_line(h, line)
            && read_line(h).map(|r| r.trim() == MSG_ACK).unwrap_or(false);
        let _ = CloseHandle(h);
        ok
    }
}

unsafe fn open_client(name: &[u16]) -> Option<HANDLE> {
    match CreateFileW(
        PCWSTR(name.as_ptr()),
        (GENERIC_READ | GENERIC_WRITE).0,
        FILE_SHARE_NONE,
        None,
        OPEN_EXISTING,
        FILE_FLAGS_AND_ATTRIBUTES(0),
        None,
    ) {
        Ok(h) if h != INVALID_HANDLE_VALUE => Some(h),
        _ => None,
    }
}

/// 常駐側: 受け取った 1 行をリクエストに変換し、ack を返す
unsafe fn handle_client(h: HANDLE) -> Option<Request> {
    let line = read_line(h)?;
    let line = line.trim();

    let req = if let Some(url) = line.strip_prefix(MSG_OPEN) {
        if !url.starts_with("http") {
            return None;
        }
        Request::Open(url.to_string())
    } else if line == MSG_EXIT {
        Request::Exit
    } else {
        return None;
    };

    let _ = write_line(h, MSG_ACK);
    Some(req)
}

/// パイプから改行までの 1 行を読む（バイトモード・小さなメッセージ前提）
unsafe fn read_line(h: HANDLE) -> Option<String> {
    let mut buf: Vec<u8> = Vec::new();
    let mut chunk = [0u8; 256];
    loop {
        let mut read = 0u32;
        if ReadFile(h, Some(&mut chunk), Some(&mut read), None).is_err() || read == 0 {
            break;
        }
        buf.extend_from_slice(&chunk[..read as usize]);
        if buf.contains(&b'\n') || buf.len() > BUF_SIZE as usize {
            break;
        }
    }
    if buf.is_empty() {
        return None;
    }
    let text = String::from_utf8_lossy(&buf);
    Some(text.lines().next().unwrap_or("").to_string())
}

unsafe fn write_line(h: HANDLE, line: &str) -> bool {
    let data = format!("{}\n", line);
    let mut written = 0u32;
    WriteFile(h, Some(data.as_bytes()), Some(&mut written), None).is_ok()
}
