use std::{collections::HashMap, vec};

use anyhow::Context;
use base64::{engine::general_purpose, Engine as _};
use config::get;
use extism_pdk::*;

mod types;
use spansy::{json::parse_str, Spanned};
use types::{PluginConfig, RequestConfig, RequestObject, StepConfig};
mod host_functions;
use host_functions::{notarize, redirect};
mod utils;
use url::Url;
use utils::{get_cookies_by_host, get_headers_by_host};

const SETTINGS_REQUEST: RequestObject = RequestObject {
    url: "https://api.ethglobal.com/graphql",
    method: "POST",
};

#[plugin_fn]
pub fn config() -> FnResult<Json<PluginConfig<'static>>> {
    let icon: String = format!(
        "data:image/png;base64,{}",
        general_purpose::STANDARD.encode(include_bytes!("../assets/icon.png"))
    );

    let config = PluginConfig {
        title: "ETHGlobal Submission (Rust)",
        description: "Notarize submission status of a project in ETHGlobal hackathon",
        steps: vec![
            StepConfig {
                title: "Visit ETHGlobal website",
                description: None,
                cta: "Go to ethglobal.com",
                action: "start",
                prover: false,
            },
            StepConfig {
                title: "Collect credentials",
                cta: "Check cookies",
                action: "two",
                prover: false,
                description: Some("Login to your account if you haven't already"),
            },
            StepConfig {
                title: "Notarize submission status",
                cta: "Notarize",
                action: "three",
                prover: true,
                description: None,
            },
        ],
        host_functions: vec!["redirect", "notarize"],
        cookies: vec!["api.ethglobal.com"],
        headers: vec!["api.ethglobal.com"],
        requests: vec![SETTINGS_REQUEST],
        notary_urls: None,
        proxy_urls: None,
        icon,
    };
    Ok(Json(config))
}

/// Implementation of the first (start) plugin step
#[plugin_fn]
pub fn start() -> FnResult<Json<bool>> {
    let ethglobal_url = Url::parse("https://ethglobal.com/events/bangkok/project")?;
    let tab_url = get("tabUrl")?.context("Error getting tab url")?;
    let tab_url = Url::parse(&tab_url)?;

    if tab_url.host_str() != ethglobal_url.host_str() {
        unsafe {
            let _ = redirect(x_url.as_str());
        };
        return Ok(Json(false));
    }

    Ok(Json(true))
}

/// Implementation of step "two".
/// This step collects and validates authentication cookies and headers for 'api.ethglobal.com'.
/// If all required information, it creates the request object.
/// Note that the url needs to be specified in the `config` too, otherwise the request will be refused.
#[plugin_fn]
pub fn two() -> FnResult<Json<RequestConfig>> {
    let cookies = get_cookies_by_host("api.ethglobal.com")?;
    let headers = get_headers_by_host("api.ethglobal.com")?;

    log!(LogLevel::Info, "cookies: {cookies:?}");
    log!(LogLevel::Info, "headers: {headers:?}");

    let auth_token = cookies
        .get("ethglobal_access_token")
        .ok_or_else(|| Error::msg("ethglobal_access_token cookie not found"))?;

    let cookie = format!("lang=en; auth_token={auth_token};");
    let headers: HashMap<String, String> = [
        (String::from("host"), String::from("api.ethglobal.com")),
        (String::from("Cookie"), cookie.clone()),
        (String::from("Accept-Encoding"), String::from("identity")),
        (String::from("Connection"), String::from("close")),
    ]
    .into_iter()
    .collect();
    let secret_headers = vec![x_csrf_token.clone(), cookie, authorization.clone()];
    let request = RequestConfig {
        url: SETTINGS_REQUEST.url.to_string(),
        method: SETTINGS_REQUEST.method.to_string(),
        headers,
        secret_headers,
        get_secret_response: Some(String::from("redact")),
    };

    let request_json = serde_json::to_string(&request)?;
    log!(LogLevel::Info, "request: {:?}", &request_json);

    return Ok(Json(request));
}

/**
 * Step 3: calls the `notarize` host function
 */
#[plugin_fn]
pub fn three() -> FnResult<Json<String>> {
    let request_json: String = input()?;
    log!(LogLevel::Info, "Input: {request_json:?}");

    let id = unsafe {
        let id = notarize(&request_json);
        log!(LogLevel::Info, "Notarization result: {:?}", id);
        id?
    };

    return Ok(Json(id));
}

/// This method is used to parse the ETHGlobal response and specify what information is revealed (i.e. **not** redacted)
/// This method is optional in the notarization request. When it is not specified nothing is redacted.
///
/// In this example it locates the `screen_name` and excludes that range from the revealed response.
/// TODO adapt it for ETHGlobal submission status
#[plugin_fn]
pub fn redact() -> FnResult<Json<Vec<String>>> {
    let body_string: String = input()?;

    // let spansy = parse_str(&body_string)?;
    // log!(LogLevel::Info, "spansy: {spansy:?}");
    // let screen_name = spansy
    //     .get("screen_name")
    //     .context("Missing \"screen_name\" in response")?;
    // log!(LogLevel::Info, "screen_name: {:?}", screen_name);

    // let screen_name_start = screen_name.span().indices().iter().next().context("foo")?;
    // let screen_name_end = screen_name.span().indices().end().context("foo")?;

    // let secret_resps = vec![
    //     body_string[0..screen_name_start - ("\"screen_name\":\"".len())].to_string(),
    //     body_string[screen_name_end + 1..body_string.len()].to_string(),
    // ];

    // Simply return the input as a single-element vector
    let secret_resps = vec![body_string];

    Ok(Json(secret_resps))
}
