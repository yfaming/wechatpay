wechatpay
=========

支持 async 的 微信支付的 Rust SDK。

# Example
```rust
use wechatpay::MchCredential;

async fn main() -> anyhow::Result<()> {
    let credential = MchCredential {
        mch_id: "<商户号>".to_string(),
        mch_certificate_serial_no: "<商户 API 证书序列号>".to_string(),
        mch_rsa_private_key: RsaPrivateKey::from_pkcs8_pem("<商户 RSA 私钥>")?,
        mch_api_v3_key: "<商户 API v3 密钥>".to_string()),
    };

    let mut builder = WechatPayClient::builder();
    let wechatpay_client = builder.mch_credential(credential)
           .fetch_platform_certificates()
           .build().await?;

    Ok(())
}
```

# wechatpay 之签名/验签，加密/解密关键点
* 签名/验签，使用的是 SHA256 with RSA 签名算法。
但是，此描述不够准确，因为其实际包含 PKCS1v1.5 和 PSS 两个变种。
查看官方 [wechatpay-go](https://github.com/wechatpay-apiv3/wechatpay-go) 代码 utils/sign.go 之 SHA256WithRSASigner struct 之实现，其用的是 PKCS1v1.5 变种。

* 敏感信息字段的加解密。
对于敏感信息字段(如用户地址、银行卡号、手机号等)，也要求加密。
加密算法使用 RSA，填充方案为 RSAES-OAEP(Optimal Asymmetric Encryption Padding)。
具体地，商户对上送的敏感信息字段加密，加密密钥为微信支付平台公钥。
微信支付也会对下行的敏感信息字段进行加密，加密密钥为商户的公钥。商户则通过自己的私钥进行解密。
Rust 的 rsa 库文档中也有示例 https://docs.rs/rsa/latest/rsa/#oaep-encryption

* 回调通知(如订单支付通知，退款结果通知)和平台证书下载接口，使用了 AES-256-GCM 算法进行加密。加密密钥为商户 API v3 密钥。
此为对称加密算法(即，同样用商户 API v3 密钥解密)。Rust 有 `aes_gcm` crate 可用， https://docs.rs/aes-gcm/latest/aes_gcm/

* 下载平台证书的接口，验签逻辑需要特殊化处理。
下载平台证书的接口，也需要验证签名，但验证签名需要用到此接口返回的平台证书。而其他接口，则是使用本地缓存的平台证书进行验签。
此接口需要先解密出平台证书(此接口返回的内容是加密过的)，然后用它来验证签名。

* 平台证书是 x509 格式的，Rust 有 [`x509_cert`](https://docs.rs/x509-cert/0.2.1/x509_cert/index.html) 可用。
它也属于  RustCrypto (前面提到的 `rsa` 和 `aes_gcm` 库也都属于 RustCrypto)。`rsa` 也支持读取各种格式的密钥文件如 `.pem` 等。


# TODO
* 将 WechatPayClient 支持 tower service 形式的 middleware，可能需要以 builder 方式构造。
* 增加测试
* 增加文档与示例代码
