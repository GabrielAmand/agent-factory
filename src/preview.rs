use std::collections::HashMap;
use std::fs;
use std::io::{Read, Write};
use std::net::{Ipv4Addr, TcpListener, TcpStream};
use std::path::Path;
use std::time::Duration;

use crate::error::{AppError, ErrorKind};
use crate::protocol::GENERATED_FILE_NAMES;
use crate::workspace::validate_run_id;

const MAX_HTTP_REQUEST_BYTES: usize = 8 * 1024;
const CSP: &str = "default-src 'none'; script-src 'self'; style-src 'self'; connect-src 'self'; img-src 'self' data:; font-src 'none'; media-src 'none'; object-src 'none'; frame-src 'none'; base-uri 'none'; form-action 'none'";

struct PreviewFile {
    content_type: &'static str,
    bytes: Vec<u8>,
}

struct PreviewSite {
    files: HashMap<&'static str, PreviewFile>,
}

impl PreviewSite {
    fn load(workspace_root: &Path, run_id: &str) -> Result<Self, AppError> {
        validate_run_id(run_id)?;
        reject_symlink(workspace_root, "workspace root")?;
        let canonical_root = workspace_root.canonicalize().map_err(|error| {
            AppError::new(
                ErrorKind::Preview,
                format!("could not resolve workspace root: {error}"),
            )
        })?;
        let run_path = workspace_root.join(format!("run-{run_id}"));
        reject_symlink(&run_path, "run workspace")?;
        let canonical_run = run_path.canonicalize().map_err(|error| {
            AppError::new(
                ErrorKind::Preview,
                format!("could not resolve run workspace: {error}"),
            )
        })?;
        if !canonical_run.starts_with(&canonical_root) || !canonical_run.is_dir() {
            return Err(AppError::new(
                ErrorKind::Preview,
                "preview workspace must stay inside the workspace root",
            ));
        }

        let mut files = HashMap::new();
        for name in GENERATED_FILE_NAMES {
            let path = canonical_run.join(name);
            let metadata = fs::symlink_metadata(&path).map_err(|error| {
                AppError::new(
                    ErrorKind::Preview,
                    format!("preview file {name} is unavailable: {error}"),
                )
            })?;
            if metadata.file_type().is_symlink() || !metadata.is_file() {
                return Err(AppError::new(
                    ErrorKind::Preview,
                    format!("preview file {name} must be a regular file"),
                ));
            }
            let bytes = fs::read(&path).map_err(|error| {
                AppError::new(
                    ErrorKind::Preview,
                    format!("could not read preview file {name}: {error}"),
                )
            })?;
            files.insert(
                route_for(name),
                PreviewFile {
                    content_type: content_type_for(name),
                    bytes,
                },
            );
        }
        Ok(Self { files })
    }
}

pub fn serve(workspace_root: &Path, run_id: &str, port: u16) -> Result<(), AppError> {
    if port < 1024 {
        return Err(AppError::new(
            ErrorKind::Preview,
            "preview port must be between 1024 and 65535",
        ));
    }
    let site = PreviewSite::load(workspace_root, run_id)?;
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, port)).map_err(|error| {
        AppError::new(
            ErrorKind::Preview,
            format!("could not bind preview server: {error}"),
        )
    })?;
    eprintln!("agent-factory: preview available at http://127.0.0.1:{port}/");
    for connection in listener.incoming() {
        let mut stream = connection.map_err(|error| {
            AppError::new(
                ErrorKind::Preview,
                format!("could not accept preview connection: {error}"),
            )
        })?;
        handle_connection(&mut stream, &site)?;
    }
    Ok(())
}

fn handle_connection(stream: &mut TcpStream, site: &PreviewSite) -> Result<(), AppError> {
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .map_err(|error| {
            AppError::new(
                ErrorKind::Preview,
                format!("could not set preview timeout: {error}"),
            )
        })?;
    let mut request = Vec::new();
    let mut buffer = [0_u8; 1024];
    loop {
        let count = stream.read(&mut buffer).map_err(|error| {
            AppError::new(
                ErrorKind::Preview,
                format!("could not read preview request: {error}"),
            )
        })?;
        if count == 0 {
            break;
        }
        request.extend_from_slice(&buffer[..count]);
        if request.windows(4).any(|window| window == b"\r\n\r\n") {
            break;
        }
        if request.len() > MAX_HTTP_REQUEST_BYTES {
            break;
        }
    }
    let response = build_response(&request, site);
    stream.write_all(&response).map_err(|error| {
        AppError::new(
            ErrorKind::Preview,
            format!("could not write preview response: {error}"),
        )
    })
}

fn build_response(request: &[u8], site: &PreviewSite) -> Vec<u8> {
    let parsed = parse_request(request);
    let (status, content_type, body, is_head) = match parsed {
        Ok((method, route)) => match site.files.get(route) {
            Some(file) => (
                "200 OK",
                file.content_type,
                file.bytes.as_slice(),
                method == "HEAD",
            ),
            None => (
                "404 Not Found",
                "text/plain; charset=utf-8",
                b"Not Found".as_slice(),
                method == "HEAD",
            ),
        },
        Err(RequestError::Method) => (
            "405 Method Not Allowed",
            "text/plain; charset=utf-8",
            b"Method Not Allowed".as_slice(),
            false,
        ),
        Err(RequestError::Route) => (
            "404 Not Found",
            "text/plain; charset=utf-8",
            b"Not Found".as_slice(),
            false,
        ),
        Err(RequestError::Malformed) => (
            "400 Bad Request",
            "text/plain; charset=utf-8",
            b"Bad Request".as_slice(),
            false,
        ),
    };
    let headers = format!(
        "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nContent-Security-Policy: {CSP}\r\nX-Content-Type-Options: nosniff\r\nReferrer-Policy: no-referrer\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n",
        body.len()
    );
    let mut response = headers.into_bytes();
    if !is_head {
        response.extend_from_slice(body);
    }
    response
}

#[derive(Debug)]
enum RequestError {
    Malformed,
    Method,
    Route,
}

fn parse_request(request: &[u8]) -> Result<(&str, &'static str), RequestError> {
    if request.len() > MAX_HTTP_REQUEST_BYTES {
        return Err(RequestError::Malformed);
    }
    let text = std::str::from_utf8(request).map_err(|_| RequestError::Malformed)?;
    let header_end = text.find("\r\n\r\n").ok_or(RequestError::Malformed)?;
    if request.len() != header_end + 4 {
        return Err(RequestError::Malformed);
    }
    let headers = &text[..header_end];
    if headers.contains('\0') {
        return Err(RequestError::Malformed);
    }
    let request_line = headers
        .split("\r\n")
        .next()
        .ok_or(RequestError::Malformed)?;
    let parts: Vec<&str> = request_line.split(' ').collect();
    if parts.len() != 3
        || parts.iter().any(|part| part.is_empty())
        || !matches!(parts[2], "HTTP/1.0" | "HTTP/1.1")
    {
        return Err(RequestError::Malformed);
    }
    if !matches!(parts[0], "GET" | "HEAD") {
        return Err(RequestError::Method);
    }
    let mut host_count = 0;
    for header in headers.split("\r\n").skip(1) {
        if header.starts_with([' ', '\t']) {
            return Err(RequestError::Malformed);
        }
        let (name, value) = header.split_once(':').ok_or(RequestError::Malformed)?;
        if name.is_empty()
            || name
                .bytes()
                .any(|byte| !byte.is_ascii_alphanumeric() && byte != b'-')
        {
            return Err(RequestError::Malformed);
        }
        if name.eq_ignore_ascii_case("host") {
            host_count += 1;
            if value.trim().is_empty() {
                return Err(RequestError::Malformed);
            }
        }
        if name.eq_ignore_ascii_case("transfer-encoding")
            || (name.eq_ignore_ascii_case("content-length") && value.trim() != "0")
        {
            return Err(RequestError::Malformed);
        }
    }
    if parts[2] == "HTTP/1.1" && host_count != 1 {
        return Err(RequestError::Malformed);
    }
    if parts[1].contains(['?', '#', '%']) || parts[1].contains("..") {
        return Err(RequestError::Route);
    }
    let route = match parts[1] {
        "/" | "/index.html" => "/index.html",
        "/app.js" => "/app.js",
        "/styles.css" => "/styles.css",
        "/resources.json" => "/resources.json",
        _ => return Err(RequestError::Route),
    };
    Ok((parts[0], route))
}

fn route_for(name: &str) -> &'static str {
    match name {
        "index.html" => "/index.html",
        "app.js" => "/app.js",
        "styles.css" => "/styles.css",
        "resources.json" => "/resources.json",
        _ => "",
    }
}

fn content_type_for(name: &str) -> &'static str {
    match name {
        "index.html" => "text/html; charset=utf-8",
        "app.js" => "text/javascript; charset=utf-8",
        "styles.css" => "text/css; charset=utf-8",
        "resources.json" => "application/json; charset=utf-8",
        _ => "application/octet-stream",
    }
}

fn reject_symlink(path: &Path, name: &str) -> Result<(), AppError> {
    let metadata = fs::symlink_metadata(path).map_err(|error| {
        AppError::new(
            ErrorKind::Preview,
            format!("{name} is unavailable: {error}"),
        )
    })?;
    if metadata.file_type().is_symlink() {
        return Err(AppError::new(
            ErrorKind::Preview,
            format!("{name} must not be a symbolic link"),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn site() -> PreviewSite {
        let mut files = HashMap::new();
        for name in GENERATED_FILE_NAMES {
            files.insert(
                route_for(name),
                PreviewFile {
                    content_type: content_type_for(name),
                    bytes: name.as_bytes().to_vec(),
                },
            );
        }
        PreviewSite { files }
    }

    fn response(request: &str) -> String {
        String::from_utf8(build_response(request.as_bytes(), &site())).unwrap()
    }

    #[test]
    fn serves_only_approved_get_and_head_routes_with_security_headers() {
        for route in [
            "/",
            "/index.html",
            "/app.js",
            "/styles.css",
            "/resources.json",
        ] {
            let result = response(&format!("GET {route} HTTP/1.1\r\nHost: localhost\r\n\r\n"));
            assert!(result.starts_with("HTTP/1.1 200 OK"));
            assert!(result.contains("Content-Security-Policy:"));
            assert!(result.contains("X-Content-Type-Options: nosniff"));
        }
        let head = response("HEAD /app.js HTTP/1.1\r\nHost: localhost\r\n\r\n");
        assert!(head.ends_with("\r\n\r\n"));
    }

    #[test]
    fn returns_correct_mime_types() {
        for (route, mime) in [
            ("/index.html", "text/html"),
            ("/app.js", "text/javascript"),
            ("/styles.css", "text/css"),
            ("/resources.json", "application/json"),
        ] {
            assert!(
                response(&format!("GET {route} HTTP/1.1\r\nHost: localhost\r\n\r\n"))
                    .contains(&format!("Content-Type: {mime}"))
            );
        }
    }

    #[test]
    fn rejects_methods_paths_queries_encoding_and_malformed_requests() {
        for request in [
            "POST / HTTP/1.1\r\nHost: localhost\r\n\r\n",
            "GET /unknown HTTP/1.1\r\nHost: localhost\r\n\r\n",
            "GET /../index.html HTTP/1.1\r\nHost: localhost\r\n\r\n",
            "GET /index.html?x=1 HTTP/1.1\r\nHost: localhost\r\n\r\n",
            "GET /%69ndex.html HTTP/1.1\r\nHost: localhost\r\n\r\n",
            "GET  / HTTP/1.1\r\nHost: localhost\r\n\r\n",
            "GET / HTTP/1.1\r\n\r\n",
            "GET / HTTP/1.1\r\nHost: localhost\r\nHost: duplicate\r\n\r\n",
            "GET / HTTP/1.1\r\nHost: localhost\r\nTransfer-Encoding: chunked\r\n\r\n",
            "GET / HTTP/1.1\r\nHost: localhost\r\nContent-Length: 1\r\n\r\nx",
        ] {
            assert!(!response(request).starts_with("HTTP/1.1 200 OK"));
        }
    }

    #[cfg(unix)]
    #[test]
    fn refuses_a_symlinked_generated_file() {
        use std::os::unix::fs::symlink;

        let root = tempfile::TempDir::new().unwrap();
        let run = root.path().join("run-safe");
        fs::create_dir(&run).unwrap();
        for name in GENERATED_FILE_NAMES {
            let path = run.join(name);
            if name == "app.js" {
                symlink(run.join("index.html"), path).unwrap();
            } else {
                fs::write(path, name).unwrap();
            }
        }
        assert!(PreviewSite::load(root.path(), "safe").is_err());
    }
}
