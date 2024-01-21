//! 微信支付通知。包括支付结果与退款结果的通知。

use crate::refund::RefundQueryResponse;
use crate::util::datetime_fmt;
use crate::{client::WechatPayClient, trade::TradeQueryResponse};
use anyhow::Result;
use bytes::Bytes;
use chrono::{DateTime, Local};
use http::{StatusCode, Version};
use hyper::Body;
use serde::{Deserialize, Serialize};

/// 微信支付通知。
/// 包括支付结果与退款结果。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WechatPayNotification {
    /// 通知的唯一 ID，长度不超过 36 字符。
    pub id: String,
    /// 通知创建的时间
    #[serde(with = "datetime_fmt")]
    pub create_time: DateTime<Local>,
    /// 通知类型。不超过 32 字符。
    /// TRANSACTION.SUCCESS：支付成功通知。
    /// REFUND.SUCCESS：退款成功通知
    /// REFUND.ABNORMAL：退款异常通知
    /// REFUND.CLOSED：退款关闭通知
    pub event_type: String,
    /// 通知的资源数据类型，不超过 32 字符。支付成功通知为 encrypt-resource。
    pub resource_type: String,
    /// 通知资源数据。
    pub resource: NotificationResourse,
    /// 回调摘要。不超过 64 字符。
    pub summary: String,
}

/// 通知资源数据。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationResourse {
    /// 加密算法类型。目前只支持AEAD_AES_256_GCM。
    pub algorithm: String,
    /// 数据密文。已经过 base64 编码。
    pub ciphertext: String,
    /// 附加数据。
    pub associated_data: String,
    /// 原始类型
    /// 支付通知的类型为 transaction
    /// 退款通知的类型为 refund
    pub original_type: String,
    /// 随机串
    pub nonce: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NotificationEvent {
    Trade(TradeQueryResponse),
    Refund(RefundQueryResponse),
}

impl WechatPayClient {
    /// 对微信支付结果通知进行验签。
    /// 为避免对于具体 web 框架的依赖，这里的参数为 `http::Request<hyper::Body>`。
    /// 参见 <https://pay.weixin.qq.com/wiki/doc/apiv3/apis/chapter3_1_5.shtml>
    pub async fn verify_notification(
        &self,
        req: http::Request<Body>,
    ) -> Result<http::Request<Bytes>> {
        let method = req.method().clone();
        let uri = req.uri().clone();
        let version = req.version();

        // 为避免代码重复，这里从 request 构造出一个 reponse 并进行验签。
        let mut res_builder = http::Response::builder()
            .status(StatusCode::OK)
            .version(Version::HTTP_11);
        for (key, value) in req.headers() {
            res_builder = res_builder.header(key, value);
        }
        let res: reqwest::Response = res_builder.body(req.into_body())?.into();
        let res = self.verify_response(res).await?;

        // 验签通过，再又基于 response 构建 request
        let mut req_builder = http::Request::builder()
            .method(method)
            .uri(uri)
            .version(version);
        for (key, value) in res.headers() {
            req_builder = req_builder.header(key, value);
        }
        let body = res.bytes().await?;
        let req: http::Request<Bytes> = req_builder.body(body)?;
        Ok(req)
    }

    /// 解密微信支付结果通知，解密结果为 TradeQueryResponse
    pub fn decrypt_notification(&self, noti: &WechatPayNotification) -> Result<NotificationEvent> {
        let plain = self.mch_credential.aes_decrypt(
            noti.resource.ciphertext.as_bytes(),
            noti.resource.associated_data.as_bytes(),
            noti.resource.nonce.as_bytes(),
        )?;

        let event = match noti.resource.original_type.as_str() {
            "transaction" => NotificationEvent::Trade(serde_json::from_slice(&plain)?),
            "refund" => NotificationEvent::Refund(serde_json::from_slice(&plain)?),
            _ => {
                return Err(anyhow::anyhow!(
                    "unknown notification type: {}",
                    noti.resource.original_type
                ));
            }
        };
        Ok(event)
    }
}
