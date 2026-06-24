use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

pub struct ArgList<'a> {
    args: &'a [String],
    pos: usize,
}

impl<'a> ArgList<'a> {
    pub fn new(args: &'a [String]) -> Self {
        ArgList { args, pos: 1 }
    }

    pub fn peek(&self) -> Option<&str> {
        self.args.get(self.pos).map(|s| s.as_str())
    }

    pub fn next_or_null(&mut self) -> Option<&str> {
        let val = self.args.get(self.pos)?;
        self.pos += 1;
        Some(val)
    }

    pub fn next(&mut self) -> Result<&str, &str> {
        self.next_or_null().ok_or("Missing argument")
    }

    pub fn next_value_or_null(&mut self) -> Option<&str> {
        match self.peek() {
            Some(val) if !val.starts_with('-') => self.next().ok(),
            _ => None,
        }
    }

    pub fn next_value(&mut self) -> Result<&str, &str> {
        self.next_value_or_null().ok_or("Missing argument value")
    }
}

type HandlerInternal = Rc<RefCell<dyn FnMut(&[String], &mut usize) -> Result<(), String>>>;

pub struct ArgParser {
    handlers: HashMap<String, HandlerInternal>,
    help_entries: Vec<(String, String, String)>,
}

impl ArgParser {
    pub fn new() -> Self {
        let mut parser = ArgParser {
            handlers: HashMap::new(),
            help_entries: Vec::new(),
        };
        parser.help_entries.push((
            "--help".into(),
            "-h".into(),
            "Show this help information".into(),
        ));
        parser
    }

    pub fn add_arg<F: FnMut(&[String], &mut usize) -> Result<(), String> + 'static>(
        &mut self,
        name: &str,
        shortname: &str,
        desc: &str,
        handler: F,
    ) {
        let h = Rc::new(RefCell::new(handler));
        self.handlers.insert(name.to_string(), h.clone());
        if !shortname.is_empty() {
            self.handlers.insert(shortname.to_string(), h);
        }
        self.help_entries
            .push((name.to_string(), shortname.to_string(), desc.to_string()));
    }

    pub fn parse(&self, args: &[String]) -> bool {
        let mut list = ArgList::new(args);
        if list.next_or_null().is_none() {
            return false;
        }
        let mut pos = 1usize;
        loop {
            let key = match args.get(pos) {
                Some(k) => k.clone(),
                None => break,
            };
            pos += 1;
            if key == "-h" || key == "--help" {
                self.print_help();
                return false;
            }
            match self.handlers.get(&key) {
                Some(handler) => {
                    if let Err(e) = handler.borrow_mut()(args, &mut pos) {
                        eprintln!("Invalid argument value for {}: {}", key, e);
                        return false;
                    }
                }
                None => {
                    eprintln!("Unknown argument: {}", key);
                    return false;
                }
            }
        }
        true
    }

    pub fn print_help(&self) {
        println!("Program Help");
        for (name, shortname, desc) in &self.help_entries {
            println!("{:<2} {:<15} {}", shortname, name, desc);
        }
    }
}

pub struct ArgValue<T> {
    inner: Rc<RefCell<T>>,
}

impl<T: Clone> ArgValue<T> {
    pub fn get(&self) -> T {
        self.inner.borrow().clone()
    }
}

pub fn arg_string(
    parser: &mut ArgParser,
    name: &str,
    shortname: &str,
    desc: &str,
    default: &str,
) -> ArgValue<String> {
    let value = Rc::new(RefCell::new(default.to_string()));
    let v = value.clone();
    parser.add_arg(
        name,
        shortname,
        desc,
        move |args: &[String], pos: &mut usize| {
            let s = args.get(*pos).ok_or_else(|| "Missing argument".to_string())?;
            *pos += 1;
            *v.borrow_mut() = s.clone();
            Ok(())
        },
    );
    ArgValue { inner: value }
}

pub fn arg_i32(
    parser: &mut ArgParser,
    name: &str,
    shortname: &str,
    desc: &str,
    default: i32,
) -> ArgValue<i32> {
    let value = Rc::new(RefCell::new(default));
    let v = value.clone();
    parser.add_arg(
        name,
        shortname,
        desc,
        move |args: &[String], pos: &mut usize| {
            let s = args.get(*pos).ok_or_else(|| "Missing argument".to_string())?;
            *pos += 1;
            let n: i32 = s.parse().map_err(|e: std::num::ParseIntError| e.to_string())?;
            *v.borrow_mut() = n;
            Ok(())
        },
    );
    ArgValue { inner: value }
}

pub fn arg_bool(
    parser: &mut ArgParser,
    name: &str,
    shortname: &str,
    desc: &str,
    default: bool,
) -> ArgValue<bool> {
    let value = Rc::new(RefCell::new(default));
    let v = value.clone();
    parser.add_arg(
        name,
        shortname,
        desc,
        move |args: &[String], pos: &mut usize| {
            let s = args.get(*pos).map(|s| s.as_str());
            let b = match s {
                Some(val) if !val.starts_with('-') => {
                    *pos += 1;
                    matches!(val.to_lowercase().as_str(), "1" | "true" | "on" | "yes")
                }
                _ => true,
            };
            *v.borrow_mut() = b;
            Ok(())
        },
    );
    ArgValue { inner: value }
}

pub fn arg_flag(
    parser: &mut ArgParser,
    name: &str,
    shortname: &str,
    desc: &str,
) -> ArgValue<bool> {
    let value = Rc::new(RefCell::new(false));
    let v = value.clone();
    parser.add_arg(
        name,
        shortname,
        desc,
        move |_: &[String], _: &mut usize| {
            *v.borrow_mut() = true;
            Ok(())
        },
    );
    ArgValue { inner: value }
}
