//! 退款相关接口。

use crate::client::WechatPayClient;
use crate::client::BASE_URL;
use crate::util::datetime_fmt;
use crate::util::option_datetime_fmt;
use anyhow::Result;
use chrono::{DateTime, Local};
use serde::Deserializer;
use serde::{Deserialize, Serialize};

impl WechatPayClient {
    /// 申请退款。
    /// 参见 <https://pay.weixin.qq.com/wiki/doc/apiv3/apis/chapter3_1_9.shtml>
    pub async fn apply_refund(&self, params: &RefundParams) -> Result<RefundQueryResponse> {
        let url = format!("{}/refund/domestic/refunds", BASE_URL);
        let req = self.client.post(&url).json(params).build()?;
        let res = self.execute(req).await?;
        let res: RefundQueryResponse = res.json().await?;
        Ok(res)
    }

    /// 查询退款。
    /// 参见 <https://pay.weixin.qq.com/wiki/doc/apiv3/apis/chapter3_1_10.shtml>
    pub async fn query_refund(&self, out_refund_no: &str) -> Result<RefundQueryResponse> {
        let url = format!("{}/refund/domestic/refunds/{}", BASE_URL, out_refund_no);
        let req = self.client.get(url).build()?;
        let res = self.execute(req).await?;
        let res: RefundQueryResponse = res.json().await?;
        Ok(res)
    }
}

/// 申请退款的参数。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefundParams {
    #[serde(flatten)]
    trade_id: TradeId,
    /// 商户退款单号，不超过 64 字符。
    /// 商户系统内部的退款单号，商户系统内部唯一，只能是数字、大小写字母_-|*@
    pub out_refund_no: String,
    /// 退款原因，不超过 80 字符。
    /// 若传入，会在下发给用户的退款消息中体现退款原因。
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub reason: Option<String>,
    /// 退款结果回调 url。
    /// 异步接收微信支付退款结果通知的回调地址，通知url必须为外网可访问的url，不能携带参数。
    /// 如果参数中传了notify_url，则商户平台上配置的回调地址将不会生效，优先回调当前传的这个地址。
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub notify_url: Option<String>,
    /// 退款资金来源。
    /// 若传递此参数则使用对应的资金账户退款，否则默认使用未结算资金退款（仅对老资金流商户适用）。
    /// 枚举值：
    /// * AVAILABLE：可用余额账户
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub funds_account: Option<String>,
    /// 金额信息
    pub amount: RefundApplyingAmount,
    /// 退款商品。
    /// 指定商品退款需要传此参数，其他场景无需传递。
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub goods_detail: Vec<RefundGoodsDetail>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum TradeId {
    /// 微信支付订单号
    #[serde(rename = "transaction_id")]
    TransactionId(String),
    /// 商户订单号
    #[serde(rename = "out_trade_no")]
    OutTradeNo(String),
}

/// 申请退款的金额信息。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefundApplyingAmount {
    /// 原支付交易的订单总金额，单位为分，只能为整数。
    pub total: i32,
    /// 退款金额，单位为分，只能为整数，不能超过原订单支付金额。
    pub refund: i32,
    /// 退款币种。符合ISO 4217标准的三位字母代码，目前只支持人民币：CNY。
    pub currency: String,
    /// 退款出资账户及金额。
    /// 退款需要从指定账户出资时，传递此参数指定出资金额（币种的最小单位，只能为整数）。
    /// 同时指定多个账户出资退款的使用场景需要满足以下条件：
    /// 1. 未开通退款支出分离产品功能；
    /// 2. 订单属于分账订单，且分账处于待分账或分账中状态。
    /// 参数传递需要满足条件：
    /// 1. 基本账户可用余额出资金额与基本账户不可用余额出资金额之和等于退款金额；
    /// 2. 账户类型不能重复。
    /// 上述任一条件不满足将返回错误
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub from: Vec<RefundFromAccount>,
}

/// 退款出资账户及金额
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefundFromAccount {
    /// 出资账户类型。枚举值：
    /// * AVAILABLE : 可用余额
    /// * UNAVAILABLE : 不可用余额
    pub account: String,
    /// 对应账户出资金额。
    pub amount: i32,
}

/// 退款商品
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefundGoodsDetail {
    /// 商户侧商品编码。
    /// 由半角的大小写字母、数字、中划线、下划线中的一种或几种组成。
    pub merchant_goods_id: String,
    /// 微信支付定义的统一商品编号（没有可不传）
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub wechatpay_goods_id: Option<String>,
    /// 商品名称
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub goods_name: Option<String>,
    /// 商品单价，单位为分。如果商户有优惠，需传输商户优惠后的单价。
    pub unit_price: i32,
    /// 商品退款金额。单位为分。
    pub refund_amount: i32,
    /// 商品退货数量。
    pub refund_quantity: i32,
}

/// 退款查询响应。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefundQueryResponse {
    /// 微信支付退款单号。不超过 32 字符。
    pub refund_id: String,
    /// 商户退款单号，不超过 64 字符。
    /// 商户系统内部的退款单号，商户系统内部唯一，只能是数字、大小写字母_-|*@
    pub out_refund_no: String,
    /// 微信支付订单号。不超过 32 字符。
    pub transaction_id: String,
    /// 商户订单号。不超过 32 字符。
    pub out_trade_no: String,
    /// 退款渠道。不超过 32 字符。
    /// 枚举值:
    /// * ORIGINAL：原路退款
    /// * BALANCE：退回到余额
    /// * OTHER_BALANCE：原账户异常退到其他余额账户
    /// * OTHER_BANKCARD：原银行卡异常退到其他银行卡
    pub channel: String,
    /// 退款入账账户。不超过 64 字符。
    /// 取当前退款单的退款入账方，有以下几种情况：
    /// 1. 退回银行卡：{银行名称}{卡类型}{卡尾号}
    /// 2. 退回支付用户零钱:支付用户零钱
    /// 3. 退还商户:商户基本账户商户结算银行账户
    /// 4. 退回支付用户零钱通:支付用户零钱通
    pub user_received_account: String,

    /// 退款成功时间，当退款状态为退款成功时有返回。
    #[serde(
        with = "option_datetime_fmt",
        skip_serializing_if = "Option::is_none",
        default
    )]
    pub success_time: Option<DateTime<Local>>,
    /// 退款创建时间。
    #[serde(with = "datetime_fmt", default)]
    pub create_time: DateTime<Local>,
    /// 退款状态。
    pub status: RefundStatus,
    /// 资金账户。退款所使用资金对应的资金账户类型。
    /// 枚举值：
    /// * UNSETTLED: 未结算资金
    /// * AVAILABLE: 可用余额
    /// * UNAVAILABLE: 不可用余额
    /// * OPERATION: 运营户
    /// * BASIC: 基本账户（含可用余额和不可用余额）
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub funds_account: Option<String>,
    /// 金额详细信息
    pub amount: RefundActualAmount,
    /// 优惠退款信息
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub promotion_detail: Vec<RefundPromotionDetail>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefundStatus {
    /// 退款成功
    Success,
    /// 退款关闭
    Closed,
    /// 退款处理中
    Processing,
    /// 退款异常
    Abnormal,
}

impl<'de> Deserialize<'de> for RefundStatus {
    fn deserialize<D>(deserializer: D) -> Result<RefundStatus, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        match s.as_str() {
            "SUCCESS" => Ok(RefundStatus::Success),
            "CLOSED" => Ok(RefundStatus::Closed),
            "PROCESSING" => Ok(RefundStatus::Processing),
            "ABNORMAL" => Ok(RefundStatus::Abnormal),
            _ => Err(serde::de::Error::custom(format!(
                "unknown refund status: {}",
                s
            ))),
        }
    }
}

impl Serialize for RefundStatus {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let s = match self {
            RefundStatus::Success => "SUCCESS",
            RefundStatus::Closed => "CLOSED",
            RefundStatus::Processing => "PROCESSING",
            RefundStatus::Abnormal => "ABNORMAL",
        };
        serializer.serialize_str(s)
    }
}

/// 实际退款的金额信息。
/// 此 struct 的各个字段，貌似有点难以理解。
/// refund: 当是申请退款时传入的退款金额(即 RefundParams.amount.refund)。
/// payer_refund: 当是用户实际收到的退款金额。
/// 二者的差异是因为，由于用户使用了优惠券，订单金额与用户实际支付金额本就不一致。
/// 看来，如果如果使用微信支付的优惠券功能，计算将会比较复杂。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefundActualAmount {
    /// 原支付交易的订单总金额，单位为分，只能为整数。
    pub total: i32,
    /// 退款标价金额，单位为分，可以做部分退款。
    pub refund: i32,
    /// 现金支付金额，单位为分。
    pub payer_total: i32,
    /// 退款给用户的金额，不包含所有优惠券金额。
    pub payer_refund: i32,

    /// 退款出资账户及金额。
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub from: Vec<RefundFromAccount>,

    /// 应结订单金额=订单金额-免充值代金券金额，应结订单金额<=订单金额，单位为分
    pub settlement_total: i32,
    /// 应结退款金额。去掉非充值代金券退款金额后的退款金额，单位为分。
    /// 退款金额=申请退款金额-非充值代金券退款金额，退款金额<=申请退款金额
    pub settlement_refund: i32,

    /// 优惠退款金额<=退款金额，退款金额-代金券或立减优惠退款金额为现金。
    pub discount_refund: i32,

    /// 退款币种。符合ISO 4217标准的三位字母代码，目前只支持人民币：CNY。
    pub currency: String,

    /// 手续费退款金额
    pub refund_fee: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefundPromotionDetail {
    /// 券ID
    pub coupon_id: String,
    /// 优惠范围
    /// GLOBAL：全场代金券
    /// SINGLE：单品优惠
    pub scope: Option<String>,
    /// 优惠类型
    /// * COUPON：代金券，需要走结算资金的充值型代金券
    /// * DISCOUNT：优惠券，不走结算资金的免充值型优惠券
    #[serde(rename = "type")]
    pub promotion_type: Option<String>,
    /// 优惠券面额
    pub amount: i32,
    /// 优惠退款金额。
    /// 优惠退款金额<=退款金额，退款金额-代金券或立减优惠退款金额为用户支付的现金。
    pub refund_amount: i32,
    /// 商品列表
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub goods_detail: Vec<RefundGoodsDetail>,
}
