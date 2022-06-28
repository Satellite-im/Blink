mod data_fragment;
mod conflux;

use crate::data_fragment::{DataFragment, StaticFragment, LiveFragment};

fn main() {
    println!("Hello, world!");
    let fragment: DataFragment = DataFragment::default().from("MockData".to_string());
    println!("{:?}", fragment);
}
