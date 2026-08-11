use serde::Deserialize;

pub(crate) fn string_from_any_or_default<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = serde_json::Value::deserialize(deserializer)?;
    Ok(string_from_value(value))
}

pub(crate) fn optional_string_from_any<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = serde_json::Value::deserialize(deserializer)?;
    let value = string_from_value(value).trim().to_string();
    Ok((!value.is_empty()).then_some(value))
}

pub(crate) fn lossy_string_vec_from_array_or_scalar<'de, D>(
    deserializer: D,
) -> Result<Vec<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = serde_json::Value::deserialize(deserializer)?;

    match value {
        serde_json::Value::Array(items) => Ok(items
            .into_iter()
            .filter_map(scalar_string)
            .filter(|value| !value.trim().is_empty())
            .collect()),
        serde_json::Value::String(value) => {
            if value.trim().is_empty() {
                Ok(Vec::new())
            } else {
                Ok(vec![value])
            }
        }
        serde_json::Value::Number(value) => Ok(vec![value.to_string()]),
        serde_json::Value::Bool(value) => Ok(vec![value.to_string()]),
        serde_json::Value::Null => Ok(Vec::new()),
        _ => Err(serde::de::Error::custom(
            "expected a string array, scalar, or empty value",
        )),
    }
}

pub(crate) fn string_from_any<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = serde_json::Value::deserialize(deserializer)?;
    scalar_string(value).ok_or_else(|| serde::de::Error::custom("expected a scalar value"))
}

pub(crate) fn u32_from_any<'de, D>(deserializer: D) -> Result<u32, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = serde_json::Value::deserialize(deserializer)?;
    Ok(value
        .as_u64()
        .map(|value| value as u32)
        .or_else(|| value.as_str()?.parse().ok())
        .or_else(|| value.as_bool().map(u32::from))
        .unwrap_or_default())
}

pub(crate) fn optional_positive_i64_from_any<'de, D>(
    deserializer: D,
) -> Result<Option<i64>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = serde_json::Value::deserialize(deserializer)?;
    let parsed = match value {
        serde_json::Value::Number(value) => value
            .as_i64()
            .or_else(|| value.as_u64().and_then(|value| i64::try_from(value).ok())),
        serde_json::Value::String(value) => value.trim().parse::<i64>().ok(),
        _ => None,
    };
    Ok(parsed.filter(|value| *value > 0))
}

pub(crate) fn bool_from_any<'de, D>(deserializer: D) -> Result<bool, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = serde_json::Value::deserialize(deserializer)?;
    Ok(match value {
        serde_json::Value::Bool(value) => value,
        serde_json::Value::Number(value) => value.as_i64().unwrap_or_default() != 0,
        serde_json::Value::String(value) => {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes"
            )
        }
        _ => false,
    })
}

fn scalar_string(value: serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::String(value) => Some(value),
        serde_json::Value::Number(value) => Some(value.to_string()),
        serde_json::Value::Bool(value) => Some(value.to_string()),
        serde_json::Value::Null => Some(String::new()),
        _ => None,
    }
}

fn string_from_value(value: serde_json::Value) -> String {
    scalar_string(value).unwrap_or_default()
}
