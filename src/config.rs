use core::fmt;

use std::{
    collections::HashMap,
    fs,
    io::{self, Read},
    path::{Path, PathBuf},
};

use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_with::{serde_as, DisplayFromStr};

#[derive(Serialize, Debug)]
pub struct Config {
    pub style: BacktraceStyle,
    pub hide: Vec<Hide>,
    pub env: HashMap<String, String>,
    pub clicolor_force: ColorChoice,
    pub hide_output: bool,
}

impl Config {
    pub fn read() -> anyhow::Result<Config> {
        OptConfig::read().map(Config::from)
    }

    pub fn should_set_clicolor_force(&self) -> bool {
        match self.clicolor_force {
            ColorChoice::Auto => {
                match anstream::AutoStream::choice(&io::stderr()) {
                    anstream::ColorChoice::Never => false,
                    anstream::ColorChoice::AlwaysAnsi | anstream::ColorChoice::Always => true,
                    // this should never happen
                    anstream::ColorChoice::Auto => false,
                }
            }
            ColorChoice::Always => true,
            ColorChoice::Never => false,
        }
    }
}

impl fmt::Display for Config {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", toml::to_string_pretty(self).unwrap())
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            style: Default::default(),
            hide: vec![Hide::Span {
                begin: Regex::new("core::panicking::panic_explicit").unwrap(),
                end: None,
            }],
            env: Default::default(),
            clicolor_force: Default::default(),
            hide_output: false,
        }
    }
}

impl From<OptConfig> for Config {
    fn from(conf: OptConfig) -> Self {
        Self {
            style: conf.style.unwrap_or_default(),
            hide: conf.hide,
            env: conf.env,
            clicolor_force: conf.clicolor_force.unwrap_or_default(),
            hide_output: conf.hide_output.unwrap_or(false),
        }
    }
}

#[derive(Default, Deserialize, Debug)]
#[serde(default)]
pub struct OptConfig {
    pub style: Option<BacktraceStyle>,
    pub hide: Vec<Hide>,
    pub env: HashMap<String, String>,
    pub clicolor_force: Option<ColorChoice>,
    pub hide_output: Option<bool>,
}

impl OptConfig {
    pub fn read() -> anyhow::Result<OptConfig> {
        let config = OptConfig::find_home_file()
            .map(OptConfig::parse_file)
            .transpose()?
            .unwrap_or_default();
        let Some(local_path) = OptConfig::find_local_file() else {
            return Ok(config);
        };
        Ok(config.merge_with(OptConfig::parse_file(local_path)?))
    }

    fn merge_with(mut self, other: OptConfig) -> OptConfig {
        self.hide.extend(other.hide);
        self.env.extend(other.env.into_iter());
        OptConfig {
            style: other.style.or(self.style),
            hide: self.hide,
            env: self.env,
            clicolor_force: other.clicolor_force.or(self.clicolor_force),
            hide_output: other.hide_output.or(self.hide_output),
        }
    }

    fn parse_file(path: PathBuf) -> anyhow::Result<OptConfig> {
        let mut contents = String::new();
        let mut file = fs::File::open(path)?;
        file.read_to_string(&mut contents)?;
        let config = toml::from_str(&contents)?;
        Ok(config)
    }

    fn find_home_file() -> Option<PathBuf> {
        let home_dir = home::home_dir()?;
        OptConfig::find_file_in(&home_dir)
    }

    fn find_local_file() -> Option<PathBuf> {
        let mut path = std::env::current_dir().unwrap();
        loop {
            if let Some(file) = OptConfig::find_file_in(&path) {
                return Some(file);
            }
            if !path.pop() {
                return None;
            }
        }
    }

    fn find_file_in(dir: &Path) -> Option<PathBuf> {
        for name in ["backtracetk.toml", ".backtracetk.toml"] {
            let file = dir.join(name);
            if file.exists() {
                return Some(file);
            }
        }
        None
    }
}

#[derive(Clone, Copy, Serialize, clap::ValueEnum, Deserialize, Debug, Default)]
#[serde(rename_all = "lowercase")]
pub enum ColorChoice {
    #[default]
    Auto,
    Always,
    Never,
}

#[derive(Clone, Copy, Serialize, Deserialize, clap::ValueEnum, Debug, Default)]
#[serde(rename_all = "lowercase")]
pub enum BacktraceStyle {
    #[default]
    Short,
    Full,
}

impl BacktraceStyle {
    pub fn env_var_str(&self) -> &'static str {
        match self {
            BacktraceStyle::Short => "1",
            BacktraceStyle::Full => "full",
        }
    }
}

#[serde_as]
#[derive(Serialize, Debug)]
#[serde(untagged)]
pub enum Hide {
    Pattern {
        #[serde_as(as = "DisplayFromStr")]
        pattern: Regex,
    },
    Span {
        #[serde_as(as = "DisplayFromStr")]
        begin: Regex,
        #[serde_as(as = "Option<DisplayFromStr>")]
        end: Option<Regex>,
    },
}

// Unfortunately we have to implement our own deserializer.
// See https://github.com/toml-rs/toml/issues/748 and https://github.com/toml-rs/toml/issues/535
impl<'de> Deserialize<'de> for Hide {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        const PATTERN: &str = "pattern";
        const BEGIN: &str = "begin";
        const END: &str = "end";

        use serde::de::Error;

        struct Visitor;
        impl<'de> serde::de::Visitor<'de> for Visitor {
            type Value = Hide;

            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                write!(
                    f,
                    "a map with wither the field `{PATTERN}`, or the fields `{BEGIN}` and optionally `{END}`"
                )
            }

            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: serde::de::MapAccess<'de>,
            {
                let re = |s: &str| Regex::new(s).map_err(|e| Error::custom(&e.to_string()));
                let mut entries = HashMap::<String, String>::default();
                while let Some((k, v)) = map.next_entry::<String, String>()? {
                    entries.insert(k, v);
                }

                if entries.contains_key(PATTERN) && entries.contains_key(BEGIN) {
                    return Err(Error::custom(format!(
                        "cannot use `{PATTERN}` and `{BEGIN}` toghether"
                    )));
                }
                if let Some(pattern) = entries.remove(PATTERN) {
                    let pattern = re(&pattern)?;
                    Ok(Hide::Pattern { pattern })
                } else if let Some(begin) = entries.remove(BEGIN) {
                    let begin = re(&begin)?;
                    let end = entries.remove(END).as_deref().map(re).transpose()?;
                    Ok(Hide::Span { begin, end })
                } else {
                    Err(Error::custom(format!(
                        "missing field `{PATTERN}` or `{BEGIN}`"
                    )))
                }
            }
        }
        deserializer.deserialize_map(Visitor)
    }
}