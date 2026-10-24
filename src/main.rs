use core::panic;
use std::{sync::Mutex, thread, time::Duration};

use fancy_regex::Regex;
use log::{error, info};
use rayon::prelude::*;
use reqwest::header::{HeaderMap, HeaderValue};
use serde_json::Value;

fn handle_response(dat: String, code: &str) {
    let Ok(json) = serde_json::from_str::<Value>(&dat) else {
        error!("unable to parse json from hoyoverse data: \n {}", dat);
        return;
    };

    let Some(retcode) = json.pointer("/retcode") else {
        error!("failed to retrieve retcode from json");
        return;
    };

    let Some(message) = json.pointer("/message") else {
        error!("failed to retrieve retcode from json");
        return;
    };

    let Some(retcode) = retcode.as_i64() else {
        error!("failed to retrieve numeric retcode from json");
        return;
    };

    let Some(message) = message.as_str() else {
        error!("failed to retrieve numeric retcode from json");
        return;
    };

    if retcode == 0 {
        info!("code {code} has been redeemed successfully");
    } else {
        error!("failed to redeem {code} with error {}", message);
    }
}

#[tokio::main]
async fn main() {
    unsafe { std::env::set_var("RUST_LOG", "info") };
    env_logger::init();
    info!("Genshin Redemption Helper Starts.");

    let cookie = std::env::var("COOKIE").unwrap();
    let os = std::env::var("OS").unwrap();

    match os.as_str() {
        "os_asia" | "os_usa" | "os_euro" | "os_cht" => {}
        _ => panic!("invalid OS"),
    };

    let mut headers = HeaderMap::new();
    headers.insert("User-Agent", HeaderValue::from_static("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/27.0 Safari/605.1.15"));

    let client = reqwest::ClientBuilder::new()
        .default_headers(headers)
        .build()
        .expect("failed to construct client");
    let redeem_code_match =
        Regex::new(r#"(?<=https://genshin.hoyoverse.com/gift\?code=).*?(?=")"#).unwrap();
    let redeem_codes = Mutex::new(Vec::<String>::new());

    // get redeem code
    client
        .get("https://t.me/s/genshin_impact_codes")
        .send()
        .await
        .expect("failed to send request to get redemption code")
        .text()
        .await
        .unwrap()
        .par_lines()
        .for_each(|line| {
            if line.contains("New code") && line.contains('🟩') {
                let code = redeem_code_match.find(line).unwrap();
                if let Some(code) = code {
                    redeem_codes.lock().unwrap().push(code.as_str().to_string());
                }
            }
        });

    let uid = serde_json::from_str::<Value>(&client.get(format!("https://api-account-os.hoyoverse.com/account/binding/api/getUserGameRolesByCookieToken?lang=en&region={}&game_biz=hk4e_global&sLangKey=en-us", os))
        .header("COOKIE", HeaderValue::from_str(&cookie).unwrap())
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap()).unwrap();

    let uid = uid
        .pointer("/data/list/0/game_uid")
        .unwrap()
        .as_str()
        .unwrap();

    {
        let codes = redeem_codes.lock().unwrap();
        for i in codes.iter() {
            let url = format!(
                "https://public-operation-hk4e.hoyoverse.com/common/apicdkey/api/webExchangeCdkey?uid={}&region={}&lang=en&cdkey={}&game_biz=hk4e_global&sLangKey=en-us",
                uid, os, i
            );
            let status = client
                .get(url)
                .header("COOKIE", HeaderValue::from_str(&cookie).unwrap())
                .send()
                .await;

            let text = if let Ok(status) = status {
                Ok(status.text().await)
            } else {
                Err(status)
            };

            if let Ok(Ok(resp)) = text {
                handle_response(resp, i);
            } else {
                error!("unable to redeem code {} with error {:?}", i, text);
            }

            thread::sleep(Duration::from_secs(6));
        }
    }
}
