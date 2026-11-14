//! 1:1 of the Continuity client.config classes, by PepperCode1 (LGPL-3.0-only,
//! https://github.com/PepperCode1/Continuity). Upstream files:
//! src/main/java/me/pepperbell/continuity/client/config/{Option,ContinuityConfig}.java.

use std::cell::Cell;
use std::path::PathBuf;
use std::rc::Rc;
use std::thread_local;

/// java: com.google.gson.JsonElement (engine surface; only the shapes the config
/// touches: primitives and objects, object order preserved).
#[derive(Clone, Debug, PartialEq)]
pub enum JsonElement {
    /// java: JsonPrimitive (boolean form only; the config only reads/writes booleans).
    Primitive(bool),
    Object(Vec<(String, JsonElement)>),
}

impl JsonElement {
    /// java: isJsonObject().
    pub fn is_json_object(&self) -> bool {
        matches!(self, JsonElement::Object(_))
    }

    /// java: JsonObject.get(String).
    pub fn get(&self, key: &str) -> Option<&JsonElement> {
        match self {
            JsonElement::Object(entries) => entries.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    /// java: JsonElement.getAsBoolean().
    pub fn get_as_boolean(&self) -> bool {
        match self {
            JsonElement::Primitive(value) => *value,
            _ => false,
        }
    }
}

/// java: JsonParseException (engine surface).
pub type JsonParseException = String;

/// java: client.config.Option<T>. Renamed ConfigOption in Rust to avoid shadowing
/// std::option::Option.
pub trait ConfigOption {
    /// java: getKey().
    fn get_key(&self) -> &str;

    /// java: get().
    fn get(&self) -> bool;

    /// java: set(T value).
    fn set(&self, value: bool);

    /// java: toJson().
    fn to_json(&self) -> JsonElement;

    /// java: fromJson(JsonElement) throws JsonParseException.
    fn from_json(&self, json: &JsonElement) -> Result<(), JsonParseException>;
}

/// java: Option.BaseOption<T> / Option.BooleanOption (only the boolean option
/// exists upstream; the generic base is carried into the one concrete class).
pub struct BooleanOption {
    key: String,
    value: Cell<bool>,
}

impl BooleanOption {
    /// java: BaseOption(String key, T defaultValue).
    pub fn new(key: &str, default_value: bool) -> BooleanOption {
        BooleanOption { key: key.to_string(), value: Cell::new(default_value) }
    }

    /// java: BaseOption.get().
    pub fn get(&self) -> bool {
        self.value.get()
    }

    /// java: BaseOption.set(T value).
    pub fn set(&self, value: bool) {
        self.value.set(value);
    }
}

impl ConfigOption for BooleanOption {
    fn get_key(&self) -> &str {
        &self.key
    }

    fn get(&self) -> bool {
        self.value.get()
    }

    fn set(&self, value: bool) {
        self.value.set(value);
    }

    /// java: BooleanOption.toJson() = new JsonPrimitive(get()).
    fn to_json(&self) -> JsonElement {
        JsonElement::Primitive(self.get())
    }

    /// java: BooleanOption.fromJson(JsonElement): primitive -> set(getAsBoolean),
    /// else throw JsonParseException("Json must be a primitive").
    fn from_json(&self, json: &JsonElement) -> Result<(), JsonParseException> {
        match json {
            JsonElement::Primitive(_) => {
                self.set(json.get_as_boolean());
                Ok(())
            }
            _ => Err("Json must be a primitive".to_string()),
        }
    }
}

/// java: FabricLoader.getInstance().getConfigDir() (engine seam; the port reads the
/// loader's config dir from CONTINUITY_CONFIG_DIR, defaulting to ./config).
pub fn fabric_loader_get_config_dir() -> PathBuf {
    std::env::var_os("CONTINUITY_CONFIG_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("config"))
}

/// java: client.config.ContinuityConfig.
pub struct ContinuityConfig {
    file: PathBuf,
    /// java: Object2ObjectLinkedOpenHashMap<String, Option<?>> (insertion order).
    option_map: Vec<(String, Rc<BooleanOption>)>,
    pub connected_textures: Rc<BooleanOption>,
    pub emissive_textures: Rc<BooleanOption>,
    pub custom_block_layers: Rc<BooleanOption>,
    pub use_manual_culling: Rc<BooleanOption>,
}

thread_local! {
    static INSTANCE: Rc<ContinuityConfig> = Rc::new(build_instance());
}

fn build_instance() -> ContinuityConfig {
    // java: static INSTANCE = new ContinuityConfig(FabricLoader.getInstance()
    // .getConfigDir().resolve("continuity.json").toFile()); static { INSTANCE.load(); }
    let config = ContinuityConfig::new(fabric_loader_get_config_dir().join("continuity.json"));
    config.load();
    config
}

/// java: ContinuityConfig.INSTANCE singleton.
pub fn config_instance() -> Rc<ContinuityConfig> {
    INSTANCE.with(|config| config.clone())
}

impl ContinuityConfig {
    pub fn new(file: PathBuf) -> ContinuityConfig {
        // java: the four addOption(...) field initializers.
        let connected_textures = Rc::new(BooleanOption::new("connected_textures", true));
        let emissive_textures = Rc::new(BooleanOption::new("emissive_textures", true));
        let custom_block_layers = Rc::new(BooleanOption::new("custom_block_layers", true));
        let use_manual_culling = Rc::new(BooleanOption::new("use_manual_culling", true));
        let mut option_map = Vec::new();
        for option in [&connected_textures, &emissive_textures, &custom_block_layers, &use_manual_culling] {
            // java: addOption (duplicate keys warn upstream; the four keys are distinct).
            option_map.push((option.get_key().to_string(), option.clone()));
        }
        ContinuityConfig {
            file,
            option_map,
            connected_textures,
            emissive_textures,
            custom_block_layers,
            use_manual_culling,
        }
    }

    /// java: load(): read the file when it exists, then always save().
    pub fn load(&self) {
        if self.file.exists() {
            match std::fs::read_to_string(&self.file) {
                Ok(text) => match parse_json(&text) {
                    Ok(json) => self.from_json(&json),
                    Err(error) => eprintln!("Could not load config from file '{}': {}", self.file.display(), error),
                },
                Err(error) => eprintln!("Could not load config from file '{}': {}", self.file.display(), error),
            }
        }
        self.save();
    }

    /// java: save().
    pub fn save(&self) {
        let json = self.to_json();
        let text = json_to_string(&json);
        if let Err(error) = std::fs::write(&self.file, text) {
            eprintln!("Could not save config to file '{}': {}", self.file.display(), error);
        }
    }

    /// java: fromJson(JsonElement): object entries in map order, per-entry parse
    /// errors logged and skipped; non-object throws JsonParseException("Json must be an object").
    pub fn from_json(&self, json: &JsonElement) {
        if json.is_json_object() {
            for (key, option) in &self.option_map {
                if let Some(element) = json.get(key) {
                    if let Err(error) = option.from_json(element) {
                        eprintln!("Could not read option '{}': {}", key, error);
                    }
                }
            }
        } else {
            eprintln!("Could not load config: Json must be an object");
        }
    }

    /// java: toJson().
    pub fn to_json(&self) -> JsonElement {
        JsonElement::Object(
            self.option_map
                .iter()
                .map(|(key, option)| (key.clone(), option.to_json()))
                .collect(),
        )
    }

    /// java: getOption(String key).
    pub fn get_option(&self, key: &str) -> Option<Rc<BooleanOption>> {
        self.option_map.iter().find(|(k, _)| k == key).map(|(_, option)| option.clone())
    }

    /// java: getOptionMapView() (the port exposes the insertion-ordered entries).
    pub fn get_option_map_view(&self) -> Vec<(String, Rc<BooleanOption>)> {
        self.option_map.clone()
    }
}

/// Engine seam: the minimal Gson reader for the config's own output (an object of
/// boolean primitives, whitespace allowed).
fn parse_json(text: &str) -> Result<JsonElement, String> {
    let trimmed = text.trim();
    if !trimmed.starts_with('{') || !trimmed.ends_with('}') {
        return Err("Json must be an object".to_string());
    }
    let mut entries = Vec::new();
    let body = &trimmed[1..trimmed.len() - 1];
    for entry in body.split(',') {
        let entry = entry.trim();
        if entry.is_empty() {
            continue;
        }
        let Some(colon) = entry.find(':') else {
            return Err(format!("Malformed entry '{}'", entry));
        };
        let key = entry[..colon].trim().trim_matches('"').to_string();
        let value_text = entry[colon + 1..].trim();
        match value_text {
            "true" => entries.push((key, JsonElement::Primitive(true))),
            "false" => entries.push((key, JsonElement::Primitive(false))),
            other => return Err(format!("Unsupported value '{}'", other)),
        }
    }
    Ok(JsonElement::Object(entries))
}

/// Engine seam: the minimal Gson writer for the config's shape (pretty-printed
/// object of boolean primitives).
fn json_to_string(json: &JsonElement) -> String {
    match json {
        JsonElement::Object(entries) => {
            let lines: Vec<String> = entries
                .iter()
                .map(|(key, value)| {
                    let value_text = match value {
                        JsonElement::Primitive(inner) => inner.to_string(),
                        _ => String::new(),
                    };
                    format!("  \"{}\": {}", key, value_text)
                })
                .collect();
            format!("{{\n{}\n}}", lines.join(",\n"))
        }
        _ => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn options_round_trip_defaults() {
        let config = ContinuityConfig::new(std::env::temp_dir().join(format!("cont-cfg-{}.json", std::process::id())));
        assert!(config.connected_textures.get());
        assert!(config.emissive_textures.get());
        assert!(config.custom_block_layers.get());
        assert!(config.use_manual_culling.get());
        let json = config.to_json();
        assert_eq!(
            json,
            JsonElement::Object(vec![
                ("connected_textures".to_string(), JsonElement::Primitive(true)),
                ("emissive_textures".to_string(), JsonElement::Primitive(true)),
                ("custom_block_layers".to_string(), JsonElement::Primitive(true)),
                ("use_manual_culling".to_string(), JsonElement::Primitive(true)),
            ])
        );
    }

    #[test]
    fn from_json_updates_and_rejects_bad_entries() {
        let config = ContinuityConfig::new(std::env::temp_dir().join(format!("cont-cfg-{}.json", std::process::id())));
        let json = JsonElement::Object(vec![
            ("connected_textures".to_string(), JsonElement::Primitive(false)),
            ("emissive_textures".to_string(), JsonElement::Object(vec![])),
        ]);
        config.from_json(&json);
        assert!(!config.connected_textures.get());
        // the malformed entry is skipped, the option keeps its default
        assert!(config.emissive_textures.get());
        config.connected_textures.set(true);
    }

    #[test]
    fn boolean_option_from_json_rejects_non_primitive() {
        let option = BooleanOption::new("k", true);
        assert!(option.from_json(&JsonElement::Object(vec![])).is_err());
        option.from_json(&JsonElement::Primitive(false)).unwrap();
        assert!(!option.get());
    }

    #[test]
    fn save_and_load_round_trip() {
        let path = std::env::temp_dir().join(format!("cont-cfg-rt-{}.json", std::process::id()));
        let config = ContinuityConfig::new(path.clone());
        config.use_manual_culling.set(false);
        config.save();
        let reloaded = ContinuityConfig::new(path.clone());
        reloaded.load();
        assert!(!reloaded.use_manual_culling.get());
        assert!(reloaded.connected_textures.get());
        let _ = std::fs::remove_file(path);
    }
}
