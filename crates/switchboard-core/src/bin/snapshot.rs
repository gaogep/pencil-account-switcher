fn main() {
    let Some(name) = std::env::args().nth(1) else {
        eprintln!("usage: snapshot <scenario>");
        std::process::exit(1);
    };
    match switchboard_core::snapshot::scenario(&name) {
        Some(snapshot) => println!(
            "{}",
            serde_json::to_string(&snapshot).expect("serialize snapshot")
        ),
        None => {
            eprintln!("unknown snapshot scenario: {name}");
            std::process::exit(1);
        }
    }
}
