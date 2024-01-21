use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize, thiserror::Error)]
#[serde(default)]
#[error("微信支付错误: {message}")]
pub struct WechatPayApiError {
    /// 错误码
    code: String,
    /// 错误描述
    message: String,
    /// 错误详情
    detail: WechatPayErrorDetail,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct WechatPayErrorDetail {
    /// 指示错误参数的位置
    pub field: String,
    /// 错误的值
    pub value: String,
    /// 具体错误原因
    pub issue: String,
    /// 出错的位置
    pub location: String,
}
