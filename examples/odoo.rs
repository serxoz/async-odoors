use serde::Deserialize;

use odoors::odoo::{deserialize_odoo_nullable, Odoo};

#[derive(Deserialize, Debug)]
struct ProductTemplate {
    name: String,
    #[serde(deserialize_with = "deserialize_odoo_nullable")]
    default_code: Option<String>,
}

async fn get_odoo() -> Odoo {
    let odoo = Odoo::new("https://demo.odoo.com", "");
    let values = odoo.start().await.unwrap();
    Odoo::new_and_login(
        values.get("host").unwrap(),
        values.get("database").unwrap(),
        values.get("user").unwrap(),
        values.get("password").unwrap(),
    )
    .await
    .unwrap()
}

#[tokio::main]
async fn main() {
    let odoo = get_odoo().await;

    let product_template: Vec<ProductTemplate> = odoo
        .search_read(
            "product.template",
            (),
            Some(vec!["name", "default_code"]),
            None,
            None,
        )
        .await
        .unwrap()
        .result;
    println!("{:?}", product_template);

    for product in product_template.iter() {
        println!(
            "[{}] {}",
            product.default_code.as_ref().unwrap_or(&String::from("")),
            product.name
        );
    }
}
