use gmr_core::ProbeName;
use gmr_transport::recipes::Recipes;
use gmr_transport::{file, http, sql};

const AS_JSON: &str = r#"{
  "http": { "serde-latest": { "url": "https://crates.io/api/v1/crates/serde",
                              "select": "$.crate.max_stable_version" } },
  "file": { "deploy-replicas": { "path": "deploy.yaml", "select": "$.service.replicas",
                                 "shaped": "yaml" } },
  "sql":  { "schema-version": { "source": { "from_env": "DATABASE_URL" },
                                "query": "SELECT version FROM migrations",
                                "column": "version" } }
}"#;

#[test]
fn a_recipe_handed_in_as_data_is_the_recipe_a_transport_asks_for() {
    let held: Recipes = serde_json::from_str(AS_JSON).unwrap();

    assert_eq!(
        http::Asks::ask(&held, &ProbeName::new("serde-latest")),
        Some(
            http::Ask::at("https://crates.io/api/v1/crates/serde")
                .selecting("$.crate.max_stable_version")
        ),
        "a caller with no Rust in its hands declares a probe by sending this, and the \
         transport must not be able to tell the difference"
    );
    assert_eq!(
        file::Asks::ask(&held, &ProbeName::new("deploy-replicas")),
        Some(
            file::Ask::at("deploy.yaml")
                .selecting("$.service.replicas")
                .shaped_as(file::Shaped::Yaml)
        )
    );
    assert_eq!(
        sql::Asks::ask(&held, &ProbeName::new("schema-version")),
        Some(
            sql::Ask::on(
                sql::Source::FromEnv("DATABASE_URL".to_owned()),
                "SELECT version FROM migrations"
            )
            .taking("version")
        )
    );
}

#[test]
fn a_credential_is_named_by_a_recipe_and_never_carried_in_one() {
    let held: Recipes = serde_json::from_str(AS_JSON).unwrap();
    let back = serde_json::to_value(&held).unwrap();

    assert_eq!(
        back.pointer("/sql/schema-version/source/from_env"),
        Some(&serde_json::json!("DATABASE_URL")),
        "the variable's name is the whole declaration, and it is what travels: {back}"
    );
    assert_eq!(
        back.pointer("/sql/schema-version/source/given"),
        None,
        "resolving the variable here would put a password into every wire, file and log \
         that ever carries this recipe -- including an append-only one"
    );
}

#[test]
fn what_a_recipe_earns_does_not_depend_on_how_it_arrived() {
    let held: Recipes = serde_json::from_str(AS_JSON).unwrap();
    let built = http::Ask::at("https://crates.io/api/v1/crates/serde")
        .selecting("$.crate.max_stable_version");

    assert_eq!(
        http::Asks::ask(&held, &ProbeName::new("serde-latest"))
            .unwrap()
            .version(),
        built.version(),
        "the version is earned from the declaration, so a probe declared over the wire and \
         the same probe declared in Rust are one probe -- were they two, every observation \
         either made of the other would be Incomparable for no reason a person could see"
    );
}
