use crate::credential::MchCredential;
use crate::error::WechatPayApiError;
use crate::platform_certificate::{
    get_platform_certificates, PlatformCertificate, PlatformCertificateState,
};
use anyhow::Result;
use reqwest::{Client, Request, Response};
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone)]
pub struct WechatPayClient {
    pub(crate) client: Client,
    pub(crate) mch_credential: MchCredential,
    pub(crate) platform_certificate_state: Arc<Mutex<PlatformCertificateState>>,
}

pub(crate) const BASE_URL: &str = "https://api.mch.weixin.qq.com/v3";

pub(crate) const USER_AGENT: &str = "wechatpay Rust client";

impl WechatPayClient {
    pub fn builder() -> WechatPayClientBuilder {
        WechatPayClientBuilder::new()
    }

    /// 执行 HTTP 请求
    /// 请求发送时，先进行签名；收到响应时，先进行验签，通过后再返回。
    /// (本 crate 未实现的接口，可以通过此方法访问)
    pub async fn execute(&self, req: Request) -> Result<Response> {
        let mut req = req;
        // 根据 https://pay.weixin.qq.com/wiki/doc/apiv3/wechatpay/wechatpay2_0.shtml#part-1
        // 给所有请求都加上 accept header。
        req.headers_mut()
            .append("Accept", "application/json".parse().unwrap());

        let req = self.mch_credential.sign_request(req)?;
        let res = self.client.execute(req).await?;

        // 请求出错时，响应中可能不存在验签相关的字段。因此直接返回 error。
        if !res.status().is_success() {
            let e: WechatPayApiError = res.json().await?;
            Err(e.into())
        } else {
            let res = self.verify_response(res).await?;
            Ok(res)
        }
    }

    /// 对响应进行数字签名验证。
    pub(crate) async fn verify_response(&self, res: Response) -> Result<Response> {
        let serial_no = res
            .headers()
            .get("Wechatpay-Serial")
            .ok_or_else(|| anyhow::format_err!("missing `Wechatpay-Serial` header"))?
            .to_str()?
            .to_string();

        let certificate = self
            .platform_certificate_state
            .lock()
            .unwrap()
            .get_platform_certificate(&serial_no)?;
        let res = certificate.verify_response(res).await?;
        Ok(res)
    }

    /// 获取平台证书列表。
    pub async fn get_platform_certificates(&self) -> Result<Vec<PlatformCertificate>> {
        let platform_certificates = get_platform_certificates(&self.mch_credential).await?;
        let mut state = self.platform_certificate_state.lock().unwrap();
        *state = PlatformCertificateState::new(platform_certificates.clone())?;
        Ok(platform_certificates)
    }
}

/// builder for `WechatPayClient`.
#[derive(Debug, Default)]
pub struct WechatPayClientBuilder {
    mch_credential: Option<MchCredential>,
    platform_certificates: Option<Vec<PlatformCertificate>>,
    fetch_platform_certificates: bool,

    user_agent: Option<String>,
}

impl WechatPayClientBuilder {
    fn new() -> WechatPayClientBuilder {
        WechatPayClientBuilder {
            ..Default::default()
        }
    }

    pub fn mch_credential(&mut self, mch_credential: MchCredential) -> &mut Self {
        self.mch_credential = Some(mch_credential);
        self
    }

    /// 平台证书列表。如果指定 fetch_platform_certificates 为 true，则此参数无效。
    pub fn platform_certificates(
        &mut self,
        platform_certificates: Vec<PlatformCertificate>,
    ) -> &mut Self {
        self.platform_certificates = Some(platform_certificates);
        self
    }

    /// build 时是否获取最新的平台证书列表。
    /// 如果指定 platform_certificates，则指定的  platform_certificates 无效。
    pub fn fetch_platform_certificates(&mut self) -> &mut Self {
        self.fetch_platform_certificates = true;
        self
    }

    /// 指定 User Agent。
    /// 如果未指定，将默认使用 "wechatpay Rust client"。
    /// 对于未指定 User Agent header 的请求，微信支付可能会拒绝。
    /// 参见 <https://pay.weixin.qq.com/wiki/doc/apiv3/wechatpay/wechatpay2_0.shtml#part-8>
    pub fn user_agent(&mut self, ua: String) -> &mut Self {
        self.user_agent = Some(ua);
        self
    }

    pub async fn build(&mut self) -> Result<WechatPayClient> {
        let mch_credential = self
            .mch_credential
            .take()
            .ok_or_else(|| anyhow::format_err!("missing `mch_credential`"))?;

        let platform_certificates = if self.fetch_platform_certificates {
            get_platform_certificates(&mch_credential).await?
        } else {
            self.platform_certificates
                .take()
                .ok_or_else(|| anyhow::format_err!("missing `platform_certificates`"))?
        };

        if platform_certificates.is_empty() {
            return Err(anyhow::format_err!("empty `platform_certificates`"));
        }

        let platform_certificate_state = Arc::new(Mutex::new(PlatformCertificateState::new(
            platform_certificates,
        )?));

        let ua = if let Some(ua) = &self.user_agent {
            ua
        } else {
            USER_AGENT
        };
        let client_builder = Client::builder().user_agent(ua);

        Ok(WechatPayClient {
            client: client_builder.build()?,
            mch_credential,
            platform_certificate_state,
        })
    }
}
