use std::collections::HashMap;

use serde::de::DeserializeOwned;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::{Map, Number, Value};

use crate::api::{Request, Response};
use crate::error::Error;

pub fn deserialize_odoo_nullable<'de, D, E>(data: D) -> Result<Option<E>, D::Error>
where
    D: Deserializer<'de>,
    E: Deserialize<'de>,
{
    let b = Deserialize::deserialize(data);

    match b {
        Ok(e) => Ok(Some(e)),
        Err(_) => Ok(None),
    }
}

#[derive(Debug)]
pub struct Odoo {
    host: String,
    database: String,
    uid: Option<u32>,
    password: Option<String>,
}

impl Odoo {
    pub fn new(host: &str, database: &str) -> Odoo {
        Odoo {
            host: host.to_string(),
            database: database.to_string(),
            uid: None,
            password: None,
        }
    }

    pub async fn new_and_login(
        host: &str,
        database: &str,
        login: &str,
        password: &str,
    ) -> Result<Odoo, Error> {
        let mut odoo = Odoo::new(host, database);
        odoo.login(login, password).await?;
        Ok(odoo)
    }

    pub async fn login(&mut self, login: &str, password: &str) -> Result<u32, Error> {
        let request = Request::new(
            "common",
            Some("authenticate"),
            (self.database.as_str(), login, password, ""),
        );
        let response: Response<u32> = self
            .send(&request, None)
            .await
            .map_err(|e| Error(e.to_string()))?;
        self.uid = Some(response.result);
        self.password = Some(password.to_string());
        Ok(response.result)
    }

    pub async fn start(&self) -> Result<HashMap<String, String>, Error> {
        let request: Request<()> = Request::new("common", Some("start"), ());

        let response: Response<HashMap<String, String>> = self
            .send(&request, Some("start"))
            .await
            .map_err(|e| Error(e.to_string()))?;

        Ok(response.result)
    }

    pub async fn call<T: Serialize, U: DeserializeOwned>(
        &self,
        model: &str,
        method: &str,
        args: T,
    ) -> Result<Response<U>, Error> {
        let password = self.password.as_ref().unwrap().as_str();

        let request = Request::new(
            "object",
            None,
            (
                self.database.as_str(),
                self.uid,
                password,
                model,
                method,
                args,
            ),
        );

        self.send(&request, None)
            .await
            .map_err(|e| Error(e.to_string()))
    }

    pub async fn search_read<T: Serialize, U: DeserializeOwned>(
        &self,
        model: &str,
        domain: T,
        fields: Option<Vec<&str>>,
        limit: Option<u32>,
        offset: Option<u32>,
    ) -> Result<Response<U>, Error> {
        let password = self.password.as_ref().unwrap().as_str();
        let fields = fields.unwrap_or(vec![]);

        let mut values = Map::new();
        values.insert(
            "fields".to_string(),
            Value::Array(
                fields
                    .iter()
                    .map(|f| Value::String(f.to_string()))
                    .collect(),
            ),
        );
        if limit.is_some() {
            values.insert(
                "limit".to_string(),
                Value::Number(Number::from(limit.unwrap())),
            );
        }
        if offset.is_some() {
            values.insert(
                "offset".to_string(),
                Value::Number(Number::from(offset.unwrap())),
            );
        }

        let request = Request::new(
            "object",
            None,
            (
                self.database.as_str(),
                self.uid,
                password,
                model,
                "search_read",
                vec![domain],
                values,
            ),
        );

        self.send(&request, None)
            .await
            .map_err(|e| Error(e.to_string()))
    }

    async fn send<T: Serialize, U: DeserializeOwned>(
        &self,
        request: &Request<T>,
        url: Option<&str>,
    ) -> Result<Response<U>, reqwest::Error> {
        let client = reqwest::Client::new();
        let url = format!("{}/{}", self.host, url.unwrap_or("jsonrpc"));
        let resp = client.post(&url).json(&request).send().await;
        Ok(resp?.json().await?)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use serde::Deserialize;
    use serde_json::{Map, Value};

    use crate::api::Response;
    use crate::odoo::{deserialize_odoo_nullable, Odoo};

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

    #[tokio::test]
    async fn test_start() {
        let odoo = Odoo::new("https://demo.odoo.com", "");
        let values = odoo.start().await.unwrap();
        assert_eq!(values.is_empty(), false);
        assert_eq!(values.contains_key("host"), true);
        assert_eq!(values.contains_key("database"), true);
        assert_eq!(values.contains_key("user"), true);
        assert_eq!(values.contains_key("password"), true);
    }

    #[tokio::test]
    async fn test_login() {
        let odoo = get_odoo().await;
        assert_ne!(odoo.uid.unwrap(), 0);
    }

    #[tokio::test]
    async fn test_login_failed() {
        let mut odoo = Odoo::new("https://demo.odoo.com", "fake");
        let resp = odoo.login("admin", "admin").await;
        assert_eq!(resp.is_err(), true);
    }

    #[tokio::test]
    async fn test_new_and_login_failed() {
        let odoo = Odoo::new_and_login("https://demo.odoo.com", "fake", "admin", "admin").await;
        assert_eq!(odoo.is_err(), true);
    }

    #[tokio::test]
    async fn test_search() {
        let odoo = get_odoo();
        let partners: Response<Vec<u32>> = odoo
            .await
            .call("res.partner", "search", [[["id", ">", "2"]]])
            .await
            .unwrap();
        assert_ne!(partners.result.len(), 0);
    }

    #[tokio::test]
    async fn test_read() {
        let odoo = get_odoo().await;
        let partners: Response<Vec<HashMap<String, Value>>> = odoo
            .call("res.partner", "read", ([2], ["name"]))
            .await
            .unwrap();
        assert_eq!(partners.result.len(), 1);
        assert_eq!(partners.result.get(0).unwrap().get("id").unwrap(), 2);
    }

    #[tokio::test]
    async fn test_search_read() {
        let odoo = get_odoo().await;
        let partners: Response<Vec<Value>> = odoo
            .search_read(
                "res.partner",
                (("id", ">", 2),),
                Some(vec!["name"]),
                None,
                None,
            )
            .await
            .unwrap();
        assert_ne!(partners.result.len(), 0);
        for partner in partners.result {
            let p = partner.as_object().unwrap();
            assert_ne!(p.get("id").unwrap().as_i64().unwrap(), 0);
        }

        let partners: Response<Vec<Value>> = odoo
            .search_read(
                "res.partner",
                (("id", ">", 0),),
                Some(vec!["name"]),
                Some(5),
                None,
            )
            .await
            .unwrap();
        assert_eq!(partners.result.len(), 5);
    }

    #[tokio::test]
    async fn test_create_and_write() {
        let odoo = get_odoo().await;
        let mut values = Map::new();
        values.insert("name".to_string(), Value::from("Test"));
        let result: Response<u32> = odoo
            .call("res.partner", "create", vec![&values])
            .await
            .unwrap();
        let id = result.result;
        assert_ne!(id, 0);
        let result: Response<bool> = odoo
            .call("res.partner", "write", (vec![id], &values))
            .await
            .unwrap();
        assert_eq!(result.result, true);
    }

    #[derive(Deserialize)]
    struct Partner {
        id: u32,
        name: String,
    }

    #[tokio::test]
    async fn test_search_read_serde() {
        let odoo = get_odoo().await;

        let partners: Response<Vec<Partner>> = odoo
            .search_read("res.partner", (("id", ">", 2),), None, Some(5), None)
            .await
            .unwrap();
        let partners = partners.result;
        assert_eq!(partners.len(), 5);
        for partner in partners {
            assert_ne!(partner.id, 0);
            assert_ne!(partner.name.len(), 0);
        }
    }

    #[derive(Deserialize)]
    struct ProductTemplate {
        pub id: u32,
        pub name: String,
        #[serde(deserialize_with = "deserialize_odoo_nullable")]
        pub default_code: Option<String>,
    }

    #[tokio::test]
    async fn test_search_read_serde_nullable() {
        let odoo = get_odoo().await;

        let products: Response<Vec<ProductTemplate>> = odoo
            .search_read(
                "product.template",
                (("default_code", "=", false),),
                Some(vec!["name", "default_code"]),
                Some(5),
                None,
            )
            .await
            .unwrap();
        let products = products.result;
        assert_eq!(products.len(), 5);
        for product in products {
            assert_ne!(product.id, 0);
            assert_ne!(product.name.len(), 0);
            assert_eq!(product.default_code, None);
        }
    }
}
