//! 交易相关接口的实现

use crate::client::{WechatPayClient, BASE_URL};
use crate::credential::generate_none_str;
use crate::util::option_datetime_fmt;
use anyhow::Result;
use base64::prelude::*;
use chrono::{DateTime, Local};
use rand::Rng;
use rsa::pkcs1v15::SigningKey;
use rsa::sha2::Sha256;
use rsa::signature::{RandomizedSigner, SignatureEncoding};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

impl WechatPayClient {
    /// JSAPI 下单，返回 prepay_id。
    /// 参见 <https://pay.weixin.qq.com/wiki/doc/apiv3/apis/chapter3_1_1.shtml>
    pub async fn jsapi_create_trade(&self, params: &JsApiCreateTradeParams) -> Result<String> {
        let url = format!("{}/pay/transactions/jsapi", BASE_URL);
        let req = self.client.post(url).json(params).build()?;
        let res = self.execute(req).await?;
        let res: JsApiCreateTradeResponse = res.json().await?;
        Ok(res.prepay_id)
    }

    /// APP 下单，返回 `prepay_id`。
    /// 参见 <https://pay.weixin.qq.com/wiki/doc/apiv3/apis/chapter3_2_1.shtml>
    pub async fn app_create_trade(&self, params: &AppCreateTradeParams) -> Result<String> {
        let url = format!("{}/pay/transactions/app", BASE_URL);
        let req = self.client.post(url).json(params).build()?;
        let res = self.execute(req).await?;
        let res: AppCreateTradeResponse = res.json().await?;
        Ok(res.prepay_id)
    }

    /// H5 下单，返回 h5_url。
    /// 参见 <https://pay.weixin.qq.com/wiki/doc/apiv3/apis/chapter3_3_1.shtml>
    pub async fn h5_create_trade(&self, params: &H5CreateTradeParams) -> Result<String> {
        let url = format!("{}/pay/transactions/h5", BASE_URL);
        let req = self.client.post(url).json(params).build()?;
        let res = self.execute(req).await?;
        let res: H5CreateTradeResponse = res.json().await?;
        Ok(res.h5_url)
    }

    /// Native 下单，返回二维码 url (code_url)。
    /// code_url 用于生成支付二维码，然后提供给用户扫码支付。
    /// 参见 <https://pay.weixin.qq.com/wiki/doc/apiv3/apis/chapter3_4_1.shtml>
    pub async fn native_create_trade(&self, params: &NativeCreateTradeParams) -> Result<String> {
        let url = format!("{}/pay/transactions/native", BASE_URL);
        let req = self.client.post(url).json(params).build()?;
        let res = self.execute(req).await?;
        let res: NativeCreateTradeResponse = res.json().await?;
        Ok(res.code_url)
    }

    /// 通过微信支付订单号(transaction_id)查询订单。
    /// 参见 <https://pay.weixin.qq.com/wiki/doc/apiv3/apis/chapter3_1_2.shtml>
    pub async fn query_trade_by_transaction_id(
        &self,
        transaction_id: &str,
    ) -> Result<TradeQueryResponse> {
        let url = format!(
            "{}/pay/transactions/id/{}?mchid={}",
            BASE_URL, transaction_id, &self.mch_credential.mch_id
        );
        let req = self.client.get(url).build()?;
        let res = self.execute(req).await?;
        let res: TradeQueryResponse = res.json().await?;
        Ok(res)
    }

    /// 通过商户订单号查询(out_trade_no)查询订单。
    /// 参见 <https://pay.weixin.qq.com/wiki/doc/apiv3/apis/chapter3_1_2.shtml>
    pub async fn query_trade_by_out_trade_no(
        &self,
        out_trade_no: &str,
    ) -> Result<TradeQueryResponse> {
        let url = format!(
            "{}/pay/transactions/out-trade-no/{}?mchid={}",
            BASE_URL, out_trade_no, &self.mch_credential.mch_id
        );
        let req = self.client.get(url).build()?;
        let res = self.execute(req).await?;
        let res: TradeQueryResponse = res.json().await?;
        Ok(res)
    }

    /// 关闭订单。
    /// 参见 <https://pay.weixin.qq.com/wiki/doc/apiv3/apis/chapter3_1_3.shtml>
    pub async fn close_trade(&self, out_trade_no: &str) -> Result<()> {
        #[derive(Debug, Clone, Serialize, Deserialize)]
        struct CloseTradeRequest {
            #[serde(rename = "mchid")]
            mch_id: String,
        }

        let url = format!(
            "{}/pay/transactions/out-trade-no/{}/close",
            BASE_URL, out_trade_no
        );
        let req = CloseTradeRequest {
            mch_id: self.mch_credential.mch_id.clone(),
        };
        let req = self.client.post(url).json(&req).build()?;
        let _res = self.execute(req).await?;
        Ok(())
    }
}

impl WechatPayClient {
    /// 对 JSAPI 下单返回的 prepay_id 进行签名。
    /// 前端在调起微信支付时，需要这些参数。
    /// 参见 <https://pay.weixin.qq.com/wiki/doc/apiv3/apis/chapter3_1_4.shtml>
    pub fn sign_jsapi_trade(&self, prepay_id: &str, app_id: &str) -> JsApiTradeSignature {
        let timestamp = Local::now().timestamp();
        let nonce_str = generate_none_str(32);
        let package = format!("prepay_id={}", prepay_id);
        let msg = format!("{}\n{}\n{}\n{}\n", app_id, timestamp, nonce_str, package);

        let mut rng = rand::thread_rng();
        let signing_key =
            SigningKey::<Sha256>::new(self.mch_credential.mch_rsa_private_key.clone());
        let signature = signing_key
            .sign_with_rng(&mut rng, msg.as_bytes())
            .to_bytes();
        let signature = BASE64_STANDARD.encode(&signature);

        JsApiTradeSignature {
            app_id: app_id.to_string(),
            timestamp: timestamp.to_string(),
            nonce_str,
            package,
            sign_type: "RSA".to_string(),
            pay_sign: signature,
        }
    }
}

/// JSAPI 下单时，针对返回的 prepay_id 生成的签名，
/// 前端在调起微信支付时，需要这些参数。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsApiTradeSignature {
    pub app_id: String,
    pub timestamp: String, // 注意，单位为秒。类型为 string。
    pub nonce_str: String,
    // 须形如 `prepay_id=xxxxx`。注意 xxxx 前后无引号。
    pub package: String,
    // 统一为 RSA
    pub sign_type: String,
    pub pay_sign: String,
}

/// JSAPI 下单参数
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsApiCreateTradeParams {
    /// 应用 ID
    #[serde(rename = "appid")]
    pub app_id: String,
    #[serde(rename = "mchid")]
    /// 商户号
    pub mch_id: String,
    /// 商品描述。不超过 127 字符。
    pub description: String,
    /// 商户订单号。商户系统内部订单号，需在同一个商户号下唯一。只能是数字、大小写字母_-*组成
    /// 长度应在 [6, 32] 字符之间
    pub out_trade_no: String,
    /// 订单失效时间
    #[serde(with = "option_datetime_fmt", skip_serializing_if = "Option::is_none")]
    pub time_expire: Option<DateTime<Local>>,
    /// 附加数据，在查询API和支付通知中原样返回，可作为自定义参数使用，实际情况下只有支付完成状态才会返回该字段。
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub attach: Option<String>,
    /// 接收微信支付结果通知的回调地址，通知url必须为外网可访问的url，不能携带参数。
    pub notify_url: String,
    /// 订单优惠标记
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub goods_tag: Option<String>,
    /// 电子发票入口开放标识。
    /// 传入true时，支付成功消息和支付详情页将出现开票入口。需要在微信支付商户平台或微信公众平台开通电子发票功能，传此字段才可生效。
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub support_fapiao: Option<bool>,
    /// 订单金额
    pub amount: Amount,
    /// 支付者
    pub payer: Payer,
    /// 优惠功能
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub detail: Option<CreateTradePromotionDetail>,
    /// 场景信息
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub scene_info: Option<CreateTradeSceneInfo>,
    /// 结算信息
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub settle_info: Option<SettleInfo>,
}

/// JSAPI 下单响应。
#[derive(Debug, Clone, Deserialize)]
struct JsApiCreateTradeResponse {
    prepay_id: String,
}

/// APP 下单响应。
#[derive(Debug, Clone, Deserialize)]
struct AppCreateTradeResponse {
    prepay_id: String,
}

/// H5 下单响应。
#[derive(Debug, Clone, Deserialize)]
struct H5CreateTradeResponse {
    h5_url: String,
}

/// Native 下单响应
#[derive(Debug, Clone, Deserialize)]
struct NativeCreateTradeResponse {
    /// 此URL用于生成支付二维码，然后提供给用户扫码支付。
    code_url: String,
}

/// 订单金额
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Amount {
    /// 订单总金额，单位为分。
    pub total: i32,
    /// 货币类型。CNY：人民币，境内商户号仅支持人民币。
    pub currency: String,
}

/// 订单支付金额
/// total, currency 为下单时的金额信息；
/// payer_total, payer_currency 为用户实际支付的金额信息。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaidAmount {
    /// 订单总金额，单位为分。
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub total: Option<i32>,
    /// 货币类型。CNY：人民币，境内商户号仅支持人民币。
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub currency: Option<String>,

    /// 用户支付金额，单位为分。
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub payer_total: Option<i32>,
    /// 用户支付币种
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub payer_currency: Option<String>,
}

impl Amount {
    /// 以人民币为单位的订单金额(单位: 分)
    pub fn new_with_cny(total: i32) -> Amount {
        Amount {
            total,
            currency: "CNY".to_string(),
        }
    }
}

/// 支付者
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Payer {
    /// 用户在直连商户 app_id 下的唯一标识。下单前需获取到用户的Openid。
    /// Openid 获取详见 <https://pay.weixin.qq.com/wiki/doc/apiv3/terms_definition/chapter1_1_3.shtml#part-3>
    pub openid: Option<String>,
}

impl Payer {
    /// 创建支付者
    pub fn new(openid: String) -> Payer {
        Payer {
            openid: Some(openid),
        }
    }
}

/// 优惠功能
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateTradePromotionDetail {
    /// 订单原价
    /// 1、商户侧一张小票订单可能被分多次支付，订单原价用于记录整张小票的交易金额。
    /// 2、当订单原价与支付金额不相等，则不享受优惠。
    /// 3、该字段主要用于防止同一张小票分多次支付，以享受多次优惠的情况，正常支付订单不必上传此参数。
    pub cost_price: Option<i32>,
    /// 商品小票ID
    pub invoice_id: Option<String>,
    /// 单品列表
    pub goods_detail: Vec<CreateTradeGoodsDetail>,
}

/// 优惠功能
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradePromotionDetail {
    /// 券ID
    pub coupon_id: String,
    /// 优惠名称
    pub name: Option<String>,
    /// 优惠范围
    /// GLOBAL：全场代金券
    /// SINGLE：单品优惠
    pub scope: Option<String>,
    /// 优惠类型
    /// CASH：充值
    /// NOCASH：预充值
    #[serde(rename = "type")]
    pub promotion_type: Option<String>,
    /// 优惠券面额
    pub amount: i32,
    /// 活动ID
    pub stock_id: Option<String>,
    /// 微信出资，单位为分
    pub wechatpay_contribute: Option<i32>,
    /// 商户出资，单位为分
    pub merchant_contribute: Option<i32>,
    /// 其他出资，单位为分
    pub other_contribute: Option<i32>,
    /// 优惠币种。CNY：人民币，境内商户号仅支持人民币。
    pub currency: Option<String>,
    /// 商品列表
    pub goods_detail: Vec<TradeGoodsDetail>,
}

/// 单品信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateTradeGoodsDetail {
    /// 商户侧商品编码。
    /// 由半角的大小写字母、数字、中划线、下划线中的一种或几种组成。
    pub merchant_goods_id: String,
    /// 微信支付定义的统一商品编号（没有可不传）
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub wechatpay_goods_id: Option<String>,
    /// 商品名称
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub goods_name: Option<String>,
    /// 商品数量
    pub quantity: i32,
    /// 商品单价，单位为分。如果商户有优惠，需传输商户优惠后的单价。
    pub unit_price: i32,
}

/// 单品信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeGoodsDetail {
    /// 商品编码
    pub goods_id: String,
    /// 用户购买的商品数量
    pub quantity: i32,
    /// 商品单价，单位为分
    pub unit_price: i32,
    /// 商品优惠金额
    pub discount_amount: i32,
    /// 商品备注信息
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub goods_remark: Option<String>,
}

/// 场景信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateTradeSceneInfo {
    /// 用户的客户端IP，支持IPv4和IPv6两种格式的IP地址。
    pub payer_client_ip: String,
    /// 商户端设备号（门店号或收银设备ID）
    pub device_id: String,
    /// 商户门店信息
    pub store_info: StoreInfo,
}

/// 场景信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeSceneInfo {
    /// 商户端设备号（发起扣款请求的商户服务器设备号）。
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub device_id: Option<String>,
}

/// 商户门店信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoreInfo {
    /// 门店编号
    pub id: String,
    /// 门店名称
    pub name: String,
    /// 地区编码。详见 [省市区编号对照表](https://pay.weixin.qq.com/wiki/doc/apiv3/terms_definition/chapter1_1_3.shtml)
    pub area_code: String,
    /// 详细地址，详细的商户门店地址
    pub address: String,
}

/// 结算信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SettleInfo {
    /// 是否指定分账
    pub profit_sharing: Option<bool>,
}

impl JsApiCreateTradeParams {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        app_id: String,
        mch_id: String,
        description: String,
        out_trade_no: String,
        time_expire: Option<DateTime<Local>>,
        attach: Option<String>,
        notify_url: String,
        amount: Amount,
        payer_openid: String,
    ) -> JsApiCreateTradeParams {
        JsApiCreateTradeParams {
            app_id,
            mch_id,
            description,
            out_trade_no,
            time_expire,
            attach,
            notify_url,
            goods_tag: None,
            support_fapiao: None,
            amount,
            payer: Payer::new(payer_openid),
            detail: None,
            scene_info: None,
            settle_info: None,
        }
    }
}

/// APP 下单参数。
/// 相比 JsApiCreateTradeParams 少了 payer 字段
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppCreateTradeParams {
    /// 应用 ID
    #[serde(rename = "appid")]
    pub app_id: String,
    #[serde(rename = "mchid")]
    /// 商户号
    pub mch_id: String,
    /// 商品描述。不超过 127 字符。
    pub description: String,
    /// 商户订单号。商户系统内部订单号，需在同一个商户号下唯一。只能是数字、大小写字母_-*组成
    /// 长度应在 [6, 32] 字符之间
    pub out_trade_no: String,
    /// 订单失效时间
    #[serde(with = "option_datetime_fmt", skip_serializing_if = "Option::is_none")]
    pub time_expire: Option<DateTime<Local>>,
    /// 附加数据，在查询API和支付通知中原样返回，可作为自定义参数使用，实际情况下只有支付完成状态才会返回该字段。
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub attach: Option<String>,
    /// 接收微信支付结果通知的回调地址，通知url必须为外网可访问的url，不能携带参数。
    pub notify_url: String,
    /// 订单优惠标记
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub goods_tag: Option<String>,
    /// 电子发票入口开放标识。
    /// 传入true时，支付成功消息和支付详情页将出现开票入口。需要在微信支付商户平台或微信公众平台开通电子发票功能，传此字段才可生效。
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub support_fapiao: Option<bool>,
    /// 订单金额
    pub amount: Amount,
    /// 优惠功能
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub detail: Option<CreateTradePromotionDetail>,
    /// 场景信息
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub scene_info: Option<CreateTradeSceneInfo>,
    /// 结算信息
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub settle_info: Option<SettleInfo>,
}

/// H5 下单参数。
/// 相比 JsApiCreateTradeParams 少了 payer 字段
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct H5CreateTradeParams {
    /// 应用 ID
    #[serde(rename = "appid")]
    pub app_id: String,
    #[serde(rename = "mchid")]
    /// 商户号
    pub mch_id: String,
    /// 商品描述。不超过 127 字符。
    pub description: String,
    /// 商户订单号。商户系统内部订单号，需在同一个商户号下唯一。只能是数字、大小写字母_-*组成
    /// 长度应在 [6, 32] 字符之间
    pub out_trade_no: String,
    /// 订单失效时间
    #[serde(with = "option_datetime_fmt", skip_serializing_if = "Option::is_none")]
    pub time_expire: Option<DateTime<Local>>,
    /// 附加数据，在查询API和支付通知中原样返回，可作为自定义参数使用，实际情况下只有支付完成状态才会返回该字段。
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub attach: Option<String>,
    /// 接收微信支付结果通知的回调地址，通知url必须为外网可访问的url，不能携带参数。
    pub notify_url: String,
    /// 订单优惠标记
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub goods_tag: Option<String>,
    /// 电子发票入口开放标识。
    /// 传入true时，支付成功消息和支付详情页将出现开票入口。需要在微信支付商户平台或微信公众平台开通电子发票功能，传此字段才可生效。
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub support_fapiao: Option<bool>,
    /// 订单金额
    pub amount: Amount,
    /// 优惠功能
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub detail: Option<CreateTradePromotionDetail>,
    /// 场景信息
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub scene_info: Option<CreateTradeSceneInfo>,
    /// 结算信息
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub settle_info: Option<SettleInfo>,
}
/// Native 下单参数。
/// 相比 JsApiCreateTradeParams 少了 payer 字段
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NativeCreateTradeParams {
    /// 应用 ID
    #[serde(rename = "appid")]
    pub app_id: String,
    #[serde(rename = "mchid")]
    /// 商户号
    pub mch_id: String,
    /// 商品描述。不超过 127 字符。
    pub description: String,
    /// 商户订单号。商户系统内部订单号，需在同一个商户号下唯一。只能是数字、大小写字母_-*组成
    /// 长度应在 [6, 32] 字符之间
    pub out_trade_no: String,
    /// 订单失效时间
    #[serde(with = "option_datetime_fmt", skip_serializing_if = "Option::is_none")]
    pub time_expire: Option<DateTime<Local>>,
    /// 附加数据，在查询API和支付通知中原样返回，可作为自定义参数使用，实际情况下只有支付完成状态才会返回该字段。
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub attach: Option<String>,
    /// 接收微信支付结果通知的回调地址，通知url必须为外网可访问的url，不能携带参数。
    pub notify_url: String,
    /// 订单优惠标记
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub goods_tag: Option<String>,
    /// 电子发票入口开放标识。
    /// 传入true时，支付成功消息和支付详情页将出现开票入口。需要在微信支付商户平台或微信公众平台开通电子发票功能，传此字段才可生效。
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub support_fapiao: Option<bool>,
    /// 订单金额
    pub amount: Amount,
    /// 优惠功能
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub detail: Option<CreateTradePromotionDetail>,
    /// 场景信息
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub scene_info: Option<CreateTradeSceneInfo>,
    /// 结算信息
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub settle_info: Option<SettleInfo>,
}

/// 订单查询响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeQueryResponse {
    /// 应用 ID
    #[serde(rename = "appid")]
    pub app_id: String,
    /// 商户号
    #[serde(rename = "mchid")]
    pub mch_id: String,
    /// 商户订单号
    pub out_trade_no: String,
    /// 微信支付订单号。不超过 32 字符。
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub transaction_id: Option<String>,
    /// 交易类型
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub trade_type: Option<TradeType>,
    /// 交易状态
    pub trade_state: TradeState,
    /// 交易状态描述
    pub trade_state_desc: String,
    /// 付款银行。取值见 <https://pay.weixin.qq.com/wiki/doc/apiv3/terms_definition/chapter1_1_3.shtml#part-6>
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub bank_type: Option<String>,
    /// 附加数据。
    /// 在查询API和支付通知中原样返回，可作为自定义参数使用，实际情况下只有支付完成状态才会返回该字段。
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub attach: Option<String>,
    /// 支付完成时间。
    #[serde(
        with = "option_datetime_fmt",
        skip_serializing_if = "Option::is_none",
        default
    )]
    pub success_time: Option<DateTime<Local>>,
    /// 支付者
    /// 文档标记此字段为必然返回，应当是文档错误。
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub payer: Option<Payer>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    /// 订单金额信息，当支付成功时返回该字段。
    pub amount: Option<PaidAmount>,
    /// 场景信息，支付场景描述
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub scene_info: Option<TradeSceneInfo>,
    /// 优惠功能，享受优惠时返回该字段
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub promotion_detail: Vec<TradePromotionDetail>,
}

/// 交易类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TradeType {
    JsApi,
    Native,
    App,
    /// 付款码支付
    Micropay,
    /// H5支付
    Mweb,
    /// 刷脸支付
    Facepay,
}
impl TradeType {
    pub fn as_str(&self) -> &'static str {
        match self {
            TradeType::JsApi => "JSAPI",
            TradeType::Native => "NATIVE",
            TradeType::App => "APP",
            TradeType::Micropay => "MICROPAY",
            TradeType::Mweb => "MWEB",
            TradeType::Facepay => "FACEPAY",
        }
    }
}

impl<'de> Deserialize<'de> for TradeType {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?.to_ascii_uppercase();
        match s.as_str() {
            "JSAPI" => Ok(TradeType::JsApi),
            "NATIVE" => Ok(TradeType::Native),
            "APP" => Ok(TradeType::App),
            "MICROPAY" => Ok(TradeType::Micropay),
            "MWEB" => Ok(TradeType::Mweb),
            "FACEPAY" => Ok(TradeType::Facepay),
            _ => Err(serde::de::Error::custom(format!(
                "unknown trade type: {}",
                s
            ))),
        }
    }
}

impl Serialize for TradeType {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let s = self.as_str();
        serializer.serialize_str(s)
    }
}

/// 交易状态
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TradeState {
    /// 支付成功
    Success,
    /// 转入退款
    Refund,
    /// 未支付
    NotPay,
    /// 已关闭
    Closed,
    /// 已撤销（仅付款码支付会返回）
    Revoked,
    /// 用户支付中（仅付款码支付会返回）
    UserPaying,
    /// 支付失败（仅付款码支付会返回）
    PayError,
}

impl TradeState {
    pub fn as_str(&self) -> &'static str {
        match self {
            TradeState::Success => "SUCCESS",
            TradeState::Refund => "REFUND",
            TradeState::NotPay => "NOTPAY",
            TradeState::Closed => "CLOSED",
            TradeState::Revoked => "REVOKED",
            TradeState::UserPaying => "USERPAYING",
            TradeState::PayError => "PAYERROR",
        }
    }
}

impl<'de> Deserialize<'de> for TradeState {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?.to_ascii_uppercase();
        match s.as_str() {
            "SUCCESS" => Ok(TradeState::Success),
            "REFUND" => Ok(TradeState::Refund),
            "NOTPAY" => Ok(TradeState::NotPay),
            "CLOSED" => Ok(TradeState::Closed),
            "REVOKED" => Ok(TradeState::Revoked),
            "USERPAYING" => Ok(TradeState::UserPaying),
            "PAYERROR" => Ok(TradeState::PayError),
            _ => Err(serde::de::Error::custom(format!(
                "unknown trade state: {}",
                s
            ))),
        }
    }
}

impl Serialize for TradeState {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let s = self.as_str();
        serializer.serialize_str(s)
    }
}

/// 生成商户订单号，长度为 27 字符。形如:
/// mca8fa-nua7q2-adaf8a-ada8fa
/// 每 6 个字符一组，以 `-` 连接。
pub fn generate_out_trade_no() -> String {
    const ALPHABET: &[u8] = b"abcdefghjkmnpqrstuvwxyzABCDEFGHJKLMNPQRSTRVWXYZ23456789";
    let mut rng = rand::thread_rng();

    let mut s = String::new();
    for i in 1..=24 {
        let idx = rng.gen_range(0..ALPHABET.len());
        s.push(ALPHABET[idx] as char);
        if i % 6 == 0 && i != 24 {
            s.push('-');
        }
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trade_type_serde() -> anyhow::Result<()> {
        #[derive(Debug, Serialize, Deserialize)]
        struct Wrapper {
            tt: TradeType,
        }
        let w = Wrapper {
            tt: TradeType::JsApi,
        };
        let s = serde_json::to_string(&w)?;
        assert_eq!(s, r#"{"tt":"JSAPI"}"#);

        let w2: Wrapper = serde_json::from_str(r#"{"tt":"NATIVE"}"#)?;
        assert_eq!(w2.tt, TradeType::Native);
        Ok(())
    }

    #[test]
    fn test_trade_state_serde() -> anyhow::Result<()> {
        #[derive(Debug, Serialize, Deserialize)]
        struct Wrapper {
            ts: TradeState,
        }

        let w = Wrapper {
            ts: TradeState::Success,
        };
        let s = serde_json::to_string(&w)?;
        assert_eq!(s, r#"{"ts":"SUCCESS"}"#);

        let w2: Wrapper = serde_json::from_str(r#"{"ts":"NOTPAY"}"#)?;
        assert_eq!(w2.ts, TradeState::NotPay);
        Ok(())
    }

    #[test]
    fn test_generate_out_trade_no() {
        let s = generate_out_trade_no();
        assert_eq!(s.len(), 27);
        assert_eq!(s.chars().nth(6), Some('-'));
        assert_eq!(s.chars().nth(13), Some('-'));
        assert_eq!(s.chars().nth(20), Some('-'));
    }
}
