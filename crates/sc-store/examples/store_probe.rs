use std::process::Command;
// Native permission/integrity diagnostic. Store::open may initialize or migrate
// the supplied database; use an explicit diagnostic database path when testing.
fn main() {
    let path = std::env::args().nth(1).expect("database path");
    println!("database: {path}");
    println!(
        "groups: {}",
        String::from_utf8_lossy(
            &Command::new("whoami")
                .arg("/groups")
                .output()
                .unwrap()
                .stdout
        )
    );
    println!("compat: {:?}", std::env::var("__COMPAT_LAYER"));
    match std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(&path)
    {
        Ok(_) => println!("native read-write handle: OK"),
        Err(error) => println!("native read-write handle: {error:?}"),
    }
    match sc_store::Store::open(&path) {
        Ok(_) => println!("Store::open: OK"),
        Err(error) => {
            eprintln!("Store::open: {error:?}");
            std::process::exit(1);
        }
    }
}
