use std::collections::BTreeMap;

use gmr_core::ProbeName;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Recipes {
    #[cfg(feature = "http")]
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub http: BTreeMap<ProbeName, crate::http::Ask>,
    #[cfg(feature = "file")]
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub file: BTreeMap<ProbeName, crate::file::Ask>,
    #[cfg(feature = "sql")]
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub sql: BTreeMap<ProbeName, crate::sql::Ask>,
}

#[cfg(feature = "http")]
impl crate::http::Asks for Recipes {
    fn ask(&self, name: &ProbeName) -> Option<crate::http::Ask> {
        self.http.get(name).cloned()
    }
}

#[cfg(feature = "file")]
impl crate::file::Asks for Recipes {
    fn ask(&self, name: &ProbeName) -> Option<crate::file::Ask> {
        self.file.get(name).cloned()
    }
}

#[cfg(feature = "sql")]
impl crate::sql::Asks for Recipes {
    fn ask(&self, name: &ProbeName) -> Option<crate::sql::Ask> {
        self.sql.get(name).cloned()
    }
}
