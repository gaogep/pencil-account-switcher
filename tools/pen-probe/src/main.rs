fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match pen_probe::run(&args) {
        Ok(value) => println!("{value}"),
        Err(message) => {
            println!("{}", serde_json::json!({ "error": message }));
            std::process::exit(1);
        }
    }
}
