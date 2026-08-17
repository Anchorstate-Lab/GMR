use async_trait::async_trait;
use gmr_content::ContentError;
use gmr_probe::Budget;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Answer {
    pub status: u16,
    pub body: String,
}

#[async_trait]
pub(crate) trait Http: Send + Sync {
    async fn get(&self, url: &str, budget: &Budget) -> Result<Answer, ContentError>;
}

pub(crate) struct Credential {
    pub(crate) header: &'static str,
    pub(crate) value: String,
}

pub(crate) struct Reqwest {
    client: reqwest::Client,
    credential: Option<Credential>,
}

impl Reqwest {
    pub(crate) fn new(credential: Option<Credential>) -> Result<Self, ContentError> {
        reqwest::Client::builder()
            .build()
            .map(|client| Self { client, credential })
            .map_err(|e| ContentError::new(format!("cannot build an HTTP client: {e}")))
    }
}

#[async_trait]
impl Http for Reqwest {
    async fn get(&self, url: &str, budget: &Budget) -> Result<Answer, ContentError> {
        let left = budget
            .remaining()
            .ok_or_else(|| ContentError::spent("no time left to call mem0"))?;
        let mut request = self.client.get(url).timeout(left);
        if let Some(credential) = &self.credential {
            request = request.header(credential.header, &credential.value);
        }
        let response = request.send().await.map_err(|e| match e.is_timeout() {
            true => ContentError::spent(format!("mem0 did not answer within the budget: {e}")),
            false => ContentError::new(format!("cannot reach mem0: {e}")),
        })?;
        let status = response.status().as_u16();
        let body = response
            .text()
            .await
            .map_err(|e| ContentError::new(format!("cannot read mem0's answer: {e}")))?;
        Ok(Answer { status, body })
    }
}

#[cfg(test)]
pub(crate) mod testkit {
    use super::{Answer, Http};
    use async_trait::async_trait;
    use gmr_content::ContentError;
    use gmr_probe::Budget;
    use std::collections::BTreeMap;
    use std::sync::{Arc, Mutex};

    pub(crate) type Asked = Arc<Mutex<Vec<String>>>;

    #[derive(Default)]
    pub(crate) struct Canned {
        answers: BTreeMap<String, Answer>,
        asked: Asked,
        refuse: Option<String>,
    }

    impl Canned {
        pub(crate) fn new() -> Self {
            Self::default()
        }

        pub(crate) fn log(&self) -> Asked {
            self.asked.clone()
        }

        pub(crate) fn on(mut self, url_suffix: &str, status: u16, body: &str) -> Self {
            self.answers.insert(
                url_suffix.to_owned(),
                Answer {
                    status,
                    body: body.to_owned(),
                },
            );
            self
        }

        pub(crate) fn refusing(mut self, why: &str) -> Self {
            self.refuse = Some(why.to_owned());
            self
        }
    }

    #[async_trait]
    impl Http for Canned {
        async fn get(&self, url: &str, _budget: &Budget) -> Result<Answer, ContentError> {
            self.asked.lock().unwrap().push(url.to_owned());
            if let Some(why) = &self.refuse {
                return Err(ContentError::new(why.clone()));
            }
            self.answers
                .iter()
                .find(|(suffix, _)| url.ends_with(suffix.as_str()))
                .map(|(_, answer)| answer.clone())
                .ok_or_else(|| ContentError::new(format!("this fake was not told about `{url}`")))
        }
    }
}
