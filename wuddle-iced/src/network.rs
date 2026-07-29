use reqwest::{
    header::{CONTENT_LENGTH, LOCATION},
    redirect::Policy,
    Client, Url,
};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};

const MAX_REDIRECTS: usize = 6;

#[derive(Debug)]
pub(crate) struct PublicDownload {
    pub bytes: Vec<u8>,
    pub content_type: Option<String>,
    pub final_url: Url,
}

fn is_public_ipv4(address: Ipv4Addr) -> bool {
    let octets = address.octets();
    if address.is_private()
        || address.is_loopback()
        || address.is_link_local()
        || address.is_unspecified()
        || address.is_multicast()
        || address.is_broadcast()
        || address.is_documentation()
    {
        return false;
    }

    // Carrier-grade NAT, benchmarking, reserved, and other non-public ranges.
    !matches!(
        octets,
        [100, 64..=127, _, _] | [192, 0, 0, _] | [198, 18..=19, _, _] | [240..=255, _, _, _]
    )
}

fn is_public_ipv6(address: Ipv6Addr) -> bool {
    let octets = address.octets();
    if address.is_loopback()
        || address.is_unspecified()
        || address.is_multicast()
        || (octets[0] & 0xfe) == 0xfc
        || (octets[0] == 0xfe && (octets[1] & 0xc0) == 0x80)
        || (octets[0..4] == [0x20, 0x01, 0x0d, 0xb8])
    {
        return false;
    }
    address.to_ipv4().is_none_or(is_public_ipv4)
}

pub(crate) fn is_public_ip(address: IpAddr) -> bool {
    match address {
        IpAddr::V4(address) => is_public_ipv4(address),
        IpAddr::V6(address) => is_public_ipv6(address),
    }
}

fn validate_public_https_url(url: &Url) -> Result<(), String> {
    if url.scheme() != "https" {
        return Err("README images must use HTTPS".to_string());
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err("README image URLs may not contain credentials".to_string());
    }
    if !matches!(url.port(), None | Some(443)) {
        return Err("README image URLs may only use the standard HTTPS port".to_string());
    }
    if url.host_str().is_none() {
        return Err("README image URL has no host".to_string());
    }
    Ok(())
}

async fn resolve_public_host(url: &Url) -> Result<(String, Vec<SocketAddr>), String> {
    validate_public_https_url(url)?;
    let host = url
        .host_str()
        .ok_or_else(|| "README image URL has no host".to_string())?
        .to_string();

    let addresses = if let Ok(address) = host.parse::<IpAddr>() {
        vec![SocketAddr::new(address, 443)]
    } else {
        tokio::net::lookup_host((host.as_str(), 443))
            .await
            .map_err(|_| "README image host could not be resolved".to_string())?
            .collect::<Vec<_>>()
    };

    if addresses.is_empty() || addresses.iter().any(|address| !is_public_ip(address.ip())) {
        return Err("README image points to a private or non-public network".to_string());
    }
    Ok((host, addresses))
}

fn content_length(response: &reqwest::Response) -> Result<Option<u64>, String> {
    let Some(value) = response.headers().get(CONTENT_LENGTH) else {
        return Ok(None);
    };
    value
        .to_str()
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .map(Some)
        .ok_or_else(|| "README image returned an invalid Content-Length".to_string())
}

/// Fetch arbitrary README media while preventing local-network access. DNS
/// results are checked and pinned for each request, redirects are handled
/// manually, and both declared and streamed byte counts are bounded.
pub(crate) async fn fetch_public_bytes<A>(
    initial_url: &str,
    max_bytes: u64,
    authorization: A,
) -> Result<PublicDownload, String>
where
    A: Fn(&str) -> Option<String>,
{
    let mut current =
        Url::parse(initial_url).map_err(|_| "README image URL is invalid".to_string())?;

    for redirect_count in 0..=MAX_REDIRECTS {
        let (host, addresses) = resolve_public_host(&current).await?;
        let client = Client::builder()
            .user_agent(concat!("wuddle/", env!("CARGO_PKG_VERSION")))
            .timeout(std::time::Duration::from_secs(10))
            .redirect(Policy::none())
            .no_proxy()
            .resolve_to_addrs(&host, &addresses)
            .build()
            .map_err(|error| error.to_string())?;

        let mut request = client.get(current.clone());
        if let Some(token) = authorization(current.as_str()) {
            request = request.bearer_auth(token);
        }
        let mut response = request.send().await.map_err(|error| error.to_string())?;
        if response.status().is_redirection() {
            if redirect_count == MAX_REDIRECTS {
                return Err("README image exceeded the redirect limit".to_string());
            }
            let location = response
                .headers()
                .get(LOCATION)
                .and_then(|value| value.to_str().ok())
                .ok_or_else(|| "README image redirect omitted its destination".to_string())?;
            current = response
                .url()
                .join(location)
                .map_err(|_| "README image redirect destination is invalid".to_string())?;
            continue;
        }
        if !response.status().is_success() {
            return Err(format!("README image HTTP {}", response.status()));
        }
        if content_length(&response)?.is_some_and(|length| length > max_bytes) {
            return Err("README image exceeds Wuddle's download limit".to_string());
        }

        let content_type = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .map(str::to_string);
        let final_url = response.url().clone();
        let mut bytes = Vec::new();
        let mut received = 0u64;
        while let Some(chunk) = response.chunk().await.map_err(|error| error.to_string())? {
            received = received
                .checked_add(chunk.len() as u64)
                .ok_or_else(|| "README image size overflowed".to_string())?;
            if received > max_bytes {
                return Err("README image exceeds Wuddle's download limit".to_string());
            }
            bytes.extend_from_slice(&chunk);
        }
        return Ok(PublicDownload {
            bytes,
            content_type,
            final_url,
        });
    }

    unreachable!("redirect loop always returns or errors")
}

#[cfg(test)]
mod tests {
    use super::{is_public_ip, validate_public_https_url};
    use reqwest::Url;
    use std::net::IpAddr;

    #[test]
    fn rejects_private_loopback_link_local_and_metadata_addresses() {
        for address in [
            "127.0.0.1",
            "10.0.0.1",
            "172.16.0.1",
            "192.168.1.1",
            "169.254.169.254",
            "100.64.0.1",
            "::1",
            "fe80::1",
            "fc00::1",
            "::ffff:127.0.0.1",
        ] {
            assert!(
                !is_public_ip(address.parse::<IpAddr>().unwrap()),
                "unexpectedly allowed {address}"
            );
        }
        assert!(is_public_ip("1.1.1.1".parse().unwrap()));
        assert!(is_public_ip("2606:4700:4700::1111".parse().unwrap()));
    }

    #[test]
    fn accepts_only_plain_standard_port_https_urls() {
        assert!(
            validate_public_https_url(&Url::parse("https://example.com/image.png").unwrap())
                .is_ok()
        );
        for url in [
            "http://example.com/image.png",
            "https://user@example.com/image.png",
            "https://example.com:8443/image.png",
            "file:///etc/passwd",
        ] {
            assert!(
                validate_public_https_url(&Url::parse(url).unwrap()).is_err(),
                "unexpectedly allowed {url}"
            );
        }
    }
}
