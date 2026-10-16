use std::net::{IpAddr, SocketAddr, UdpSocket};

use orrin_core::daemon::DaemonInfo;

use crate::service::Inner;

impl Inner {
    pub fn addresses(&self) -> Vec<String> {
        let port = DaemonInfo::read(&self.home).map(|d| d.port).unwrap_or(self.config.server.port);
        let bind = self.config.server.bind.as_str();
        match bind.parse::<IpAddr>() {
            Ok(ip) if ip.is_unspecified() => {
                let mut found: Vec<String> =
                    lan_ip().map(|ip| SocketAddr::new(ip, port).to_string()).into_iter().collect();
                found.push(format!("{}:{port}", self.me.name));
                found
            }
            Ok(ip) => vec![SocketAddr::new(ip, port).to_string()],
            Err(_) => vec![format!("{bind}:{port}")],
        }
    }

    pub fn address(&self) -> String {
        self.addresses().into_iter().next().unwrap_or_default()
    }
}

fn lan_ip() -> Option<IpAddr> {
    let socket = UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect("8.8.8.8:80").ok()?;
    let ip = socket.local_addr().ok()?.ip();
    (!ip.is_unspecified() && !ip.is_loopback()).then_some(ip)
}

pub fn url(address: &str, request: &str) -> String {
    let base = address.trim().trim_end_matches('/');
    if base.starts_with("http://") || base.starts_with("https://") {
        format!("{base}/api/{request}")
    } else {
        format!("http://{base}/api/{request}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_urls() {
        assert_eq!(url("10.0.0.2:4520", "sync.get"), "http://10.0.0.2:4520/api/sync.get");
        assert_eq!(url("http://box:1/", "ping"), "http://box:1/api/ping");
    }
}
