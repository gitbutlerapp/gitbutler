//! HTTP clients using platform trust and additional locally configured roots.
#![deny(missing_docs)]

/// Create a client builder that merges PEM roots from `SSL_CERT_FILE` and
/// `SSL_CERT_DIR` with the platform's certificate verifier.
///
/// `SSL_CERT_DIR` accepts paths separated by `:` on Unix or `;` on Windows.
/// Unreadable certificates are logged; valid certificates are still used.
/// Certificate validity and hostname verification remain enabled.
/// Returns an error if a loaded certificate cannot be converted for reqwest.
pub fn client_builder() -> reqwest::Result<reqwest::ClientBuilder> {
    let builder = reqwest::Client::builder();
    let has_file = std::env::var_os("SSL_CERT_FILE").is_some();
    let has_dirs = std::env::var_os("SSL_CERT_DIR")
        .is_some_and(|dirs| std::env::split_paths(&dirs).any(|path| !path.as_os_str().is_empty()));
    // Loading OS roots again could override their platform trust restrictions.
    if !has_file && !has_dirs {
        return Ok(builder);
    }

    let roots = rustls_native_certs::load_native_certs();
    for error in roots.errors {
        tracing::warn!(%error, "Could not load a local TLS root certificate");
    }
    let certs = roots
        .certs
        .iter()
        .filter_map(
            |cert| match rustls::RootCertStore::empty().add(cert.clone()) {
                Ok(()) => Some(reqwest::Certificate::from_der(cert)),
                Err(error) => {
                    tracing::warn!(%error, "Could not parse a local TLS root certificate");
                    None
                }
            },
        )
        .collect::<reqwest::Result<Vec<_>>>()?;
    Ok(builder.tls_certs_merge(certs))
}
