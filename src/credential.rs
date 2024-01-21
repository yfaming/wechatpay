//! 微信支付商户的证书和密钥。
//! 这些信息均为敏感信息，注意确保安全，避免泄露。

use aes_gcm::aead::{Aead, KeyInit, Payload};
use aes_gcm::{Aes256Gcm, Nonce};
use anyhow::Result;
use base64::prelude::*;
use bytes::{BufMut, BytesMut};
use rand::Rng;
use reqwest::header::AUTHORIZATION;
use reqwest::Request;
use rsa::pkcs1v15::SigningKey;
use rsa::sha2::Sha256;
use rsa::signature::{RandomizedSigner, SignatureEncoding};
use rsa::RsaPrivateKey;
use std::fmt::Debug;
use std::time::{SystemTime, UNIX_EPOCH};

/// 微信支付商户的证书和密钥
#[derive(Clone)]
pub struct MchCredential {
    /// 商户号
    pub mch_id: String,
    /// 商户 API 证书序列号
    pub mch_certificate_serial_no: String,
    /// 商户 RSA 私钥
    pub mch_rsa_private_key: RsaPrivateKey,
    /// 商户 API v3 密钥
    pub mch_api_v3_key: String,
}

impl MchCredential {
    /// 使用商户 RSA 私钥，对请求进行数字签名。
    /// <https://pay.weixin.qq.com/wiki/doc/apiv3/wechatpay/wechatpay4_0.shtml>
    pub fn sign_request(&self, mut req: Request) -> Result<Request> {
        const SIGNATURE_TYPE: &str = "WECHATPAY2-SHA256-RSA2048";

        let mut msg = BytesMut::new();

        msg.put_slice(req.method().as_str().as_bytes());
        msg.put_u8(b'\n');

        let url = if let Some(quer) = req.url().query() {
            format!("{}?{}", req.url().path(), quer)
        } else {
            req.url().path().to_string()
        };
        msg.put_slice(url.as_bytes());
        msg.put_u8(b'\n');

        let timestamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();
        msg.put_slice(format!("{}", timestamp).as_bytes());
        msg.put_u8(b'\n');

        let nonce_str = generate_none_str(32);
        msg.put_slice(nonce_str.as_bytes());
        msg.put_u8(b'\n');

        if let Some(body) = req.body() {
            // 由本项目保证 body.as_bytes() 返回 Some(...)。
            // 也即，由本项目保证 body 为  `Reusable`，而非 `Streaming`。
            msg.put_slice(body.as_bytes().unwrap());
        }
        msg.put_u8(b'\n');

        let mut rng = rand::thread_rng();
        let signing_key = SigningKey::<Sha256>::new(self.mch_rsa_private_key.clone());
        let signature = signing_key.sign_with_rng(&mut rng, &msg).to_bytes();
        let signature = BASE64_STANDARD.encode(&signature);

        let authorization_value = format!(
            r#"{} mchid="{}",nonce_str="{}",signature="{}",timestamp="{}",serial_no="{}""#,
            SIGNATURE_TYPE,
            self.mch_id,
            nonce_str,
            signature,
            timestamp,
            self.mch_certificate_serial_no
        );
        req.headers_mut()
            .insert(AUTHORIZATION, authorization_value.parse().unwrap());

        Ok(req)
    }

    /// 使用商户 API v3 密钥解密
    pub fn aes_decrypt(
        &self,
        ciphertext: &[u8],
        associated_data: &[u8],
        nonce: &[u8],
    ) -> Result<Vec<u8>> {
        let cipher = Aes256Gcm::new_from_slice(self.mch_api_v3_key.as_bytes())?;
        let nonce = Nonce::from_slice(nonce);
        let payload = Payload {
            msg: ciphertext,
            aad: associated_data,
        };

        let plaintext = cipher.decrypt(nonce, payload)?;
        Ok(plaintext)
    }

    /// 使用商户 API v3 密钥解密，并转换为字符串
    pub fn aes_decrypt_to_string(
        &self,
        ciphertext: &[u8],
        associated_data: &[u8],
        nonce: &[u8],
    ) -> Result<String> {
        let bytes = self.aes_decrypt(ciphertext, associated_data, nonce)?;
        Ok(String::from_utf8(bytes)?)
    }
}

/// 生成随机的 none_str
pub fn generate_none_str(n: usize) -> String {
    // 去掉了符号及容易混淆的字符等，比如 0, o, O, 1, l, i, I。
    const ALPHABET: &[u8] = b"abcdefghjkmnpqrstuvwxyzABCDEFGHJKLMNPQRSTRVWXYZ23456789";
    let mut rng = rand::thread_rng();
    (0..n)
        .map(|_| {
            let idx = rng.gen_range(0..ALPHABET.len());
            ALPHABET[idx] as char
        })
        .collect::<String>()
}

impl Debug for MchCredential {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MchCredential")
            .field("mch_id", &self.mch_id)
            .field("mch_certificate_serial_no", &"...")
            .field("mch_rsa_private_key", &"...")
            .field("mch_api_v3_key", &"...")
            .finish()
    }
}
