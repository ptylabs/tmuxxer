use std::io;

use crate::config::{self, BOOL_SETTING_KEYS, Config, STRING_SETTING_KEYS};

pub fn run(args: &[String]) -> io::Result<()> {
    match args {
        [cmd] if cmd == "path" => {
            println!("{}", config::config_path().display());
            Ok(())
        }
        [cmd] if cmd == "list" => {
            let config = load_config()?;
            print!("{}", format_list(&config));
            Ok(())
        }
        [cmd, key] if cmd == "get" => {
            let config = load_config()?;
            if let Some(value) = config.bool_setting(key) {
                println!("{value}");
            } else if let Some(value) = config.string_setting(key) {
                println!("{value}");
            } else {
                return Err(unknown_key(key));
            }
            Ok(())
        }
        [cmd, key, value] if cmd == "set" => {
            let mut config = load_config()?;
            if config.bool_setting(key).is_some() {
                let value = config::parse_bool(value).ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidInput,
                        format!("invalid value for {key}: {value} (expected true or false)"),
                    )
                })?;
                config.set_bool_setting(key, value);
                config.save()?;
                println!("{key} = {value}");
                return Ok(());
            }

            if config.string_setting(key).is_some() {
                config.set_string_setting(key, value).map_err(|message| {
                    io::Error::new(
                        io::ErrorKind::InvalidInput,
                        format!("invalid value for {key}: {value} ({message})"),
                    )
                })?;
                config.save()?;
                let value = config.string_setting(key).unwrap_or("");
                println!("{key} = {value}");
                return Ok(());
            }

            Err(unknown_key(key))
        }
        [cmd, key] if cmd == "toggle" => {
            let mut config = load_config()?;
            if config.string_setting(key).is_some() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("cannot toggle non-boolean config key '{key}'"),
                ));
            }
            let value = config
                .toggle_bool_setting(key)
                .ok_or_else(|| unknown_key(key))?;
            config.save()?;
            println!("{key} = {value}");
            Ok(())
        }
        [cmd] if cmd == "migrate" => {
            let config = load_config()?;
            config.save()?;
            println!("Migrated {}", config::config_path().display());
            Ok(())
        }
        [cmd] if cmd == "validate" => {
            load_config()?;
            println!("Config valid: {}", config::config_path().display());
            Ok(())
        }
        _ => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "usage: tmuxxer config path|list|validate|migrate|get KEY|set KEY VALUE|toggle KEY",
        )),
    }
}

fn load_config() -> io::Result<Config> {
    Config::load()
        .map(|config| config.into_inner())
        .map_err(|e| {
            if e.kind() == io::ErrorKind::NotFound {
                io::Error::new(
                    io::ErrorKind::NotFound,
                    "config not found; run tmuxxer init first",
                )
            } else {
                e.into()
            }
        })
}

fn format_list(config: &Config) -> String {
    let mut output = String::new();

    for key in BOOL_SETTING_KEYS {
        let value = config.bool_setting(key).unwrap_or(false);
        output.push_str(&format!("{key} = {value}\n"));
    }

    for key in STRING_SETTING_KEYS {
        let value = config.string_setting(key).unwrap_or("");
        output.push_str(&format!("{key} = {}\n", toml_string(value)));
    }

    output.push_str(&format!(
        "search.ignore = {}\n",
        toml_string_array(&config.search.ignores)
    ));

    for (index, root) in config.search.roots.iter().enumerate() {
        output.push_str(&format!(
            "search.roots[{index}].path = {}\n",
            toml_string(&config::stored_path(&root.path))
        ));
        output.push_str(&format!(
            "search.roots[{index}].depth = {}\n",
            root.depth.max(1)
        ));
    }

    output
}

fn unknown_key(key: &str) -> io::Error {
    let supported = BOOL_SETTING_KEYS
        .iter()
        .chain(STRING_SETTING_KEYS.iter())
        .copied()
        .collect::<Vec<_>>()
        .join(", ");

    io::Error::new(
        io::ErrorKind::InvalidInput,
        format!("unknown config key '{key}' (supported: {supported})"),
    )
}

fn toml_string_array(values: &[String]) -> String {
    let values = values
        .iter()
        .map(|value| toml_string(value))
        .collect::<Vec<_>>()
        .join(", ");
    format!("[{values}]")
}

fn toml_string(value: &str) -> String {
    let escaped = value.replace('\\', "\\\\").replace('"', "\\\"");
    format!("\"{escaped}\"")
}

#[cfg(test)]
mod tests;
