use fluffy_open_scp_lib::asset_build_m1::build_from_schema;

fn main() {
    let mut args = std::env::args_os().skip(1);
    let root = args.next().expect("project root");
    let schema = std::fs::read_to_string(args.next().expect("schema path")).expect("read schema");
    let result = build_from_schema(std::path::Path::new(&root), &schema)
        .unwrap_or_else(|error| panic!("asset build failed: {error}"));
    println!(
        "{}",
        serde_json::to_string_pretty(&result).expect("serialize result")
    );
}
