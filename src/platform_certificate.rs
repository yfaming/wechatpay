//! 微信支付平台证书。

use crate::client::{BASE_URL, USER_AGENT};
use crate::credential::MchCredential;
use crate::util::datetime_fmt;
use anyhow::Result;
use base64::prelude::*;
use bytes::{BufMut, BytesMut};
use chrono::{DateTime, Local};
use reqwest::{Client, Response};
use rsa::pkcs1::DecodeRsaPublicKey;
use rsa::pkcs1v15::{Signature, VerifyingKey};
use rsa::sha2::Sha256;
use rsa::signature::Verifier;
use rsa::RsaPublicKey;
use serde::Deserialize;
use std::cmp::Reverse;
use x509_cert::der::DecodePem;
use x509_cert::Certificate;

/// 微信支付平台证书。
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct PlatformCertificate {
    pub serial_no: String,
    pub effective_time: DateTime<Local>,
    pub expire_time: DateTime<Local>,
    pub certificate: Certificate,
}

impl PlatformCertificate {
    pub fn public_key(&self) -> Result<RsaPublicKey> {
        let bytes = self
            .certificate
            .tbs_certificate
            .subject_public_key_info
            .subject_public_key
            .raw_bytes();

        RsaPublicKey::from_pkcs1_der(bytes)
            .map_err(|e| anyhow::format_err!("failed to get public key from ca, err: {}", e))
    }

    /// 对响应进行数字签名验证。
    pub(crate) async fn verify_response(&self, res: Response) -> Result<Response> {
        let public_key = self.public_key()?;
        let res = verify_response(&public_key, res).await?;
        Ok(res)
    }

    // TODO: 定义一个 RSA 加密方法，用于对敏感信息进行加密
}

/// 微信支付平台证书状态。
#[derive(Debug, Clone)]
pub struct PlatformCertificateState {
    /// 证书列表
    certificates: Vec<PlatformCertificate>,
    /// 最新的证书索引
    #[allow(unused)]
    newest_certificate_idx: usize,
}

impl PlatformCertificateState {
    pub fn new(certificates: Vec<PlatformCertificate>) -> Result<Self> {
        let now = Local::now();
        let mut certificates: Vec<_> = certificates
            .into_iter()
            .filter(|c| now < c.expire_time)
            .collect();
        if certificates.is_empty() {
            return Err(anyhow::format_err!("no available certificates found"));
        }
        certificates.sort_by_key(|c| Reverse(c.effective_time));
        Ok(PlatformCertificateState {
            certificates,
            newest_certificate_idx: 0,
        })
    }

    /// 根据 serial_no 获取平台证书。
    pub fn get_platform_certificate(&self, serial_no: &str) -> Result<PlatformCertificate> {
        let certificate = self
            .certificates
            .iter()
            .find(|c| c.serial_no == serial_no)
            .ok_or_else(|| {
                anyhow::format_err!("no certificate found for serial_no: {}", serial_no)
            })?
            .clone();
        Ok(certificate)
    }

    /// 平台证书列表
    pub fn certificates(&self) -> &Vec<PlatformCertificate> {
        &self.certificates
    }
}

/// 响应签名验证器: 对响应进行数字签名验证。
/// 验证响应的签名。
/// <https://pay.weixin.qq.com/wiki/doc/apiv3/wechatpay/wechatpay4_1.shtml>
pub async fn verify_response(public_key: &RsaPublicKey, res: Response) -> Result<Response> {
    // 需要这个 builder 重新构建一个 Response 并返回。
    let mut builder = http::Response::builder()
        .status(res.status())
        .version(res.version());
    for (key, value) in res.headers() {
        builder = builder.header(key, value);
    }

    let signature = res
        .headers()
        .get("Wechatpay-Signature")
        .ok_or_else(|| anyhow::format_err!("missing `Wechatpay-Signature` header"))?
        .to_str()?;
    let signature = BASE64_STANDARD.decode(signature.as_bytes())?;

    let timestamp = res
        .headers()
        .get("Wechatpay-Timestamp")
        .ok_or_else(|| anyhow::format_err!("missing `Wechatpay-Timestamp` header"))?
        .to_str()?;
    let nonce_str = res
        .headers()
        .get("Wechatpay-Nonce")
        .ok_or_else(|| anyhow::format_err!("missing `Wechatpay-Nonce` header"))?
        .to_str()?;

    let mut msg = BytesMut::new();
    msg.put_slice(timestamp.as_bytes());
    msg.put_u8(b'\n');
    msg.put_slice(nonce_str.as_bytes());
    msg.put_u8(b'\n');
    let body = res.text().await?;
    msg.put_slice(body.as_bytes());
    msg.put_u8(b'\n');

    let verifying_key = VerifyingKey::<Sha256>::new(public_key.clone());
    let signature = Signature::try_from(signature.as_slice())?;
    verifying_key.verify(&msg, &signature)?;

    let new_res = builder.body(body)?;
    Ok(new_res.into())
}

/// 获取微信支付平台证书。
/// 此接口与其他接口不同。收到响应时，需要先处理响应，后进行验签。因此单独实现。
pub async fn get_platform_certificates(
    mch_credential: &MchCredential,
) -> Result<Vec<PlatformCertificate>> {
    #[derive(Deserialize)]
    struct EncryptedCertificate {
        #[allow(unused)]
        algorithm: String,
        nonce: String,
        associated_data: String,
        ciphertext: String,
    }

    #[derive(Deserialize)]
    struct PlatformCertificateItem {
        serial_no: String,
        #[serde(with = "datetime_fmt")]
        effective_time: DateTime<Local>,
        #[serde(with = "datetime_fmt")]
        expire_time: DateTime<Local>,
        encrypt_certificate: EncryptedCertificate,
    }

    #[derive(Deserialize)]
    struct GetPlatformCertificatesRes {
        data: Vec<PlatformCertificateItem>,
    }

    let client = Client::new();
    let url = format!("{}/certificates", BASE_URL);
    let mut req = client.get(&url).build()?;
    req.headers_mut()
        .append("Accept", "application/json".parse().unwrap());
    req.headers_mut()
        .append("User-Agent", USER_AGENT.parse().unwrap());

    let req = mch_credential.sign_request(req)?;
    let res = client.execute(req).await?;

    // 用于验签的 serial_no
    let serial_no = res
        .headers()
        .get("Wechatpay-Serial")
        .ok_or_else(|| anyhow::format_err!("missing `Wechatpay-Serial` header"))?
        .to_str()?
        .to_string();

    // 需要这个 builder 重新构建一个 Response 以便最后进行验签。
    let mut builder = http::Response::builder()
        .status(res.status())
        .version(res.version());
    for (key, value) in res.headers() {
        builder = builder.header(key, value);
    }
    let body_txt = res.text().await?;

    let mut platform_certificates = vec![];
    for item in serde_json::from_str::<GetPlatformCertificatesRes>(&body_txt)?.data {
        let ciphertext = BASE64_STANDARD.decode(&item.encrypt_certificate.ciphertext)?;
        let plain = mch_credential.aes_decrypt(
            &ciphertext,
            item.encrypt_certificate.associated_data.as_bytes(),
            item.encrypt_certificate.nonce.as_bytes(),
        )?;
        let certificate = PlatformCertificate {
            serial_no: item.serial_no,
            effective_time: item.effective_time,
            expire_time: item.expire_time,
            certificate: Certificate::from_pem(&plain)?,
        };
        platform_certificates.push(certificate);
    }

    let public_key = platform_certificates
        .iter()
        .find(|c| c.serial_no == serial_no)
        .ok_or_else(|| anyhow::format_err!("no certificate found for serial_no: {}", serial_no))?
        .public_key()?;

    let res_clone = Response::from(builder.body(body_txt)?);
    verify_response(&public_key, res_clone).await?;
    Ok(platform_certificates)
}
