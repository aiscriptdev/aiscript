use chrono::Datelike;
use regex::Regex;
use serde_json::Value;
use std::any::Any;
use std::sync::LazyLock;

use crate::{Directive, DirectiveParams, FromDirective};

use super::Validator;

static EMAIL_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}$").unwrap());

static URL_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^https?://[\w.-]+(:\d+)?(/[\w/.~:%-]+)*/?(\?\S*)?$").unwrap());

static UUID_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$").unwrap()
});

static IPV4_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"^((25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?)\.){3}(25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?)$",
    )
    .unwrap()
});

static IPV6_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(([0-9a-fA-F]{1,4}:){7}[0-9a-fA-F]{1,4}|([0-9a-fA-F]{1,4}:){1,7}:|([0-9a-fA-F]{1,4}:){1,6}:[0-9a-fA-F]{1,4}|([0-9a-fA-F]{1,4}:){1,5}(:[0-9a-fA-F]{1,4}){1,2}|([0-9a-fA-F]{1,4}:){1,4}(:[0-9a-fA-F]{1,4}){1,3}|([0-9a-fA-F]{1,4}:){1,3}(:[0-9a-fA-F]{1,4}){1,4}|([0-9a-fA-F]{1,4}:){1,2}(:[0-9a-fA-F]{1,4}){1,5}|[0-9a-fA-F]{1,4}:((:[0-9a-fA-F]{1,4}){1,6})|:((:[0-9a-fA-F]{1,4}){1,7}|:)|fe80:(:[0-9a-fA-F]{0,4}){0,4}%[0-9a-zA-Z]+|::(ffff(:0{1,4})?:)?((25[0-5]|(2[0-4]|1?[0-9])?[0-9])\.){3}(25[0-5]|(2[0-4]|1?[0-9])?[0-9])|([0-9a-fA-F]{1,4}:){1,4}:((25[0-5]|(2[0-4]|1?[0-9])?[0-9])\.){3}(25[0-5]|(2[0-4]|1?[0-9])?[0-9]))$").unwrap()
});

static DATE_REGEX: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\d{4}-\d{2}-\d{2}$").unwrap());

static DATETIME_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(\.\d+)?(Z|[+-]\d{2}:\d{2})?$").unwrap()
});

static TIME_REGEX: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\d{2}:\d{2}:\d{2}$").unwrap());

static MONTH_REGEX: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\d{4}-\d{2}$").unwrap());

static WEEK_REGEX: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\d{4}-W\d{2}$").unwrap());

static COLOR_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^#([0-9a-fA-F]{3}|[0-9a-fA-F]{6})$").unwrap());

mod uscc {
    use std::{collections::HashMap, sync::LazyLock};

    use regex::Regex;

    static USCC_REGEX: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"^[0-9A-HJ-NPQRTUWXY]{2}\d{6}[0-9A-HJ-NPQRTUWXY]{10}$").unwrap()
    });

    static USCC_BASE_CHARS: LazyLock<HashMap<char, u8>> = LazyLock::new(|| {
        let mut base_chars = HashMap::with_capacity(17);

        [
            '0', '1', '2', '3', '4', '5', '6', '7', '8', '9', 'A', 'B', 'C', 'D', 'E', 'F', 'G',
            'H', 'J', 'K', 'L', 'M', 'N', 'P', 'Q', 'R', 'T', 'U', 'W', 'X', 'Y',
        ]
        .into_iter()
        .enumerate()
        .for_each(|(index, c)| {
            base_chars.insert(c, index as u8);
        });

        base_chars
    });

    static USCC_WEIGHT: LazyLock<[u8; 17]> = LazyLock::new(|| {
        [
            1, 3, 9, 27, 19, 26, 16, 17, 20, 29, 25, 13, 8, 24, 10, 30, 28,
        ]
    });

    /// Determine whether it is a valid [Unified Social Credit Code](http://c.gb688.cn/bzgk/gb/showGb?type=online&hcno=24691C25985C1073D3A7C85629378AC0).
    pub fn is_valid_unified_social_credit_code(uscc: &str) -> bool {
        if USCC_REGEX.is_match(uscc) {
            let total_weight = uscc
                .chars()
                .take(17)
                .enumerate()
                .map(|(index, ref c)| {
                    // The previously executed regular expression match ensures that the value retrieval operation here is safe.
                    *USCC_BASE_CHARS.get(c).unwrap() as usize * USCC_WEIGHT[index] as usize
                })
                .sum::<usize>();

            let check_flag = ((31 - (total_weight % 31)) % 31) as u8;

            match USCC_BASE_CHARS.iter().find(|(_, v)| **v == check_flag) {
                Some((&flag, _)) => uscc.chars().last().unwrap() == flag,
                _ => false,
            }
        } else {
            false
        }
    }
}

pub struct FormatValidator {
    pub format_type: String,
}

// Improve the validate method for these formats
impl Validator for FormatValidator {
    fn name(&self) -> &'static str {
        "@format"
    }

    fn validate(&self, value: &Value) -> Result<(), String> {
        let value_str = match value.as_str() {
            Some(s) => s,
            None => return Err("Value must be a string".into()),
        };

        match self.format_type.as_str() {
            // Other formats remain unchanged
            "time" => {
                if !TIME_REGEX.is_match(value_str) {
                    return Err("Value doesn't match time format (HH:MM:SS)".into());
                }

                // Validate time components
                let parts: Vec<&str> = value_str.split(':').collect();
                if parts.len() != 3 {
                    return Err("Time must have hours, minutes, and seconds".into());
                }

                let hours: u32 = parts[0].parse().map_err(|_| "Invalid hours")?;
                let minutes: u32 = parts[1].parse().map_err(|_| "Invalid minutes")?;
                let seconds: u32 = parts[2].parse().map_err(|_| "Invalid seconds")?;

                if hours >= 24 || minutes >= 60 || seconds >= 60 {
                    return Err("Invalid time components".into());
                }

                Ok(())
            }
            "month" => {
                if !MONTH_REGEX.is_match(value_str) {
                    return Err("Value doesn't match month format (YYYY-MM)".into());
                }

                // Validate month components
                let parts: Vec<&str> = value_str.split('-').collect();
                if parts.len() != 2 {
                    return Err("Month must have year and month parts".into());
                }

                let month: u32 = parts[1].parse().map_err(|_| "Invalid month")?;

                if !(1..=12).contains(&month) {
                    return Err("Month must be between 1 and 12".into());
                }

                Ok(())
            }
            "week" => {
                if !WEEK_REGEX.is_match(value_str) {
                    return Err("Value doesn't match week format (YYYY-Www)".into());
                }

                // Validate week components
                let year_part = &value_str[0..4];
                let week_part = &value_str[6..8];

                let year: i32 = year_part.parse().map_err(|_| "Invalid year")?;
                let week: u32 = week_part.parse().map_err(|_| "Invalid week")?;

                // ISO 8601 defines the valid range for week numbers is 1-53,
                // but only certain years have a 53rd week
                if !(1..=52).contains(&week) {
                    // Week 53 is only valid in years where January 1 is a Thursday
                    // or in leap years where January 1 is a Wednesday
                    if week == 53 {
                        // Calculate if this year has 53 weeks
                        let has_week_53 = chrono::NaiveDate::from_ymd_opt(year, 1, 1)
                            .map(|date| {
                                let weekday = date.weekday().num_days_from_monday();
                                weekday == 3
                                    || (weekday == 2
                                        && chrono::NaiveDate::from_ymd_opt(year, 2, 29).is_some())
                            })
                            .unwrap_or(false);

                        if !has_week_53 {
                            return Err("This year doesn't have a week 53".into());
                        }
                    } else {
                        return Err(
                            "Week must be between 1 and 52 (or 53 for certain years)".into()
                        );
                    }
                }

                Ok(())
            }
            // Handle other formats here as before
            _ => {
                // Original validation for other formats...
                let valid = match self.format_type.as_str() {
                    "email" => EMAIL_REGEX.is_match(value_str),
                    "url" => URL_REGEX.is_match(value_str),
                    "uuid" => UUID_REGEX.is_match(value_str),
                    "ipv4" => IPV4_REGEX.is_match(value_str),
                    "ipv6" => IPV6_REGEX.is_match(value_str),
                    "date" => {
                        if DATE_REGEX.is_match(value_str) {
                            if let Ok(date) =
                                chrono::NaiveDate::parse_from_str(value_str, "%Y-%m-%d")
                            {
                                let year = date.year();
                                let month = date.month();
                                let day = date.day();
                                year >= 1 && (1..=12).contains(&month) && (1..=31).contains(&day)
                            } else {
                                false
                            }
                        } else {
                            false
                        }
                    }
                    "datetime" => {
                        DATETIME_REGEX.is_match(value_str)
                            && chrono::DateTime::parse_from_rfc3339(value_str).is_ok()
                    }
                    "color" => COLOR_REGEX.is_match(value_str),
                    "uscc" => uscc::is_valid_unified_social_credit_code(value_str),
                    _ => return Err(format!("Unsupported format type: {}", self.format_type)),
                };

                if valid {
                    Ok(())
                } else {
                    Err(format!("Value doesn't match {} format", self.format_type))
                }
            }
        }
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}
// The FromDirective implementation remains the same
impl FromDirective for FormatValidator {
    fn from_directive(directive: Directive) -> Result<Self, String> {
        // Same implementation as before
        match directive.params {
            DirectiveParams::KeyValue(params) => {
                match params.get("type").and_then(|v| v.as_str()) {
                    Some(format_type) => match format_type {
                        "email" | "url" | "uuid" | "ipv4" | "ipv6" | "date" | "datetime"
                        | "time" | "month" | "week" | "color" | "uscc" => Ok(Self {
                            format_type: format_type.to_string(),
                        }),
                        _ => Err(format!("Unsupported format type: {}", format_type)),
                    },
                    None => Err("@format directive requires a 'type' parameter".into()),
                }
            }
            _ => Err("Invalid params for @format directive".into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Directive, DirectiveParams};
    use serde_json::json;
    use std::collections::HashMap;

    fn create_directive(params: HashMap<String, Value>) -> Directive {
        Directive {
            name: "format".into(),
            params: DirectiveParams::KeyValue(params),
            line: 1,
        }
    }

    #[test]
    fn test_email_format() {
        let mut params = HashMap::new();
        params.insert("type".into(), json!("email"));
        let directive = create_directive(params);
        let validator = FormatValidator::from_directive(directive).unwrap();

        assert!(validator.validate(&json!("user@example.com")).is_ok());
        assert!(
            validator
                .validate(&json!("user.name+tag@example.co.uk"))
                .is_ok()
        );
        assert!(validator.validate(&json!("invalid-email")).is_err());
        assert!(validator.validate(&json!("missing@domain")).is_err());
        assert!(validator.validate(&json!("@example.com")).is_err());
    }

    #[test]
    fn test_url_format() {
        let mut params = HashMap::new();
        params.insert("type".into(), json!("url"));
        let directive = create_directive(params);
        let validator = FormatValidator::from_directive(directive).unwrap();

        assert!(validator.validate(&json!("http://example.com")).is_ok());
        assert!(
            validator
                .validate(&json!("https://subdomain.example.com/path"))
                .is_ok()
        );
        assert!(
            validator
                .validate(&json!("https://example.com/path?query=value"))
                .is_ok()
        );
        assert!(validator.validate(&json!("example.com")).is_err());
        assert!(validator.validate(&json!("http://")).is_err());
    }

    #[test]
    fn test_uuid_format() {
        let mut params = HashMap::new();
        params.insert("type".into(), json!("uuid"));
        let directive = create_directive(params);
        let validator = FormatValidator::from_directive(directive).unwrap();

        assert!(
            validator
                .validate(&json!("123e4567-e89b-12d3-a456-426614174000"))
                .is_ok()
        );
        assert!(
            validator
                .validate(&json!("123e4567-e89b-12d3-a456-42661417400"))
                .is_err()
        ); // too short
        assert!(
            validator
                .validate(&json!("123e4567-e89b-12d3-a456-4266141740000"))
                .is_err()
        ); // too long
        assert!(
            validator
                .validate(&json!("123e4567e89b12d3a456426614174000"))
                .is_err()
        ); // no hyphens
    }

    #[test]
    fn test_ipv4_format() {
        let mut params = HashMap::new();
        params.insert("type".into(), json!("ipv4"));
        let directive = create_directive(params);
        let validator = FormatValidator::from_directive(directive).unwrap();

        assert!(validator.validate(&json!("192.168.0.1")).is_ok());
        assert!(validator.validate(&json!("127.0.0.1")).is_ok());
        assert!(validator.validate(&json!("255.255.255.255")).is_ok());
        assert!(validator.validate(&json!("256.0.0.1")).is_err()); // out of range
        assert!(validator.validate(&json!("192.168.0")).is_err()); // too few octets
        assert!(validator.validate(&json!("192.168.0.1.5")).is_err()); // too many octets
    }

    #[test]
    fn test_ipv6_format() {
        let mut params = HashMap::new();
        params.insert("type".into(), json!("ipv6"));
        let directive = create_directive(params);
        let validator = FormatValidator::from_directive(directive).unwrap();

        assert!(
            validator
                .validate(&json!("2001:0db8:85a3:0000:0000:8a2e:0370:7334"))
                .is_ok()
        );
        assert!(validator.validate(&json!("::1")).is_ok()); // localhost
        assert!(validator.validate(&json!("2001:db8::")).is_ok()); // with ::
        assert!(validator.validate(&json!("192.168.0.1")).is_err()); // IPv4
        assert!(
            validator
                .validate(&json!("2001:db8:85a3:0000:0000:8a2e:0370:7334:1234"))
                .is_err()
        ); // too long
    }

    #[test]
    fn test_date_format() {
        let mut params = HashMap::new();
        params.insert("type".into(), json!("date"));
        let directive = create_directive(params);
        let validator = FormatValidator::from_directive(directive).unwrap();

        assert!(validator.validate(&json!("2023-01-15")).is_ok());
        assert!(validator.validate(&json!("2023-02-28")).is_ok());
        assert!(validator.validate(&json!("2023-02-30")).is_err()); // invalid day
        assert!(validator.validate(&json!("2023-13-01")).is_err()); // invalid month
        assert!(validator.validate(&json!("01-15-2023")).is_err()); // wrong format
        assert!(validator.validate(&json!("2023/01/15")).is_err()); // wrong separator
    }

    #[test]
    fn test_datetime_format() {
        let mut params = HashMap::new();
        params.insert("type".into(), json!("datetime"));
        let directive = create_directive(params);
        let validator = FormatValidator::from_directive(directive).unwrap();

        assert!(validator.validate(&json!("2023-01-15T12:30:45Z")).is_ok());
        assert!(
            validator
                .validate(&json!("2023-01-15T12:30:45+01:00"))
                .is_ok()
        );
        assert!(
            validator
                .validate(&json!("2023-01-15T12:30:45.123Z"))
                .is_ok()
        );
        assert!(validator.validate(&json!("2023-01-15 12:30:45")).is_err()); // missing T
        assert!(validator.validate(&json!("2023-01-15T25:30:45Z")).is_err()); // invalid hour
    }

    #[test]
    fn test_time_format() {
        let mut params = HashMap::new();
        params.insert("type".into(), json!("time"));
        let directive = create_directive(params);
        let validator = FormatValidator::from_directive(directive).unwrap();

        assert!(validator.validate(&json!("12:30:45")).is_ok());
        assert!(validator.validate(&json!("00:00:00")).is_ok());
        assert!(validator.validate(&json!("23:59:59")).is_ok());
        assert!(validator.validate(&json!("24:00:00")).is_err()); // invalid hour
        assert!(validator.validate(&json!("12:60:45")).is_err()); // invalid minute
        assert!(validator.validate(&json!("12:30")).is_err()); // missing seconds
    }

    #[test]
    fn test_month_format() {
        let mut params = HashMap::new();
        params.insert("type".into(), json!("month"));
        let directive = create_directive(params);
        let validator = FormatValidator::from_directive(directive).unwrap();

        assert!(validator.validate(&json!("2023-01")).is_ok());
        assert!(validator.validate(&json!("2023-12")).is_ok());
        assert!(validator.validate(&json!("2023-13")).is_err()); // invalid month
        assert!(validator.validate(&json!("01-2023")).is_err()); // wrong format
    }

    #[test]
    fn test_week_format() {
        let mut params = HashMap::new();
        params.insert("type".into(), json!("week"));
        let directive = create_directive(params);
        let validator = FormatValidator::from_directive(directive).unwrap();

        assert!(validator.validate(&json!("2023-W01")).is_ok());
        assert!(validator.validate(&json!("2023-W52")).is_ok());
        assert!(validator.validate(&json!("2023-W00")).is_err()); // invalid week
        assert!(validator.validate(&json!("2023-W53")).is_err()); // invalid week
        assert!(validator.validate(&json!("2023W01")).is_err()); // missing hyphen
    }

    #[test]
    fn test_color_format() {
        let mut params = HashMap::new();
        params.insert("type".into(), json!("color"));
        let directive = create_directive(params);
        let validator = FormatValidator::from_directive(directive).unwrap();

        assert!(validator.validate(&json!("#000000")).is_ok());
        assert!(validator.validate(&json!("#FFFFFF")).is_ok());
        assert!(validator.validate(&json!("#123")).is_ok());
        assert!(validator.validate(&json!("#1234")).is_err()); // invalid length
        assert!(validator.validate(&json!("000000")).is_err()); // missing #
        assert!(validator.validate(&json!("#GHIJKL")).is_err()); // invalid hex
    }

    #[test]
    fn test_invalid_format_type() {
        let mut params = HashMap::new();
        params.insert("type".into(), json!("invalid_type"));
        let directive = create_directive(params);
        assert!(FormatValidator::from_directive(directive).is_err());
    }

    #[test]
    fn test_missing_type_parameter() {
        let params = HashMap::new();
        let directive = create_directive(params);
        assert!(FormatValidator::from_directive(directive).is_err());
    }

    #[test]
    fn test_non_string_value() {
        let mut params = HashMap::new();
        params.insert("type".into(), json!("email"));
        let directive = create_directive(params);
        let validator = FormatValidator::from_directive(directive).unwrap();

        assert!(validator.validate(&json!(123)).is_err());
        assert!(validator.validate(&json!(true)).is_err());
        assert!(validator.validate(&json!(null)).is_err());
        assert!(validator.validate(&json!(["email@example.com"])).is_err());
    }

    #[test]
    fn test_uscc_format() {
        let mut params = HashMap::new();
        params.insert("type".into(), json!("uscc"));
        let directive = create_directive(params);
        let validator = FormatValidator::from_directive(directive).unwrap();

        assert!(validator.validate(&json!("91440300MA5FXT4K8N")).is_ok());
        assert!(validator.validate(&json!("91110108660511594M")).is_ok());
        assert!(validator.validate(&json!("91330110MA2AXY0E7F")).is_ok());
        assert!(validator.validate(&json!("91330100716105852F")).is_ok());
        assert!(validator.validate(&json!("911101085923662400")).is_ok());
        assert!(validator.validate(&json!("911101085923662401")).is_err()); // The value of the check digit (the last character) is incorrect.
        assert!(validator.validate(&json!("91110108592366240")).is_err()); // invalid length
        assert!(validator.validate(&json!("9111010859236624001")).is_err()); // invalid length
    }
}
