use std::io;

use crate::config::{self, Config, BOOL_SETTING_KEYS};

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
            let value = config.bool_setting(key).ok_or_else(|| unknown_key(key))?;
            println!("{value}");
            Ok(())
        }
        [cmd, key, value] if cmd == "set" => {
            let value = config::parse_bool(value).ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("invalid value for {key}: {value} (expected true or false)"),
                )
            })?;
            let mut config = load_config()?;
            if !config.set_bool_setting(key, value) {
                return Err(unknown_key(key));
            }
            config.save()?;
            println!("{key} = {value}");
            Ok(())
        }
        [cmd, key] if cmd == "toggle" => {
            let mut config = load_config()?;
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
        _ => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "usage: tmuxxer config path|list|migrate|get KEY|set KEY true|false|toggle KEY",
        )),
    }
}

fn load_config() -> io::Result<Config> {
    Config::load().map_err(|e| {
        if e.kind() == io::ErrorKind::NotFound {
            io::Error::new(
                io::ErrorKind::NotFound,
                "config not found; run tmuxxer init first",
            )
        } else {
            e
        }
    })
}

fn format_list(config: &Config) -> String {
    let mut output = String::new();

    for key in BOOL_SETTING_KEYS {
        let value = config.bool_setting(key).unwrap_or(false);
        output.push_str(&format!("{key} = {value}\n"));
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
    io::Error::new(
        io::ErrorKind::InvalidInput,
        format!(
            "unknown config key '{key}' (supported: {})",
            BOOL_SETTING_KEYS.join(", ")
        ),
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
mod tests {
    use super::*;
    use crate::config::SearchRoot;
    use std::path::PathBuf;

    #[test]
    fn format_list_includes_launch_settings_and_search_values() {
        let mut config = Config::default();
        config.sources.docker = false;
        config.docker.new_session = false;
        config.search.ignores = vec!["target".to_string(), ".git".to_string()];
        config.search.roots = vec![SearchRoot {
            path: PathBuf::from("/tmp/code"),
            depth: 2,
        }];

        let output = format_list(&config);

        assert!(output.contains("sources.sessions = true"));
        assert!(output.contains("sources.docker = false"));
        assert!(output.contains("docker.new_session = false"));
        assert!(output.contains("search.ignore = [\"target\", \".git\"]"));
        assert!(output.contains("search.roots[0].path = \"/tmp/code\""));
        assert!(output.contains("search.roots[0].depth = 2"));
    }
}
