use git2::build::RepoBuilder;
use std::path::Path;

fn main() {
    let url = "https://github.com/Bennylavaa/pfQuest-wotlk.git";
    let path = Path::new("test_clone");
    let mut builder = RepoBuilder::new();
    println!("Cloning...");
    match builder.clone(url, path) {
        Ok(_) => println!("Success!"),
        Err(e) => println!("Error: {}", e),
    }
}
