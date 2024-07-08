use core::fmt;

use std::{
    collections::HashMap,
    fs,
    io::Read,
    path::{Path, PathBuf},
};

use macros::{Complete, Partialize};
use regex::Regex;
use serde::{ser::SerializeMap, Deserialize, Serialize};

use crate::partial::{Complete, Partial};

#[derive(Serialize, Partialize, Debug)]
pub struct Config {
    pub style: BacktraceStyle,
    pub echo: Echo,
    pub hyperlinks: HyperLinks,
    pub env: HashMap<String, String>,
    pub hide: Vec<Hide>,
}

impl Config {
    pub fn read() -> anyhow::Result<Config> {
        PartialConfig::read().map(PartialConfig::into_complete)
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
            hide: vec![Hide::Range {
                begin: Regex::new("core::panicking::panic_explicit").unwrap(),
                end: None,
            }],
            env: Default::default(),
            echo: Default::default(),
            hyperlinks: Default::default(),
        }
    }
}

#[derive(Serialize, Partialize, Debug)]
pub struct HyperLinks {
    pub enabled: bool,
    pub url: String,
}

impl HyperLinks {
    pub fn render(&self, file: &str, line: usize, col: usize) -> String {
        self.url
            .replace("${LINE}", &format!("{line}"))
            .replace("${COLUMN}", &format!("{col}"))
            .replace("${FILE_PATH}", file)
    }
}

impl Default for HyperLinks {
    fn default() -> Self {
        Self {
            enabled: false,
            url: r"file://${FILE_PATH}".to_string(),
        }
    }
}

#[derive(Clone, Copy, Serialize, Deserialize, Complete, Default, Debug)]
#[serde(from = "bool")]
#[serde(into = "bool")]
pub enum Echo {
    #[default]
    True,
    False,
}

impl From<bool> for Echo {
    fn from(b: bool) -> Self {
        if b {
            Echo::True
        } else {
            Echo::False
        }
    }
}

impl Into<bool> for Echo {
    fn into(self) -> bool {
        match self {
            Echo::True => true,
            Echo::False => false,
        }
    }
}

#[derive(Clone, Copy, Serialize, Deserialize, Debug, Default, Complete)]
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

#[derive(Debug)]
pub enum Hide {
    Pattern { pattern: Regex },
    Range { begin: Regex, end: Option<Regex> },
}

const PATTERN: &str = "pattern";
const BEGIN: &str = "begin";
const END: &str = "end";

// Unfortunately we have to implement our own deserializer.
// See https://github.com/toml-rs/toml/issues/748 and https://github.com/toml-rs/toml/issues/535
impl<'de> Deserialize<'de> for Hide {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
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
                    Ok(Hide::Range { begin, end })
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

impl Serialize for Hide {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut m = serializer.serialize_map(None)?;
        match self {
            Hide::Pattern { pattern } => m.serialize_entry(PATTERN, pattern.as_str())?,
            Hide::Range { begin, end } => {
                m.serialize_entry(BEGIN, begin.as_str())?;
                if let Some(end) = end {
                    m.serialize_entry(END, end.as_str())?;
                }
            }
        }
        m.end()
    }
}

impl PartialConfig {
    fn read() -> anyhow::Result<PartialConfig> {
        let config = PartialConfig::find_home_file()
            .map(PartialConfig::parse_file)
            .transpose()?
            .unwrap_or_else(|| Config::default().into_partial());
        let Some(local_path) = PartialConfig::find_local_file() else {
            return Ok(config);
        };
        Ok(config.merge_with(PartialConfig::parse_file(local_path)?))
    }

    fn parse_file(path: PathBuf) -> anyhow::Result<PartialConfig> {
        let mut contents = String::new();
        let mut file = fs::File::open(path)?;
        file.read_to_string(&mut contents)?;
        let config = toml::from_str(&contents)?;
        Ok(config)
    }

    fn find_home_file() -> Option<PathBuf> {
        let home_dir = home::home_dir()?;
        PartialConfig::find_file_in(&home_dir)
    }

    fn find_local_file() -> Option<PathBuf> {
        let mut path = std::env::current_dir().unwrap();
        loop {
            if let Some(file) = PartialConfig::find_file_in(&path) {
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
