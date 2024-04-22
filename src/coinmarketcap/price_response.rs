use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct PriceResponse {
    data: Data,
}

#[derive(Debug, Deserialize)]
struct Data {
    #[serde(alias = "1027", alias = "825")]
    id: IdData,
}

#[derive(Debug, Deserialize)]
struct IdData {
    quote: Quote,
}

#[derive(Debug, Deserialize)]
struct Quote {
    #[serde(rename = "USD")]
    usd: Usd,
}

#[derive(Debug, Deserialize)]
struct Usd {
    price: f64,
}

impl PriceResponse {
    pub fn get_usd_price(&self) -> f64 {
        self.data.id.quote.usd.price
    }
}
