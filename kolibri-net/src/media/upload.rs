use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use super::http::{self, HttpResponse, ParsedUrl};
use super::{MediaError, ProgressFn};

const USER_AGENT: &str =
    "OKMessages/26.14.1 (Android 11; TECNO MOBILE LIMITED TECNO LE7n; xxhdpi 480dpi 1080x2208)";

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(300);

/// Single POST with a `Content-Range` covering the whole body. status 200 = ok.
pub async fn upload_file(
    url: &str,
    data: &[u8],
    filename: &str,
    insecure: bool,
    progress: Option<ProgressFn>,
) -> Result<HttpResponse, MediaError> {
    let parsed = ParsedUrl::parse(url)?;
    let total = data.len() as u64;
    let headers = vec![
        ("Host", parsed.host.clone()),
        (
            "Content-Type",
            "application/x-binary; charset=x-user-defined".to_string(),
        ),
        (
            "Content-Disposition",
            format!("attachment; filename={filename}"),
        ),
        ("Connection", "keep-alive".to_string()),
        ("User-Agent", percent_encode(USER_AGENT)),
        (
            "Content-Range",
            format!("bytes 0-{}/{}", total.saturating_sub(1), total),
        ),
        ("Content-Length", total.to_string()),
    ];
    http::request(
        &parsed,
        "POST",
        &headers,
        data,
        insecure,
        DEFAULT_TIMEOUT,
        progress.as_ref(),
        total,
    )
    .await
}

/// `multipart/form-data`; caller extracts `photoToken` from the JSON body.
pub async fn upload_photo(
    url: &str,
    data: &[u8],
    filename: &str,
    insecure: bool,
    progress: Option<ProgressFn>,
) -> Result<HttpResponse, MediaError> {
    let parsed = ParsedUrl::parse(url)?;
    let boundary = format!("----KolibriBoundary{}", now_micros());
    let preamble = format!(
        "--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"{filename}\"\r\nContent-Type: {}\r\n\r\n",
        content_type_for_filename(filename)
    );
    let epilogue = format!("\r\n--{boundary}--\r\n");

    let mut body = Vec::with_capacity(preamble.len() + data.len() + epilogue.len());
    body.extend_from_slice(preamble.as_bytes());
    body.extend_from_slice(data);
    body.extend_from_slice(epilogue.as_bytes());
    let total = body.len() as u64;

    let headers = vec![
        ("Host", parsed.host.clone()),
        (
            "Content-Type",
            format!("multipart/form-data; boundary={boundary}"),
        ),
        ("Content-Length", total.to_string()),
        ("Connection", "keep-alive".to_string()),
        ("User-Agent", percent_encode(USER_AGENT)),
    ];
    http::request(
        &parsed,
        "POST",
        &headers,
        &body,
        insecure,
        Duration::from_secs(120),
        progress.as_ref(),
        total,
    )
    .await
}

/// Parallel-chunk video upload with resume. GET handshake returns the resume
/// offset, then each `chunk_size` range is POSTed by up to `concurrency` workers.
pub async fn upload_video(
    url: &str,
    data: Vec<u8>,
    chunk_size: usize,
    concurrency: usize,
    insecure: bool,
    progress: Option<ProgressFn>,
) -> Result<bool, MediaError> {
    let parsed = Arc::new(ParsedUrl::parse(url)?);
    let total = data.len();
    if total == 0 {
        return Ok(false);
    }
    let filename = Arc::new(now_micros().to_string());
    let data = Arc::new(data);

    let handshake = ok_cdn_request(&parsed, "GET", &filename, &[], None, insecure).await?;
    if handshake.status != 200 {
        return Ok(false);
    }
    let mut start_offset = 0usize;
    if let Ok(resumed) = String::from_utf8_lossy(&handshake.body).trim().parse::<usize>() {
        if resumed <= total {
            start_offset = resumed;
        }
    }

    let mut ranges = Vec::new();
    let mut o = start_offset;
    while o < total {
        let end = (o + chunk_size).min(total);
        ranges.push((o, end));
        o = end;
    }
    if ranges.is_empty() {
        return Ok(true);
    }
    let ranges = Arc::new(ranges);

    let next = Arc::new(AtomicUsize::new(0));
    let sent = Arc::new(AtomicUsize::new(start_offset));
    let worker_count = concurrency.max(1).min(ranges.len());

    let mut handles = Vec::with_capacity(worker_count);
    for _ in 0..worker_count {
        let parsed = parsed.clone();
        let filename = filename.clone();
        let data = data.clone();
        let ranges = ranges.clone();
        let next = next.clone();
        let sent = sent.clone();
        let progress = progress.clone();
        handles.push(tokio::spawn(async move {
            loop {
                let i = next.fetch_add(1, Ordering::SeqCst);
                if i >= ranges.len() {
                    return Ok::<(), MediaError>(());
                }
                let (start, end) = ranges[i];
                let range = format!("bytes {start}-{}/{total}", end - 1);
                let resp = ok_cdn_request(
                    &parsed,
                    "POST",
                    &filename,
                    &data[start..end],
                    Some(&range),
                    insecure,
                )
                .await?;
                if resp.status != 200 && resp.status != 201 {
                    return Err(MediaError::Http(resp.status));
                }
                let done = sent.fetch_add(end - start, Ordering::SeqCst) + (end - start);
                if let Some(cb) = &progress {
                    cb(done as u64, total as u64);
                }
            }
        }));
    }

    for handle in handles {
        match handle.await {
            Ok(Ok(())) => {}
            Ok(Err(_)) => return Ok(false),
            Err(_) => return Ok(false),
        }
    }
    Ok(true)
}

async fn ok_cdn_request(
    url: &ParsedUrl,
    method: &str,
    filename: &str,
    body: &[u8],
    content_range: Option<&str>,
    insecure: bool,
) -> Result<HttpResponse, MediaError> {
    let mut headers = vec![
        ("Host", url.host.clone()),
        (
            "Content-Type",
            "application/x-binary; charset=x-user-defined".to_string(),
        ),
        (
            "Content-Disposition",
            format!("attachment; fileName=\"{filename}\""),
        ),
        ("Content-Length", body.len().to_string()),
        ("X-Uploading-Mode", "parallel".to_string()),
        ("Connection", "close".to_string()),
    ];
    if let Some(range) = content_range {
        headers.push(("Content-Range", range.to_string()));
    }
    http::request(
        url,
        method,
        &headers,
        body,
        insecure,
        Duration::from_secs(120),
        None,
        body.len() as u64,
    )
    .await
}

pub fn content_type_for_filename(filename: &str) -> &'static str {
    let ext = filename
        .rsplit('.')
        .next()
        .map(|e| e.to_ascii_lowercase())
        .unwrap_or_default();
    match ext.as_str() {
        "png" => "image/png",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "heic" | "heif" => "image/heic",
        "bmp" => "image/bmp",
        _ => "image/jpeg",
    }
}

fn now_micros() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_micros() as u64 & 0x7FFF_FFFF)
        .unwrap_or(0)
}

// matches Dart's Uri.encodeComponent; unreserved chars pass through
fn percent_encode(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for &b in input.as_bytes() {
        match b {
            b'A'..=b'Z'
            | b'a'..=b'z'
            | b'0'..=b'9'
            | b'-'
            | b'_'
            | b'.'
            | b'!'
            | b'~'
            | b'*'
            | b'\''
            | b'('
            | b')' => out.push(b as char),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}
