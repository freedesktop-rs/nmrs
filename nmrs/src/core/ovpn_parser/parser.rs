use std::collections::HashMap;
use std::net::Ipv4Addr;

use crate::api::models::ConnectionError;
use crate::core::ovpn_parser::error::OvpnParseError;

#[derive(Debug, Clone)]
pub struct OvpnFile {
    // All remote entries. Each defines a possible server endpoint.
    // OpenVPN tries them in order unless configured otherwise.
    pub remotes: Vec<Remote>,

    // device directive (e.g. "tun", "tap").
    // Controls the virtual network interface type.
    pub dev: Option<String>,

    // protocol directive (e.g. "udp", "tcp-client").
    pub proto: Option<String>,

    // ca directive. Certificate Authority used to verify server cert.
    // Supports file path or inline block.
    pub ca: Option<CertSource>,

    // cert directive. Client certificate.
    pub cert: Option<CertSource>,

    // key directive. Private key corresponding to cert.
    pub key: Option<CertSource>,

    // tls-auth directive. HMAC key used for additional packet auth.
    // This may include key-direction (0/1).
    pub tls_auth: Option<TlsAuth>,

    // tls-crypt directive. Encrypts control channel metadata.
    pub tls_crypt: Option<CertSource>,

    // cipher directive. Legacy data channel cipher (deprecated in newer configs).
    pub cipher: Option<String>,

    // data-ciphers directive. Preferred list of ciphers (this is colon-separated).
    pub data_ciphers: Vec<String>,

    // auth directive. HMAC digest algorithm (e.g. SHA256).
    pub auth: Option<String>,

    // compress directive. Either enabled or specifies algorithm (e.g. "lz4").
    pub compress: Option<Compress>,

    // OpenVPN 2.5+ specifies a allow-compress directive for safety
    // https://community.openvpn.net/Security%20Announcements/VORACLE
    pub allow_compress: Option<AllowCompress>,

    // All route directives.
    // Each represents a network route pushed or defined locally.
    pub routes: Vec<Route>,

    // redirect-gateway flag.
    // Forces all traffic through VPN if present.
    pub redirect_gateway: Option<RedirectGateway>,

    // Standalone flag directives with no arguments.
    // Examples: client, nobind, persist-key, persist-tun.
    pub flags: Vec<String>,

    // auth-user-pass directive. Indicates the server requires
    // username/password authentication. The optional file path argument
    // is ignored (NM handles interactive prompts).
    pub auth_user_pass: bool,

    // Catch-all for unmodeled or less common directives.
    // Key = directive name, Value = list of argument lists.
    // Preserves information for round-tripping / forward compatibility.
    pub options: HashMap<String, Vec<String>>,
}

#[derive(Debug, Clone)]
pub struct Remote {
    pub host: String,
    pub port: Option<u16>,
    pub proto: Option<String>,
}

#[derive(Debug, Clone)]
pub enum CertSource {
    File(String),
    Inline(String),
}

#[derive(Debug, Clone)]
pub struct TlsAuth {
    pub source: CertSource,
    pub key_direction: Option<u8>,
}

#[derive(Debug, Clone)]
pub enum Compress {
    Stub,
    StubV2,
    Algorithm(String),
}

#[derive(Debug, Clone)]
pub enum AllowCompress {
    Yes,
    No,
    Asym,
    Other(String),
}

#[derive(Debug, Clone)]
pub struct Route {
    pub network: Ipv4Addr,
    pub netmask: Option<Ipv4Addr>,
    pub gateway: Option<Ipv4Addr>,
}

#[derive(Debug, Clone)]
pub struct RedirectGateway {
    pub def1: bool,
    pub bypass_dhcp: bool,
    pub bypass_dns: bool,
    pub local: bool,
    pub ipv6: bool,
}

enum OvpnItem {
    Directive {
        key: String,
        args: Vec<String>,
        line: usize,
    },
    Block {
        key: String,
        content: String,
        line: usize,
    },
}

#[derive(Default)]
struct OvpnFileBuilder {
    remotes: Vec<Remote>,
    dev: Option<String>,
    proto: Option<String>,
    ca: Option<CertSource>,
    cert: Option<CertSource>,
    key: Option<CertSource>,
    key_direction: Option<u8>,
    tls_auth: Option<TlsAuth>,
    tls_crypt: Option<CertSource>,
    cipher: Option<String>,
    data_ciphers: Vec<String>,
    auth: Option<String>,
    compress: Option<Compress>,
    allow_compress: Option<AllowCompress>,
    routes: Vec<Route>,
    redirect_gateway: Option<RedirectGateway>,
    auth_user_pass: bool,
    flags: Vec<String>,
    options: HashMap<String, Vec<String>>,
}

impl OvpnFileBuilder {
    fn build(mut self) -> OvpnFile {
        if let Some(ref mut ta) = self.tls_auth
            && ta.key_direction.is_none()
        {
            ta.key_direction = self.key_direction;
        }

        OvpnFile {
            remotes: self.remotes,
            dev: self.dev,
            proto: self.proto,
            ca: self.ca,
            cert: self.cert,
            key: self.key,
            tls_auth: self.tls_auth,
            tls_crypt: self.tls_crypt,
            cipher: self.cipher,
            data_ciphers: self.data_ciphers,
            auth: self.auth,
            compress: self.compress,
            allow_compress: self.allow_compress,
            routes: self.routes,
            redirect_gateway: self.redirect_gateway,
            auth_user_pass: self.auth_user_pass,
            flags: self.flags,
            options: self.options,
        }
    }
}

fn lexer(input: &str) -> Result<Vec<OvpnItem>, OvpnParseError> {
    let mut items = Vec::new();

    let mut current_line = String::new();
    let mut continuing = false;

    let mut in_block: Option<String> = None;
    let mut block_buffer = String::new();
    let mut block_line_start = 0;

    for (idx, raw_line) in input.lines().enumerate() {
        let line_number = idx + 1;
        let line = raw_line;

        // We're in a block
        if let Some(block_name) = &in_block {
            let trimmed = line.trim();

            if trimmed.starts_with("</") && trimmed.ends_with(">") {
                let end_tag = trimmed[2..trimmed.len() - 1].trim().to_lowercase();

                if end_tag == *block_name {
                    items.push(OvpnItem::Block {
                        key: block_name.clone(),
                        content: block_buffer.clone(),
                        line: block_line_start,
                    });

                    in_block = None;
                    block_buffer.clear();
                    continue;
                } else {
                    return Err(OvpnParseError::UnexpectedBlockEnd {
                        block: end_tag,
                        line: line_number,
                    });
                }
            }

            block_buffer.push_str(line);
            block_buffer.push('\n');

            continue;
        }

        // Typically, one might track line numbers where the directive
        // starts, as opposed to when it ends
        // e.g.
        //
        // remote example.com \
        // 1194 udp
        //
        // For the sake of reporting errors in a user friendly fashion,
        // I find it okay to do the latter here.
        if continuing {
            current_line.push(' ');
            current_line.push_str(line.trim_start());
        } else {
            current_line.clear();
            current_line.push_str(line);
        }

        if current_line.ends_with('\\') {
            continuing = true;
            current_line.pop();
            continue;
        } else {
            continuing = false;
        }

        let line = current_line.trim();

        // Remove comments
        let mut cleaned = String::new();
        let mut prev_whitespace = true;

        for c in line.chars() {
            if (c == '#' || c == ';') && prev_whitespace {
                break;
            }

            prev_whitespace = c.is_whitespace();
            cleaned.push(c);
        }

        current_line.clear();
        let line = cleaned.trim();

        if line.is_empty() {
            continue;
        }

        if line.starts_with('<') && line.ends_with('>') && !line.starts_with("</") {
            let key = line[1..line.len() - 1].trim().to_lowercase();

            if key.is_empty() {
                return Err(OvpnParseError::InvalidDirectiveSyntax { line: line_number });
            }

            in_block = Some(key);
            block_line_start = line_number;
            block_buffer.clear();
            continue;
        }

        if line.starts_with("</") && line.ends_with('>') {
            let key = line[2..line.len() - 1].trim().to_lowercase();

            return Err(OvpnParseError::UnexpectedBlockEnd {
                block: key,
                line: line_number,
            });
        }

        let mut parts = line.split_whitespace();
        let key = match parts.next() {
            Some(k) => k.to_lowercase(),
            None => {
                return Err(OvpnParseError::InvalidDirectiveSyntax { line: line_number });
            }
        };

        let args: Vec<String> = parts.map(|s| s.to_string()).collect();

        items.push(OvpnItem::Directive {
            key,
            args,
            line: line_number,
        });
    }

    if continuing {
        return Err(OvpnParseError::InvalidContinuation {
            line: input.lines().count(),
        });
    }

    if let Some(block) = in_block {
        return Err(OvpnParseError::UnterminatedBlock {
            block,
            line: block_line_start,
        });
    }

    Ok(items)
}

pub fn parse_ovpn(content: &str) -> Result<OvpnFile, ConnectionError> {
    let mut b = OvpnFileBuilder::default();
    let items = lexer(content)?;

    for item in items {
        match item {
            OvpnItem::Directive { key, args, line } => {
                match key.as_str() {
                    "remote" => {
                        // remote <HOST> [PORT] [PROTO]

                        if args.len() > 3 {
                            return Err(OvpnParseError::InvalidArgument {
                                key,
                                arg: format!("{args:?}"),
                                line,
                            }
                            .into());
                        }

                        let host = args
                            .first()
                            .ok_or(OvpnParseError::MissingArgument {
                                key: key.clone(),
                                line,
                            })?
                            .clone();

                        let port = args
                            .get(1)
                            .map(|p| {
                                p.parse::<u16>().map_err(|_| OvpnParseError::InvalidNumber {
                                    key: key.clone(),
                                    value: p.clone(),
                                    line,
                                })
                            })
                            .transpose()?;

                        let proto = args.get(2).cloned();

                        b.remotes.push(Remote { host, port, proto });
                    }
                    "dev" => {
                        // dev <DEVICE>

                        if args.len() != 1 {
                            Err(OvpnParseError::InvalidArgument {
                                key: key.clone(),
                                arg: format!("{args:?}"),
                                line,
                            })?;
                        }

                        let value = args
                            .first()
                            .ok_or(OvpnParseError::MissingArgument { key, line })?;

                        b.dev = Some(value.clone());
                    }
                    "proto" => {
                        // proto <PROTOCOL>

                        if args.len() != 1 {
                            Err(OvpnParseError::InvalidArgument {
                                key: key.clone(),
                                arg: format!("{args:?}"),
                                line,
                            })?;
                        }

                        let value = args
                            .first()
                            .ok_or(OvpnParseError::MissingArgument { key, line })?;

                        b.proto = Some(value.clone());
                    }
                    "ca" => {
                        if args.len() > 1 {
                            return Err(OvpnParseError::InvalidArgument {
                                key,
                                arg: format!("{args:?}"),
                                line,
                            }
                            .into());
                        }
                        let path = args
                            .first()
                            .ok_or(OvpnParseError::MissingArgument {
                                key: key.clone(),
                                line,
                            })?
                            .clone();
                        b.ca = Some(CertSource::File(path));
                    }
                    "cert" => {
                        if args.len() > 1 {
                            return Err(OvpnParseError::InvalidArgument {
                                key,
                                arg: format!("{args:?}"),
                                line,
                            }
                            .into());
                        }
                        let path = args
                            .first()
                            .ok_or(OvpnParseError::MissingArgument {
                                key: key.clone(),
                                line,
                            })?
                            .clone();
                        b.cert = Some(CertSource::File(path));
                    }
                    "key" => {
                        if args.len() > 1 {
                            return Err(OvpnParseError::InvalidArgument {
                                key,
                                arg: format!("{args:?}"),
                                line,
                            }
                            .into());
                        }
                        let path = args
                            .first()
                            .ok_or(OvpnParseError::MissingArgument {
                                key: key.clone(),
                                line,
                            })?
                            .clone();
                        b.key = Some(CertSource::File(path));
                    }
                    "tls-crypt" => {
                        if args.len() > 1 {
                            return Err(OvpnParseError::InvalidArgument {
                                key,
                                arg: format!("{args:?}"),
                                line,
                            }
                            .into());
                        }
                        let path = args
                            .first()
                            .ok_or(OvpnParseError::MissingArgument {
                                key: key.clone(),
                                line,
                            })?
                            .clone();
                        b.tls_crypt = Some(CertSource::File(path));
                    }
                    "tls-auth" => {
                        // tls-auth <KEY-FILE> [DIRECTION]

                        if args.len() > 2 {
                            return Err(OvpnParseError::InvalidArgument {
                                key,
                                arg: format!("{args:?}"),
                                line,
                            }
                            .into());
                        }

                        let path = args
                            .first()
                            .ok_or(OvpnParseError::MissingArgument {
                                key: key.clone(),
                                line,
                            })?
                            .clone();

                        let kd = if let Some(value) = args.get(1) {
                            let direction =
                                value
                                    .parse::<u8>()
                                    .map_err(|_| OvpnParseError::InvalidNumber {
                                        key: key.clone(),
                                        value: value.clone(),
                                        line,
                                    })?;
                            if direction > 1 {
                                return Err(OvpnParseError::InvalidArgument {
                                    key,
                                    arg: value.clone(),
                                    line,
                                }
                                .into());
                            }
                            Some(direction)
                        } else {
                            None
                        };

                        b.tls_auth = Some(TlsAuth {
                            source: CertSource::File(path),
                            key_direction: kd,
                        });
                    }
                    "key-direction" => {
                        // key-direction <0/1>

                        let value = args.first().ok_or(OvpnParseError::MissingArgument {
                            key: key.clone(),
                            line,
                        })?;

                        let dir =
                            value
                                .parse::<u8>()
                                .map_err(|_| OvpnParseError::InvalidNumber {
                                    key: key.clone(),
                                    value: value.clone(),
                                    line,
                                })?;

                        // 0 = server, 1 = client
                        if dir > 1 {
                            Err(OvpnParseError::InvalidArgument {
                                key,
                                arg: value.clone(),
                                line,
                            })?;
                        }

                        b.key_direction = Some(dir);
                    }
                    "cipher" => {
                        // cipher <CIPHER>
                        // Note: This is deprecated in new configs

                        let value = args
                            .first()
                            .ok_or(OvpnParseError::MissingArgument { key, line })?;

                        b.cipher = Some(value.clone());
                    }
                    "data-ciphers" => {
                        // data-ciphers <[cipher1]:[cipher2]...>

                        let ciphers = args
                            .first()
                            .ok_or(OvpnParseError::MissingArgument { key, line })?;

                        b.data_ciphers.extend(ciphers.split(':').map(String::from));
                    }
                    "auth" => {
                        // auth <ALGORITHM>

                        let value = args
                            .first()
                            .ok_or(OvpnParseError::MissingArgument { key, line })?;

                        b.auth = Some(value.clone());
                    }
                    "compress" => {
                        // compress [ALGORITHM]

                        b.compress = Some(match args.first().map(|s| s.as_str()) {
                            None | Some("stub") => Compress::Stub,
                            Some("stub-v2") => Compress::StubV2,
                            Some(alg) => Compress::Algorithm(alg.to_string()),
                        });
                    }
                    "allow-compress" => {
                        // allow-compress asym (default) <- receive compressed data but don't send
                        // allow-compress [yes/no]

                        let value = args
                            .first()
                            .ok_or(OvpnParseError::MissingArgument { key, line })?;

                        let parsed = match value.as_str() {
                            "yes" => AllowCompress::Yes,
                            "no" => AllowCompress::No,
                            "asym" => AllowCompress::Asym,
                            other => AllowCompress::Other(other.to_string()),
                        };

                        b.allow_compress = Some(parsed);
                    }
                    "route" => {
                        // route <NETWORK> [NETMASK] [GATEWAY]

                        let network = parse_ipv4_arg(&key, args.first(), line)?;
                        let netmask = args
                            .get(1)
                            .map(|v| parse_ipv4_arg(&key, Some(v), line))
                            .transpose()?;
                        let gateway = args
                            .get(2)
                            .map(|v| parse_ipv4_arg(&key, Some(v), line))
                            .transpose()?;

                        b.routes.push(Route {
                            network,
                            netmask,
                            gateway,
                        });
                    }
                    "auth-user-pass" => {
                        // auth-user-pass [FILE]
                        // Optional file path is ignored — NM handles interactive prompts.
                        b.auth_user_pass = true;
                    }
                    "redirect-gateway" => {
                        let mut rg = RedirectGateway {
                            def1: false,
                            bypass_dhcp: false,
                            bypass_dns: false,
                            local: false,
                            ipv6: false,
                        };

                        for arg in args {
                            match arg.as_str() {
                                "def1" => rg.def1 = true,
                                "bypass-dhcp" => rg.bypass_dhcp = true,
                                "bypass-dns" => rg.bypass_dns = true,
                                "local" => rg.local = true,
                                "ipv6" => rg.ipv6 = true,
                                _ => {}
                            }
                        }

                        b.redirect_gateway = Some(rg);
                    }
                    _ => {
                        if args.is_empty() {
                            b.flags.push(key);
                        } else {
                            b.options.entry(key).or_default().extend(args);
                        }
                    }
                }
            }
            OvpnItem::Block {
                key: block_key,
                content,
                line: _line,
            } => match block_key.as_str() {
                "ca" => {
                    b.ca = Some(CertSource::Inline(content));
                }

                "cert" => {
                    b.cert = Some(CertSource::Inline(content));
                }

                "key" => {
                    b.key = Some(CertSource::Inline(content));
                }

                "tls-auth" => {
                    b.tls_auth = Some(TlsAuth {
                        source: CertSource::Inline(content),
                        key_direction: None,
                    });
                }

                "tls-crypt" => {
                    b.tls_crypt = Some(CertSource::Inline(content));
                }

                _ => {
                    b.options.entry(block_key).or_default().push(content);
                }
            },
        }
    }

    Ok(b.build())
}

fn parse_ipv4_arg(
    key: &str,
    value: Option<&String>,
    line: usize,
) -> Result<Ipv4Addr, OvpnParseError> {
    let v = value.ok_or(OvpnParseError::MissingArgument {
        key: key.to_string(),
        line,
    })?;

    v.parse::<Ipv4Addr>()
        .map_err(|_| OvpnParseError::InvalidNumber {
            key: key.to_string(),
            value: v.clone(),
            line,
        })
}

#[cfg(test)]
mod tests {
    use std::net::Ipv4Addr;

    use super::*;

    // Macro to reduce noise on failure assertions
    macro_rules! assert_parse_err {
        ($input:expr, $pattern:pat $(if $guard:expr)? ) => {
            match parse_ovpn($input).unwrap_err() {
                ConnectionError::ParseError(e) => {
                    assert!(matches!(e, $pattern $(if $guard)?));
                }
                _ => panic!("expected OvpnParseError"),
            }
        };
    }

    fn parse_ok(input: &str) -> OvpnFile {
        parse_ovpn(input).unwrap()
    }

    fn assert_one_remote(f: &OvpnFile, host: &str, port: Option<u16>, proto: Option<&str>) {
        assert_eq!(f.remotes.len(), 1, "expected exactly one remote");
        let r = &f.remotes[0];
        assert_eq!(r.host, host);
        assert_eq!(r.port, port);
        assert_eq!(r.proto.as_deref(), proto);
    }

    fn assert_inline_cert(source: &CertSource, expected_substr: &str) {
        match source {
            CertSource::Inline(s) => assert!(
                s.contains(expected_substr),
                "inline cert should contain {expected_substr:?}, got {s:?}"
            ),
            other => panic!("expected CertSource::Inline, got {other:?}"),
        }
    }

    #[test]
    fn parse_remote_directive() {
        let result = parse_ok("remote example.com 1194 udp");
        assert_one_remote(&result, "example.com", Some(1194), Some("udp"));
    }

    #[test]
    fn remote_missing_host_fails() {
        assert_parse_err!(
            "remote",
            OvpnParseError::MissingArgument { key, .. } if key == "remote"
        );
    }

    #[test]
    fn remote_host_only_passes() {
        let result = parse_ok("remote example.com");
        assert_one_remote(&result, "example.com", None, None);
    }

    #[test]
    fn remote_invalid_port_fails() {
        assert_parse_err!(
            "remote example.com bogus",
            OvpnParseError::InvalidNumber { key, value, .. }
                if key == "remote" && value == "bogus"
        );
    }

    #[test]
    fn remote_multiple_in_order_passes() {
        let result = parse_ok("remote a.example 1194 udp\nremote b.example 443 tcp-client");
        assert_eq!(result.remotes.len(), 2);
        assert_eq!(result.remotes[0].host, "a.example");
        assert_eq!(result.remotes[0].port, Some(1194));
        assert_eq!(result.remotes[0].proto.as_deref(), Some("udp"));
        assert_eq!(result.remotes[1].host, "b.example");
        assert_eq!(result.remotes[1].port, Some(443));
        assert_eq!(result.remotes[1].proto.as_deref(), Some("tcp-client"));
    }

    #[test]
    fn parse_dev_directive() {
        let result = parse_ok("dev tun");
        assert_eq!(result.dev.as_deref(), Some("tun"));
    }

    #[test]
    fn dev_arity_check_fails() {
        assert_parse_err!(
            "dev tun panu",
            OvpnParseError::InvalidArgument { key, .. } if key == "dev"
        );
    }

    #[test]
    fn dev_missing_device_fails() {
        assert_parse_err!(
            "dev",
            OvpnParseError::InvalidArgument { key, .. } if key == "dev"
        );
    }

    #[test]
    fn parse_proto_directive() {
        let result = parse_ok("proto udp");
        assert_eq!(result.proto.as_deref(), Some("udp"));
    }

    #[test]
    fn proto_arity_check_fails() {
        assert_parse_err!(
            "proto udp tcp",
            OvpnParseError::InvalidArgument { key, .. } if key == "proto"
        );
    }

    #[test]
    fn proto_missing_arg_fails() {
        assert_parse_err!(
            "proto",
            OvpnParseError::InvalidArgument { key, .. } if key == "proto"
        );
    }

    #[test]
    fn dev_strips_comments_passes() {
        let result = parse_ok("  dev tun  # interface\n; ignored");
        assert_eq!(result.dev.as_deref(), Some("tun"));
    }

    #[test]
    fn remote_line_continuation_passes() {
        let result = parse_ok("remote example.com \\\n1194 udp");
        assert_one_remote(&result, "example.com", Some(1194), Some("udp"));
    }

    #[test]
    fn invalid_line_continuation_fails() {
        assert_parse_err!("dev tun\\", OvpnParseError::InvalidContinuation { .. });
    }

    #[test]
    fn block_unterminated_fails() {
        assert_parse_err!(
            "<ca>\n-----BEGIN CERTIFICATE-----",
            OvpnParseError::UnterminatedBlock { block, .. } if block == "ca"
        );
    }

    #[test]
    fn block_close_without_open_fails() {
        assert_parse_err!("</ca>", OvpnParseError::UnexpectedBlockEnd { .. });
    }

    #[test]
    fn block_mismatched_end_tag_fails() {
        assert_parse_err!(
            "<ca>\n</cert>",
            OvpnParseError::UnexpectedBlockEnd { block, .. } if block == "cert"
        );
    }

    #[test]
    fn parse_cipher_directive() {
        let result = parse_ok("cipher AES-256-GCM");
        assert_eq!(result.cipher.as_deref(), Some("AES-256-GCM"));
    }

    #[test]
    fn parse_data_ciphers_directive() {
        let result = parse_ok("data-ciphers AES-128-GCM:CHACHA20-POLY1305");
        assert_eq!(
            result.data_ciphers,
            vec!["AES-128-GCM", "CHACHA20-POLY1305"]
        );
    }

    #[test]
    fn parse_auth_directive() {
        let result = parse_ok("auth SHA256");
        assert_eq!(result.auth.as_deref(), Some("SHA256"));
    }

    #[test]
    fn cipher_missing_value_fails() {
        assert_parse_err!(
            "cipher",
            OvpnParseError::MissingArgument { key, .. } if key == "cipher"
        );
    }

    #[test]
    fn data_ciphers_missing_value_fails() {
        assert_parse_err!(
            "data-ciphers",
            OvpnParseError::MissingArgument { key, .. } if key == "data-ciphers"
        );
    }

    #[test]
    fn data_ciphers_repeat_directives_passes() {
        let result =
            parse_ok("data-ciphers AES-256-GCM:AES-128-GCM\ndata-ciphers CHACHA20-POLY1305");
        assert_eq!(
            result.data_ciphers,
            vec!["AES-256-GCM", "AES-128-GCM", "CHACHA20-POLY1305"]
        );
    }

    #[test]
    fn compress_directive_variants_passes() {
        assert!(matches!(
            parse_ok("compress").compress,
            Some(Compress::Stub)
        ));
        assert!(matches!(
            parse_ok("compress stub").compress,
            Some(Compress::Stub)
        ));
        assert!(matches!(
            parse_ok("compress stub-v2").compress,
            Some(Compress::StubV2)
        ));
        assert!(matches!(
            parse_ok("compress lz4").compress,
            Some(Compress::Algorithm(s)) if s == "lz4"
        ));
    }

    #[test]
    fn allow_compress_directive_variants_passes() {
        assert!(matches!(
            parse_ok("allow-compress yes").allow_compress,
            Some(AllowCompress::Yes)
        ));
        assert!(matches!(
            parse_ok("allow-compress no").allow_compress,
            Some(AllowCompress::No)
        ));
        assert!(matches!(
            parse_ok("allow-compress asym").allow_compress,
            Some(AllowCompress::Asym)
        ));
        assert!(matches!(
            parse_ok("allow-compress legacy").allow_compress,
            Some(AllowCompress::Other(s)) if s == "legacy"
        ));
    }

    #[test]
    fn allow_compress_missing_arg_fails() {
        assert_parse_err!(
            "allow-compress",
            OvpnParseError::MissingArgument { key, .. } if key == "allow-compress"
        );
    }

    #[test]
    fn parse_route_directive() {
        let result = parse_ok("route 10.0.0.0 255.255.255.0 192.168.1.1");
        assert_eq!(result.routes.len(), 1);
        assert_eq!(result.routes[0].network, Ipv4Addr::new(10, 0, 0, 0));
        assert_eq!(
            result.routes[0].netmask,
            Some(Ipv4Addr::new(255, 255, 255, 0))
        );
        assert_eq!(
            result.routes[0].gateway,
            Some(Ipv4Addr::new(192, 168, 1, 1))
        );
    }

    #[test]
    fn route_network_only_passes() {
        let result = parse_ok("route 172.16.0.0");
        assert_eq!(result.routes.len(), 1);
        assert_eq!(result.routes[0].network, Ipv4Addr::new(172, 16, 0, 0));
        assert_eq!(result.routes[0].netmask, None);
        assert_eq!(result.routes[0].gateway, None);
    }

    #[test]
    fn route_missing_network_fails() {
        assert_parse_err!(
            "route",
            OvpnParseError::MissingArgument { key, .. } if key == "route"
        );
    }

    #[test]
    fn route_invalid_ipv4_fails() {
        assert_parse_err!(
            "route not-an-ip",
            OvpnParseError::InvalidNumber { key, value, .. }
                if key == "route" && value == "not-an-ip"
        );
    }

    #[test]
    fn route_invalid_optional_addresses_identify_the_bad_value() {
        assert_parse_err!(
            "route 10.0.0.0 bad-netmask",
            OvpnParseError::InvalidNumber { key, value, line }
                if key == "route" && value == "bad-netmask" && line == 1
        );
        assert_parse_err!(
            "route 10.0.0.0 255.255.255.0 bad-gateway",
            OvpnParseError::InvalidNumber { key, value, line }
                if key == "route" && value == "bad-gateway" && line == 1
        );
    }

    #[test]
    fn parse_redirect_gateway_directive() {
        let result = parse_ok("redirect-gateway def1 bypass-dhcp bypass-dns local ipv6");
        let rg = result.redirect_gateway.expect("redirect-gateway");
        assert!(rg.def1 && rg.bypass_dhcp && rg.bypass_dns && rg.local && rg.ipv6);
    }

    #[test]
    fn redirect_gateway_unknown_flags_passes() {
        let result = parse_ok("redirect-gateway def1 nosuch");
        let rg = result.redirect_gateway.expect("redirect-gateway");
        assert!(rg.def1);
        assert!(!rg.bypass_dhcp);
    }

    #[test]
    fn flags_and_options_directives_passes() {
        let result = parse_ok("client\nnobind\n tls-version-min 1.2 \n");
        assert!(result.flags.contains(&"client".to_string()));
        assert!(result.flags.contains(&"nobind".to_string()));
        let expected = vec!["1.2".to_string()];
        assert_eq!(result.options.get("tls-version-min"), Some(&expected));
    }

    #[test]
    fn ca_inline_block_passes() {
        let result = parse_ok("<ca>\nTESTCABODY\n</ca>");
        assert_inline_cert(result.ca.as_ref().expect("ca"), "TESTCABODY");
    }

    #[test]
    fn cert_and_key_inline_blocks_passes() {
        let result = parse_ok("<cert>\nCERTPEM\n</cert>\n<key>\nKEYPEM\n</key>");
        assert_inline_cert(result.cert.as_ref().expect("cert"), "CERTPEM");
        assert_inline_cert(result.key.as_ref().expect("key"), "KEYPEM");
    }

    #[test]
    fn tls_auth_and_tls_crypt_inline_passes() {
        let result =
            parse_ok("<tls-auth>\nAUTHKEY\n</tls-auth>\n<tls-crypt>\nCRYPTKEY\n</tls-crypt>");
        let ta = result.tls_auth.as_ref().expect("tls-auth");
        assert_inline_cert(&ta.source, "AUTHKEY");
        assert_eq!(ta.key_direction, None);
        assert_inline_cert(result.tls_crypt.as_ref().expect("tls-crypt"), "CRYPTKEY");
    }

    #[test]
    fn unknown_inline_block_in_options_passes() {
        let result = parse_ok("<foo>\nbar\n</foo>");
        let v = result.options.get("foo").expect("foo block");
        assert_eq!(v.len(), 1);
        assert!(v[0].contains("bar"));
    }

    #[test]
    fn error_reports_correct_line_number() {
        let input = "dev tun\nproto udp\nremote\ncipher AES-256-GCM";
        match parse_ovpn(input).unwrap_err() {
            ConnectionError::ParseError(OvpnParseError::MissingArgument { key, line }) => {
                assert_eq!(key, "remote");
                assert_eq!(line, 3);
            }
            other => panic!("expected MissingArgument, got {other:?}"),
        }
    }

    #[test]
    fn unterminated_block_reports_start_line() {
        let input = "dev tun\n<ca>\ncontent";
        match parse_ovpn(input).unwrap_err() {
            ConnectionError::ParseError(OvpnParseError::UnterminatedBlock { block, line }) => {
                assert_eq!(block, "ca");
                assert_eq!(line, 2);
            }
            other => panic!("expected UnterminatedBlock, got {other:?}"),
        }
    }

    #[test]
    fn tls_auth_directive_with_direction_passes() {
        let result = parse_ok("tls-auth /etc/openvpn/ta.key 1");
        let ta = result.tls_auth.as_ref().expect("tls-auth");
        match &ta.source {
            CertSource::File(p) => assert_eq!(p, "/etc/openvpn/ta.key"),
            other => panic!("expected CertSource::File, got {other:?}"),
        }
        assert_eq!(ta.key_direction, Some(1));
    }

    #[test]
    fn tls_auth_directive_without_direction_passes() {
        let result = parse_ok("tls-auth /etc/openvpn/ta.key");
        let ta = result.tls_auth.as_ref().expect("tls-auth");
        match &ta.source {
            CertSource::File(p) => assert_eq!(p, "/etc/openvpn/ta.key"),
            other => panic!("expected CertSource::File, got {other:?}"),
        }
        assert_eq!(ta.key_direction, None);
    }

    #[test]
    fn tls_auth_directive_missing_path_fails() {
        assert_parse_err!(
            "tls-auth",
            OvpnParseError::MissingArgument { key, .. } if key == "tls-auth"
        );
    }

    #[test]
    fn file_certificate_directives_require_exactly_one_path() {
        for directive in ["ca", "cert", "key", "tls-crypt"] {
            let error = parse_ovpn(directive).unwrap_err();
            assert!(matches!(
                error,
                ConnectionError::ParseError(OvpnParseError::MissingArgument { key, line })
                    if key == directive && line == 1
            ));

            let input = format!("{directive} first.pem second.pem");
            let error = parse_ovpn(&input).unwrap_err();
            assert!(matches!(
                error,
                ConnectionError::ParseError(OvpnParseError::InvalidArgument { key, line, .. })
                    if key == directive && line == 1
            ));
        }
    }

    #[test]
    fn tls_auth_directive_rejects_invalid_direction() {
        assert_parse_err!(
            "tls-auth /etc/openvpn/ta.key 2",
            OvpnParseError::InvalidArgument { key, arg, line }
                if key == "tls-auth" && arg == "2" && line == 1
        );
        assert_parse_err!(
            "tls-auth /etc/openvpn/ta.key client",
            OvpnParseError::InvalidNumber { key, value, line }
                if key == "tls-auth" && value == "client" && line == 1
        );
    }

    #[test]
    fn tls_auth_and_remote_reject_extra_arguments() {
        assert_parse_err!(
            "tls-auth ta.key 1 extra",
            OvpnParseError::InvalidArgument { key, .. } if key == "tls-auth"
        );
        assert_parse_err!(
            "remote vpn.example.com 1194 udp extra",
            OvpnParseError::InvalidArgument { key, .. } if key == "remote"
        );
    }

    #[test]
    fn key_direction_standalone_passes() {
        let result = parse_ok("<tls-auth>\nAUTHKEY\n</tls-auth>\nkey-direction 0");
        let ta = result.tls_auth.as_ref().expect("tls-auth");
        assert_inline_cert(&ta.source, "AUTHKEY");
        assert_eq!(ta.key_direction, Some(0));
    }

    #[test]
    fn key_direction_standalone_before_block_passes() {
        let result = parse_ok("key-direction 1\n<tls-auth>\nAUTHKEY\n</tls-auth>");
        let ta = result.tls_auth.as_ref().expect("tls-auth");
        assert_inline_cert(&ta.source, "AUTHKEY");
        assert_eq!(ta.key_direction, Some(1));
    }

    #[test]
    fn key_direction_does_not_override_inline_arg() {
        let result = parse_ok("tls-auth /path/ta.key 1\nkey-direction 0");
        let ta = result.tls_auth.as_ref().expect("tls-auth");
        assert_eq!(ta.key_direction, Some(1));
    }

    #[test]
    fn key_direction_invalid_value_fails() {
        assert_parse_err!(
            "key-direction 2",
            OvpnParseError::InvalidArgument { key, arg, .. }
                if key == "key-direction" && arg == "2"
        );
    }

    #[test]
    fn key_direction_non_numeric_fails() {
        assert_parse_err!(
            "key-direction server",
            OvpnParseError::InvalidNumber { key, value, .. }
                if key == "key-direction" && value == "server"
        );
    }

    #[test]
    fn key_direction_missing_arg_fails() {
        assert_parse_err!(
            "key-direction",
            OvpnParseError::MissingArgument { key, .. } if key == "key-direction"
        );
    }

    #[test]
    fn auth_user_pass_bare_passes() {
        let result = parse_ok("auth-user-pass");
        assert!(result.auth_user_pass);
    }

    #[test]
    fn auth_user_pass_with_file_path_passes() {
        let result = parse_ok("auth-user-pass /etc/openvpn/creds.txt");
        assert!(result.auth_user_pass);
    }

    #[test]
    fn auth_user_pass_absent_defaults_false() {
        let result = parse_ok("remote example.com 1194 udp");
        assert!(!result.auth_user_pass);
    }
}
