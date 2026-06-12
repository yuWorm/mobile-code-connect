use std::{
    fmt, io,
    net::SocketAddr,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
};

use quinn::crypto::rustls::QuicClientConfig;
use quinn::{default_runtime, EndpointConfig};
use quinn::{ClientConfig, Endpoint, RecvStream, SendStream, ServerConfig};
use rustls::{
    client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier},
    pki_types::{CertificateDer, PrivatePkcs8KeyDer, ServerName, UnixTime},
};
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};

pub struct ServerEndpoint {
    endpoint: Endpoint,
    certificate_der: CertificateDer<'static>,
}

pub struct P2pQuicIdentity {
    certificate_der: CertificateDer<'static>,
    private_key: PrivatePkcs8KeyDer<'static>,
}

impl Clone for P2pQuicIdentity {
    fn clone(&self) -> Self {
        Self {
            certificate_der: self.certificate_der.clone(),
            private_key: self.private_key.clone_key(),
        }
    }
}

impl fmt::Debug for P2pQuicIdentity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("P2pQuicIdentity")
            .field("certificate_der_len", &self.certificate_der.len())
            .finish_non_exhaustive()
    }
}

impl P2pQuicIdentity {
    pub fn from_der_parts(certificate_der: Vec<u8>, private_key_der: Vec<u8>) -> Self {
        Self {
            certificate_der: CertificateDer::from(certificate_der),
            private_key: PrivatePkcs8KeyDer::from(private_key_der),
        }
    }

    pub fn certificate_der(&self) -> &CertificateDer<'static> {
        &self.certificate_der
    }

    pub fn private_key_der(&self) -> &[u8] {
        self.private_key.secret_pkcs8_der()
    }

    fn private_key(&self) -> PrivatePkcs8KeyDer<'static> {
        self.private_key.clone_key()
    }
}

impl ServerEndpoint {
    pub fn endpoint(&self) -> &Endpoint {
        &self.endpoint
    }

    pub fn into_endpoint(self) -> Endpoint {
        self.endpoint
    }

    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.endpoint.local_addr()
    }

    pub fn certificate_der(&self) -> &CertificateDer<'static> {
        &self.certificate_der
    }
}

pub async fn make_server_endpoint(bind_addr: SocketAddr) -> Result<ServerEndpoint, QuicError> {
    let socket = std::net::UdpSocket::bind(bind_addr)?;
    make_server_endpoint_from_std_socket(socket).await
}

pub async fn make_server_endpoint_from_std_socket(
    socket: std::net::UdpSocket,
) -> Result<ServerEndpoint, QuicError> {
    make_server_endpoint_from_std_socket_with_identity(
        socket,
        generate_self_signed_server_identity()?,
    )
    .await
}

pub fn generate_self_signed_server_identity() -> Result<P2pQuicIdentity, QuicError> {
    let cert = rcgen::generate_simple_self_signed(vec!["localhost".to_string()])?;
    Ok(P2pQuicIdentity {
        certificate_der: CertificateDer::from(cert.cert),
        private_key: PrivatePkcs8KeyDer::from(cert.key_pair.serialize_der()),
    })
}

pub async fn make_server_endpoint_from_std_socket_with_identity(
    socket: std::net::UdpSocket,
    identity: P2pQuicIdentity,
) -> Result<ServerEndpoint, QuicError> {
    let mut server_config = ServerConfig::with_single_cert(
        vec![identity.certificate_der.clone()],
        identity.private_key().into(),
    )?;
    let transport_config =
        Arc::get_mut(&mut server_config.transport).ok_or(QuicError::SharedTransportConfig)?;
    transport_config.max_concurrent_uni_streams(0_u8.into());

    Ok(ServerEndpoint {
        endpoint: Endpoint::new(
            EndpointConfig::default(),
            Some(server_config),
            socket,
            default_runtime().ok_or(QuicError::NoAsyncRuntime)?,
        )?,
        certificate_der: identity.certificate_der,
    })
}

pub async fn make_client_endpoint(
    bind_addr: SocketAddr,
    server_certs: &[CertificateDer<'static>],
) -> Result<Endpoint, QuicError> {
    let socket = std::net::UdpSocket::bind(bind_addr)?;
    make_client_endpoint_from_std_socket(socket, server_certs).await
}

pub async fn make_client_endpoint_from_std_socket(
    socket: std::net::UdpSocket,
    server_certs: &[CertificateDer<'static>],
) -> Result<Endpoint, QuicError> {
    let mut roots = rustls::RootCertStore::empty();
    for cert in server_certs {
        roots.add(cert.clone())?;
    }

    let client_config = ClientConfig::with_root_certificates(Arc::new(roots))?;
    let runtime = default_runtime().ok_or(QuicError::NoAsyncRuntime)?;
    let mut endpoint = Endpoint::new(EndpointConfig::default(), None, socket, runtime)?;
    endpoint.set_default_client_config(client_config);
    Ok(endpoint)
}

pub async fn make_insecure_client_endpoint_from_std_socket(
    socket: std::net::UdpSocket,
) -> Result<Endpoint, QuicError> {
    let rustls_config = rustls::ClientConfig::builder_with_provider(Arc::new(
        rustls::crypto::ring::default_provider(),
    ))
    .with_protocol_versions(&[&rustls::version::TLS13])?
    .dangerous()
    .with_custom_certificate_verifier(SkipServerVerification::new())
    .with_no_client_auth();
    let client_config = ClientConfig::new(Arc::new(QuicClientConfig::try_from(rustls_config)?));
    let runtime = default_runtime().ok_or(QuicError::NoAsyncRuntime)?;
    let mut endpoint = Endpoint::new(EndpointConfig::default(), None, socket, runtime)?;
    endpoint.set_default_client_config(client_config);
    Ok(endpoint)
}

pub struct QuicBiStream {
    send: SendStream,
    recv: RecvStream,
}

impl QuicBiStream {
    pub fn new(send: SendStream, recv: RecvStream) -> Self {
        Self { send, recv }
    }

    pub fn into_inner(self) -> (SendStream, RecvStream) {
        (self.send, self.recv)
    }
}

impl AsyncRead for QuicBiStream {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        Pin::new(&mut self.recv).poll_read(cx, buf)
    }
}

impl AsyncWrite for QuicBiStream {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        AsyncWrite::poll_write(Pin::new(&mut self.send), cx, buf)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        AsyncWrite::poll_flush(Pin::new(&mut self.send), cx)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        AsyncWrite::poll_shutdown(Pin::new(&mut self.send), cx)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum QuicError {
    #[error("io failed: {0}")]
    Io(#[from] io::Error),
    #[error("certificate generation failed: {0}")]
    CertificateGeneration(#[from] rcgen::Error),
    #[error("tls failed: {0}")]
    Tls(#[from] rustls::Error),
    #[error("tls verifier failed: {0}")]
    TlsVerifier(#[from] rustls::client::VerifierBuilderError),
    #[error("server config transport is shared")]
    SharedTransportConfig,
    #[error("no async runtime found")]
    NoAsyncRuntime,
    #[error("quic tls config has no usable initial cipher suite")]
    NoInitialCipherSuite(#[from] quinn::crypto::rustls::NoInitialCipherSuite),
}

#[derive(Debug)]
struct SkipServerVerification(Arc<rustls::crypto::CryptoProvider>);

impl SkipServerVerification {
    fn new() -> Arc<Self> {
        Arc::new(Self(Arc::new(rustls::crypto::ring::default_provider())))
    }
}

impl ServerCertVerifier for SkipServerVerification {
    fn verify_server_cert(
        &self,
        _end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, rustls::Error> {
        Ok(ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &rustls::DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls12_signature(
            message,
            cert,
            dss,
            &self.0.signature_verification_algorithms,
        )
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &rustls::DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls13_signature(
            message,
            cert,
            dss,
            &self.0.signature_verification_algorithms,
        )
    }

    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        self.0.signature_verification_algorithms.supported_schemes()
    }
}
