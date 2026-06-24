use std::collections::HashMap;
use std::cell::RefCell;
use std::rc::Rc;

struct PropertyDef {
    parse: Box<dyn FnMut(String)>,
    serialize: Box<dyn Fn() -> String>,
}

pub struct PropertyList {
    props: HashMap<String, PropertyDef>,
    unknown: HashMap<String, String>,
    sep: char,
}

impl PropertyList {
    pub fn new() -> Self {
        PropertyList {
            props: HashMap::new(),
            unknown: HashMap::new(),
            sep: '=',
        }
    }

    pub fn with_separator(sep: char) -> Self {
        PropertyList {
            props: HashMap::new(),
            unknown: HashMap::new(),
            sep,
        }
    }

    pub fn register(
        &mut self,
        name: &str,
        parse: Box<dyn FnMut(String)>,
        serialize: Box<dyn Fn() -> String>,
    ) {
        self.props.insert(name.to_string(), PropertyDef { parse, serialize });
    }

    pub fn set_property(&mut self, name: &str, value: &str) {
        if let Some(def) = self.props.get_mut(name) {
            (def.parse)(value.to_string());
        } else {
            self.unknown.insert(name.to_string(), value.to_string());
        }
    }

    pub fn load(&mut self, input: &str) {
        for line in input.lines() {
            let line = line.trim_end_matches('\r');
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if let Some(pos) = line.find(self.sep) {
                let key = line[..pos].trim().to_string();
                let value = line[pos + 1..].trim().to_string();
                self.set_property(&key, &value);
            }
        }
    }

    pub fn save(&self) -> String {
        let mut out = String::new();
        for (name, def) in &self.props {
            out.push_str(name);
            out.push(self.sep);
            out.push_str(&(def.serialize)());
            out.push('\n');
        }
        for (name, value) in &self.unknown {
            out.push_str(name);
            out.push(self.sep);
            out.push_str(value);
            out.push('\n');
        }
        out
    }
}

pub struct Property<T> {
    inner: Rc<RefCell<T>>,
}

impl<T: PropertyValue + 'static> Property<T> {
    pub fn new(list: &mut PropertyList, name: &str, default: T) -> Self {
        let inner = Rc::new(RefCell::new(default));
        let inner_clone = inner.clone();
        let inner_clone2 = inner.clone();
        list.register(
            name,
            Box::new(move |v| {
                *inner_clone.borrow_mut() = T::parse_value(&v);
            }),
            Box::new(move || {
                T::serialize_value(&*inner_clone2.borrow())
            }),
        );
        Property { inner }
    }

    pub fn get(&self) -> std::cell::Ref<'_, T> {
        self.inner.borrow()
    }

    pub fn set(&self, val: T) {
        *self.inner.borrow_mut() = val;
    }
}

pub trait PropertyValue: Sized {
    fn parse_value(value: &str) -> Self;
    fn serialize_value(&self) -> String;
}

impl PropertyValue for String {
    fn parse_value(value: &str) -> Self { value.to_string() }
    fn serialize_value(&self) -> String { self.clone() }
}

impl PropertyValue for i32 {
    fn parse_value(value: &str) -> Self { value.parse().unwrap_or(0) }
    fn serialize_value(&self) -> String { self.to_string() }
}

impl PropertyValue for f32 {
    fn parse_value(value: &str) -> Self { value.parse().unwrap_or(0.0) }
    fn serialize_value(&self) -> String { self.to_string() }
}

impl PropertyValue for bool {
    fn parse_value(value: &str) -> Self {
        matches!(value.to_lowercase().as_str(), "1" | "true" | "on" | "yes")
    }
    fn serialize_value(&self) -> String {
        if *self { "true".into() } else { "false".into() }
    }
}
