use std::{
    net::SocketAddr,
    sync::OnceLock,
    time::{Duration, Instant},
};

use thiserror::Error;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
};
use tokio_tungstenite::{
    client_async_tls, tungstenite::http::Uri, MaybeTlsStream, WebSocketStream,
};

#[derive(Debug, Error)]
pub enum NetworkError {
    #[error("websocket URL is invalid")]
    InvalidUrl,
    #[error("DNS resolution failed: {0}")]
    Dns(#[source] Box<std::io::Error>),
    #[error("TCP connection failed: {0}")]
    Tcp(#[source] Box<std::io::Error>),
    #[error("HTTP proxy tunnel failed: {0}")]
    Proxy(String),
    #[error("websocket error: {0}")]
    WebSocket(#[source] Box<tokio_tungstenite::tungstenite::Error>),
    #[error("websocket connection timed out")]
    Timeout,
}
pub type ConnectedWebSocket = WebSocketStream<MaybeTlsStream<TcpStream>>;

pub async fn connect_websocket(
    url: &str,
    timeout_ms: u64,
    http_proxy: Option<&str>,
) -> Result<ConnectedWebSocket, NetworkError> {
    if timeout_ms == 0 {
        return Err(NetworkError::Timeout);
    }
    install_rustls_provider();

    let parsed: Uri = url.parse::<Uri>().map_err(|_| NetworkError::InvalidUrl)?;
    let target_host = parsed.host().ok_or(NetworkError::InvalidUrl)?;
    let target_port = parsed
        .port_u16()
        .or_else(|| match parsed.scheme_str() {
            Some("wss") => Some(443),
            Some("ws") => Some(80),
            _ => None,
        })
        .ok_or(NetworkError::InvalidUrl)?;
    let proxy_endpoint = http_proxy.map(parse_proxy_endpoint).transpose()?;
    let (connect_host, connect_port) = proxy_endpoint
        .as_ref()
        .map(|(host, port)| (host.as_str(), *port))
        .unwrap_or((target_host, target_port));
    let mut addresses: Vec<SocketAddr> = tokio::net::lookup_host((connect_host, connect_port))
        .await
        .map_err(|error| NetworkError::Dns(Box::new(error)))?
        .collect();
    addresses.sort_by_key(|address| !address.is_ipv4());
    if addresses.is_empty() {
        return Err(NetworkError::Dns(Box::new(std::io::Error::new(
            std::io::ErrorKind::AddrNotAvailable,
            "host resolved to no addresses",
        ))));
    }

    let deadline = Instant::now() + Duration::from_millis(timeout_ms);
    let mut last_tcp_error = None;
    let mut last_proxy_error = None;
    let mut last_websocket_error = None;
    for address in addresses {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            break;
        }
        let attempt_timeout = remaining.min(Duration::from_millis(1_500));
        match tokio::time::timeout(attempt_timeout, TcpStream::connect(address)).await {
            Ok(Ok(mut socket)) => {
                if proxy_endpoint.is_some() {
                    let remaining = deadline.saturating_duration_since(Instant::now());
                    if remaining.is_zero() {
                        break;
                    }
                    match tokio::time::timeout(
                        remaining,
                        establish_proxy_tunnel(&mut socket, target_host, target_port),
                    )
                    .await
                    {
                        Ok(Ok(())) => {}
                        Ok(Err(error)) => {
                            last_proxy_error = Some(error);
                            continue;
                        }
                        Err(_) => break,
                    }
                }
                let remaining = deadline.saturating_duration_since(Instant::now());
                if remaining.is_zero() {
                    break;
                }
                match tokio::time::timeout(remaining, client_async_tls(url.to_owned(), socket))
                    .await
                {
                    Ok(Ok((socket, _response))) => return Ok(socket),
                    Ok(Err(error)) => last_websocket_error = Some(error),
                    Err(_) => break,
                }
            }
            Ok(Err(error)) => last_tcp_error = Some(error),
            Err(_) => {}
        }
    }
    if let Some(error) = last_proxy_error {
        return Err(NetworkError::Proxy(error));
    }
    if let Some(error) = last_websocket_error {
        return Err(NetworkError::WebSocket(Box::new(error)));
    }
    if let Some(error) = last_tcp_error {
        return Err(NetworkError::Tcp(Box::new(error)));
    }
    Err(NetworkError::Timeout)
}
fn install_rustls_provider() {
    static PROVIDER: OnceLock<()> = OnceLock::new();
    PROVIDER.get_or_init(|| {
        let _ = rustls::crypto::ring::default_provider().install_default();
    });
}

fn parse_proxy_endpoint(proxy_url: &str) -> Result<(String, u16), NetworkError> {
    let normalized = if proxy_url.contains("://") {
        proxy_url.to_owned()
    } else {
        format!("http://{proxy_url}")
    };
    let parsed: Uri = normalized
        .parse::<Uri>()
        .map_err(|_| NetworkError::Proxy("invalid HTTP proxy URL".to_owned()))?;
    if parsed.scheme_str().is_some_and(|scheme| scheme != "http") {
        return Err(NetworkError::Proxy(
            "only HTTP CONNECT proxies are supported".to_owned(),
        ));
    }
    let host = parsed
        .host()
        .ok_or_else(|| NetworkError::Proxy("HTTP proxy URL has no host".to_owned()))?;
    Ok((host.to_owned(), parsed.port_u16().unwrap_or(80)))
}

async fn establish_proxy_tunnel(
    socket: &mut TcpStream,
    target_host: &str,
    target_port: u16,
) -> Result<(), String> {
    let authority = format!("{target_host}:{target_port}");
    let request = format!(
        "CONNECT {authority} HTTP/1.1\r\nHost: {authority}\r\nProxy-Connection: Keep-Alive\r\n\r\n"
    );
    socket
        .write_all(request.as_bytes())
        .await
        .map_err(|error| error.to_string())?;
    let mut response = Vec::with_capacity(1024);
    let mut chunk = [0_u8; 1024];
    loop {
        let read = socket
            .read(&mut chunk)
            .await
            .map_err(|error| error.to_string())?;
        if read == 0 {
            return Err("proxy closed before CONNECT response".to_owned());
        }
        response.extend_from_slice(&chunk[..read]);
        if response.windows(4).any(|window| window == b"\r\n\r\n") {
            break;
        }
        if response.len() > 8 * 1024 {
            return Err("proxy CONNECT response exceeds 8 KiB".to_owned());
        }
    }
    let header = String::from_utf8_lossy(&response);
    let status = header.lines().next().unwrap_or_default();
    if status.split_whitespace().nth(1) != Some("200") {
        return Err(format!("proxy CONNECT rejected: {status}"));
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_http_proxy_endpoint_without_enabling_it_by_default() {
        assert_eq!(
            parse_proxy_endpoint("127.0.0.1:7890").unwrap(),
            ("127.0.0.1".to_owned(), 7890)
        );
        assert!(matches!(
            parse_proxy_endpoint("https://127.0.0.1:7890"),
            Err(NetworkError::Proxy(_))
        ));
    }

    #[tokio::test]
    async fn rejects_zero_timeout_before_network_access() {
        assert!(matches!(
            connect_websocket("wss://demo-fstream.binance.com", 0, None).await,
            Err(NetworkError::Timeout)
        ));
    }
}
