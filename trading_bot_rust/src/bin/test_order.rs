use reqwest::Client;
use serde_json::json;

#[tokio::main]
async fn main() {
    let token = std::env::var("TINVEST_TOKEN").expect("TINVEST_TOKEN not set");
    let account_id = "6baab97c-8426-42fe-884c-21d877705b27";
    let figi = "BBG004730N88";
    let price: f64 = 322.50;
    let quantity = 1;

    let units = price.floor() as i64;
    let nano = ((price - units as f64) * 1_000_000_000.0) as i32;

    let client = Client::builder()
        .danger_accept_invalid_certs(true)
        .build()
        .unwrap();

    let url = "https://sandbox-invest-public-api.tbank.ru/rest/tinkoff.public.invest.api.contract.v1.OrdersService/PostOrder";

    let body = json!({
        "figi": figi,
        "quantity": quantity,
        "price": {"units": units, "nano": nano},
        "direction": "ORDER_DIRECTION_BUY",
        "accountId": account_id,
        "orderType": "ORDER_TYPE_LIMIT"
    });

    println!("📊 Sending test order...");
    println!("📊 Body: {}", serde_json::to_string_pretty(&body).unwrap());

    match client
        .post(url)
        .header("Authorization", format!("Bearer {}", token))
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
    {
        Ok(resp) => {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            println!("📊 Response status: {}", status);
            println!("📊 Response: {}", text);
            if status.is_success() {
                println!("✅ TEST ORDER SUCCESS!");
            } else {
                println!("❌ TEST ORDER FAILED: {}", text);
            }
        }
        Err(e) => println!("❌ Error: {}", e),
    }
}
