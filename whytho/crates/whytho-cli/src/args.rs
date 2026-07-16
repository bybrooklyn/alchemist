use std::collections::HashMap;

pub struct CliArgs {
    pub command: String,
    pub positional: Vec<String>,
    pub flags: HashMap<String, String>,
    pub bool_flags: Vec<String>,
}

impl CliArgs {
    pub fn parse() -> Result<Self, String> {
        let raw: Vec<String> = std::env::args().skip(1).collect();
        let mut iter = raw.iter();
        let command = match iter.next() {
            Some(c) => c.clone(),
            None => return Err("no command provided".into()),
        };

        let mut positional = Vec::new();
        let mut flags = HashMap::new();
        let mut bool_flags = Vec::new();

        while let Some(arg) = iter.next() {
            if arg == "--" {
                positional.extend(iter.map(|s| s.clone()));
                break;
            }

            if arg.starts_with("--") {
                let flag_name = arg.trim_start_matches('-').to_string();
                if let Some(next) = iter.clone().next() {
                    if next.starts_with('-') {
                        bool_flags.push(flag_name);
                    } else {
                        iter.next();
                        flags.insert(flag_name, next.clone());
                    }
                } else {
                    bool_flags.push(flag_name);
                }
            } else if arg.starts_with('-') && arg.len() > 1 {
                let flag_name = arg[1..].to_string();
                if let Some(next) = iter.clone().next() {
                    if next.starts_with('-') {
                        bool_flags.push(flag_name);
                    } else {
                        iter.next();
                        flags.insert(flag_name, next.clone());
                    }
                } else {
                    bool_flags.push(flag_name);
                }
            } else {
                positional.push(arg.clone());
            }
        }

        Ok(Self {
            command,
            positional,
            flags,
            bool_flags,
        })
    }

    pub fn flag(&self, name: &str) -> Option<&str> {
        self.flags.get(name).map(|s| s.as_str())
    }

    pub fn has_flag(&self, name: &str) -> bool {
        self.bool_flags.contains(&name.to_string())
    }
}
