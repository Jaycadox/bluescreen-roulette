use std::{fs::File, io::Write};

use pfa::{builder::PfaBuilder, shared::DataFlags};

fn main() {
    let mut b = PfaBuilder::new("bundle");
    b.include_directory("./bundle", DataFlags::auto()).unwrap();
    let mut file = File::create("./src/bundle.pfa").unwrap();
    file.write_all(&b.build().unwrap()).unwrap();

    println!("cargo:rerun-if-changed=bundle");
}
