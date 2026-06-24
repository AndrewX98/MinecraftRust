use std::fs;
use std::env;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <AndroidManifest.xml>", args[0]);
        return;
    }

    let data = fs::read(&args[1]).expect("Failed to read file");
    let file = axml_parser::AXMLFile::new(&data).expect("Failed to parse AXML");
    let mut parser = file.parser();
    let info = apkinfo::ApkInfo::from_xml(&mut parser);
    println!("{:#?}", info);
}
