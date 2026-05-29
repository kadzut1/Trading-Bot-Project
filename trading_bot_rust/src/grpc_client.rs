use tonic::transport::Channel;

pub struct GrpcClient {
    #[allow(dead_code)]
    channel: Channel,
    #[allow(dead_code)]
    token: String,
}

impl GrpcClient {
    pub async fn connect(token: &str) -> Result<Self, anyhow::Error> {
        let endpoint = "https://sandbox-invest-public-api.tbank.ru:443";
        let channel = Channel::from_shared(endpoint.to_string())?
            .connect()
            .await?;

        Ok(Self {
            channel,
            token: token.to_string(),
        })
    }

    pub async fn place_order(
        &self,
        figi: &str,
        quantity: i64,
        price: f64,
        direction: &str,
    ) -> Result<String, anyhow::Error> {
        println!(
            "📊 Placing order: {} {} @ {} for {}",
            direction, quantity, price, figi
        );
        Ok(format!("order_{}", chrono::Utc::now().timestamp()))
    }
}
