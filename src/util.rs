/// 日期时间格式，形如 `2018-06-08T10:34:56+08:00`。
pub const DATETIME_FORMAT: &str = "%Y-%m-%dT%H:%M:%S%:z";

/// 根据 DATETIME_FORMAT 格式序列化/反序列化日期时间。
pub mod datetime_fmt {
    use super::DATETIME_FORMAT;
    use chrono::{DateTime, FixedOffset, Local};
    use serde::{Deserialize, Deserializer, Serializer};

    /// 根据 DATETIME_FORMAT 格式解析日期时间字符串。形如 `2018-06-08T10:34:56+08:00`。
    pub fn deserialize<'de, D>(deserializer: D) -> Result<DateTime<Local>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        let dt = DateTime::<FixedOffset>::parse_from_str(&s, DATETIME_FORMAT)
            .map_err(serde::de::Error::custom)?;
        Ok(dt.with_timezone(&Local))
    }

    /// 根据 DATETIME_FORMAT 格式格式化日期时间字符串。形如 `2018-06-08T10:34:56+08:00`。
    pub fn serialize<S>(dt: &DateTime<Local>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let s = format!("{}", dt.format(DATETIME_FORMAT));
        serializer.serialize_str(&s)
    }
}

/// 根据 DATETIME_FORMAT 格式序列化/反序列化日期时间，只是针对 Option 类型。
pub mod option_datetime_fmt {
    use super::datetime_fmt::deserialize as deserialize_datetime;
    use super::datetime_fmt::serialize as serialize_datetime;
    use chrono::{DateTime, Local};
    use serde::{Deserialize, Deserializer, Serializer};

    /// 根据 DATETIME_FORMAT 格式解析日期时间字符串。形如 `2018-06-08T10:34:56+08:00`。
    /// 此实现来自 <https://github.com/serde-rs/serde/issues/1444#issuecomment-447546415>
    pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<DateTime<Local>>, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct Wrapper(#[serde(deserialize_with = "deserialize_datetime")] DateTime<Local>);

        let v = Option::deserialize(deserializer)?;
        Ok(v.map(|Wrapper(a)| a))
    }

    /// 根据 DATETIME_FORMAT 格式格式化日期时间字符串。形如 `2018-06-08T10:34:56+08:00`。
    pub fn serialize<S>(dt: &Option<DateTime<Local>>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match dt {
            Some(dt) => serialize_datetime(dt, serializer),
            None => serializer.serialize_none(),
        }
    }
}
