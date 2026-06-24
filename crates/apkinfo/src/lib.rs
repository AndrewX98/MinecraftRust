use axml_parser::{AXMLParser, EventType};

#[derive(Debug, Clone)]
pub struct ApkInfo {
    pub version_code: i32,
    pub version_name: String,
    pub package: String,
    pub split: String,
}

impl ApkInfo {
    pub fn from_xml(parser: &mut AXMLParser) -> Self {
        let mut path: Vec<String> = Vec::new();
        let mut ret = ApkInfo {
            version_code: 0,
            version_name: String::new(),
            package: String::new(),
            split: String::new(),
        };

        while parser.next() {
            match parser.event_type() {
                EventType::StartElement => {
                    let name = parser.get_element_name();
                    path.push(name);

                    if path.len() == 1 && path[0] == "manifest" {
                        let count = parser.get_element_attribute_count();
                        for i in 0..count {
                            let attr_name = parser.get_element_attribute_name(i);
                            if attr_name == "versionCode" {
                                ret.version_code = parser.get_element_attribute_typed_value(i).data as i32;
                            } else if attr_name == "versionName" {
                                ret.version_name = parser.get_element_attribute_raw_value(i);
                            } else if attr_name == "package" {
                                ret.package = parser.get_element_attribute_raw_value(i);
                            } else if attr_name == "split" {
                                ret.split = parser.get_element_attribute_raw_value(i);
                            }
                        }
                    }
                }
                EventType::EndElement => {
                    path.pop();
                }
                _ => {}
            }
        }
        ret
    }
}
