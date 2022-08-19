use cosmwasm_schema::{export_schema, remove_schemas, schema_for};
use std::{env::current_dir, fs::create_dir_all};

fn main() {
    let current_dir = current_dir().unwrap();
    let mut schema_dir = current_dir;
    schema_dir.push("schema");

    let mut tier_dir = schema_dir.clone();
    tier_dir.push("tier");

    let mut token_dir = schema_dir;
    token_dir.push("token");

    create_dir_all(&tier_dir).unwrap();
    remove_schemas(&tier_dir).unwrap();

    create_dir_all(&token_dir).unwrap();
    remove_schemas(&token_dir).unwrap();

    export_schema(&schema_for!(tier::msg::InitMsg), &tier_dir);
    export_schema(&schema_for!(tier::msg::HandleMsg), &tier_dir);
    export_schema(&schema_for!(tier::msg::QueryMsg), &tier_dir);
    export_schema(&schema_for!(tier::msg::HandleAnswer), &tier_dir);
    export_schema(&schema_for!(tier::msg::QueryAnswer), &tier_dir);

    export_schema(&schema_for!(snip721_tier_token::msg::InitMsg), &token_dir);
    export_schema(&schema_for!(snip721_tier_token::msg::HandleMsg), &token_dir);
    export_schema(&schema_for!(snip721_tier_token::msg::QueryMsg), &token_dir);
    export_schema(
        &schema_for!(snip721_tier_token::msg::HandleAnswer),
        &token_dir,
    );
    export_schema(
        &schema_for!(snip721_tier_token::msg::QueryAnswer),
        &token_dir,
    );
}
