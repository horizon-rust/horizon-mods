//! Emojiful ported to Horizon as a native Rust mod.
//!
//! Upstream: https://github.com/InnovativeOnlineIndustries/Emojiful at commit
//! 6442d9d75ddc39308d8cdce8f14e0a9aa3011519 (2024-06-12), MIT (c) 2022
//! Innovative Online Industries. 1:1 feature port of the client core: config,
//! emoji model (with Github/Custom/Twemoji sources), regex + text matching,
//! profanity filter, twemoji and custom-package loading, emoji indexing for
//! the selection GUI, and the datapack emoji flow.
//!
//! Game-side seams (documented in README.md): font renderer swap and emoji
//! quad rendering (EmojiFontRenderer, EmojiUtil.createRenderType/renderEmoji),
//! texture upload and GIF frame upload (Emoji.DownloadImageData and the
//! Twitmoji/Emojipedia variants), asset HTTP fetch (CommonClass.readStringFromURL,
//! Emoji.loadTextureFromServer), recipe-manager lookup (onRecipesUpdated) and
//! the chat GUI (autocomplete, selector, keybind handling).

use serde::{Deserialize, Serialize};
use serde_json::Value as Json;
use std::collections::HashMap;

pub const MOD_ID: &str = "emojiful";
pub const ASSETS_BASE: &str =
    "https://raw.githubusercontent.com/InnovativeOnlineIndustries/emojiful-assets/master/";
pub const TWEMOJI_DATA_URL: &str =
    "https://raw.githubusercontent.com/iamcal/emoji-data/master/emoji.json";
pub const CATEGORIES_URL: &str = "https://raw.githubusercontent.com/InnovativeOnlineIndustries/emojiful-assets/master/Categories.yml";
/// Upstream constant: the mod never ships a dedicated error texture, it reuses no-signal (26d4) for both.
pub const LOADING_TEXTURE: &str = "emojiful:textures/gui/26a0.png";
pub const NO_SIGNAL_TEXTURE: &str = "emojiful:textures/gui/26d4.png";
pub const ERROR_TEXTURE: &str = "emojiful:textures/gui/26d4.png";
/// Emoji quad metrics from EmojiUtil.renderEmoji (draw half is the render seam; the geometry is data).
pub const EMOJI_QUAD_SIZE: f32 = 10.0;
pub const EMOJI_OFFSET_X: f32 = 0.0;
pub const EMOJI_OFFSET_Y: f32 = 1.0;
pub const TEXTURE_SIZE: f32 = 16.0;
pub const SELECTOR_COLUMNS: usize = 9;

// ---------------------------------------------------------------------------
// Config (FabricConfigHelper / MidnightConfig)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    #[serde(rename = "renderEmoji")]
    pub render_emoji: bool,
    #[serde(rename = "emojiSelector")]
    pub emoji_selector: bool,
    #[serde(rename = "emojiAutocomplete")]
    pub emoji_autocomplete: bool,
    #[serde(rename = "gifEmojis")]
    pub gif_emojis: bool,
    #[serde(rename = "emojiReplacement")]
    pub emoji_replacement: bool,
    #[serde(rename = "enableLoadTwemoji")]
    pub enable_load_twemoji: bool,
    #[serde(rename = "loadCustom")]
    pub load_custom: bool,
    #[serde(rename = "loadDatapack")]
    pub load_datapack: bool,
    #[serde(rename = "profanityFilter")]
    pub profanity_filter: bool,
    #[serde(rename = "profanityFilterReplacement")]
    pub profanity_filter_replacement: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            render_emoji: true,
            emoji_selector: true,
            emoji_autocomplete: true,
            gif_emojis: true,
            emoji_replacement: true,
            enable_load_twemoji: true,
            load_custom: true,
            load_datapack: true,
            profanity_filter: false,
            profanity_filter_replacement: ":swear:".to_string(),
        }
    }
}

pub const CONFIG_FILE: &str = "emojiful.json";

impl Config {
    pub fn to_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_else(|_| "{}".to_string())
    }

    /// Broken file falls back to defaults, mirroring MidnightConfig's behavior
    /// of regenerating entries that are missing or unreadable.
    pub fn from_json(text: &str) -> Config {
        serde_json::from_str(text).unwrap_or_default()
    }
}

// ---------------------------------------------------------------------------
// Emoji model (api/Emoji.java, EmojiFromGithub)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct EmojiCategory {
    pub name: String,
    pub world_based: bool,
}

impl EmojiCategory {
    pub fn new(name: &str, world_based: bool) -> Self {
        Self {
            name: name.to_string(),
            world_based,
        }
    }
}

/// Source of the emoji image URL. Upstream subclasses:
/// Emoji (default, emojiful-assets), EmojiFromGithub (own `url` field).
/// EmojiFromTwitmoji / EmojiFromEmojipedia only customize texture upload, so
/// they stay on the default URL here and their upload halves are seams.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EmojiSource {
    Default,
    Github { url: String },
}

#[derive(Debug, Clone)]
pub struct Emoji {
    pub name: String,
    pub strings: Vec<String>,
    pub texts: Vec<String>,
    pub location: String,
    pub version: i64,
    pub sort: i64,
    pub world_based: bool,
    pub source: EmojiSource,
    pub delete_old_texture: bool,
    pub finished_loading: bool,
    pub loaded_textures: bool,
}

impl Default for Emoji {
    fn default() -> Self {
        Self {
            name: String::new(),
            strings: Vec::new(),
            texts: Vec::new(),
            location: String::new(),
            version: 1,
            sort: 0,
            world_based: false,
            source: EmojiSource::Default,
            delete_old_texture: false,
            finished_loading: false,
            loaded_textures: false,
        }
    }
}

impl Emoji {
    /// `Predicate<String>`: case-insensitive match against any binding string.
    pub fn test(&self, s: &str) -> bool {
        self.strings.iter().any(|text| text.eq_ignore_ascii_case(s))
    }

    pub fn world_based(&self) -> bool {
        self.world_based
    }

    /// First strictly-shorter string wins, matching upstream's scan order.
    pub fn shorter_string(&self) -> String {
        let mut short = self.strings[0].clone();
        for string in &self.strings {
            if string.chars().count() < short.chars().count() {
                short = string.clone();
            }
        }
        short
    }

    /// Upstream: word boundaries only around letters (a-z case-insensitively,
    /// checked against the lowercased first/last characters), then each entry
    /// is cleaned for regex and the list joined with `|`.
    pub fn regex_string(&self) -> String {
        let mut processed: Vec<String> = Vec::new();
        for string in &self.strings {
            let lower = string.to_lowercase();
            let mut s = string.clone();
            if let Some(last) = lower.chars().last() {
                if last.is_ascii_lowercase() {
                    s.push_str("\\b");
                }
            }
            if let Some(first) = lower.chars().next() {
                if first.is_ascii_lowercase() {
                    s = format!("\\b{}", s);
                }
            }
            processed.push(clean_string_for_regex(&s));
        }
        processed.join("|")
    }

    /// Upstream wraps the cleaned texts in whitespace lookarounds so only
    /// whole-word text emoticons match. The pattern string is kept verbatim;
    /// the consuming regex engine is a game-side seam (Rust's `regex` crate
    /// has no lookbehind, so the game side matches this with its own engine).
    pub fn text_regex(&self) -> String {
        let processed: Vec<String> = self
            .texts
            .iter()
            .map(|t| clean_string_for_regex(t))
            .collect();
        format!("(?<=^|\\s)({})(?=$|\\s)", processed.join("|"))
    }

    pub fn url(&self) -> String {
        match &self.source {
            EmojiSource::Default => format!("{}{}", ASSETS_BASE, self.location),
            EmojiSource::Github { url } => url.clone(),
        }
    }

    /// Upstream cache path: emojiful/cache/<name>-<version> (working dir relative).
    pub fn cache_path(&self) -> String {
        format!("emojiful/cache/{}-{}", self.name, self.version)
    }

    /// Texture id sanitizer: `name.toLowerCase().replaceAll("[^a-z0-9/._-]", "")`.
    pub fn sanitized_name(&self) -> String {
        self.name
            .to_lowercase()
            .chars()
            .filter(|c| {
                c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '/' | '.' | '_' | '-')
            })
            .collect()
    }
}

/// EmojiUtil.renderEmoji quad geometry (upload/render is the seam).
pub fn emoji_quad_uvs() -> [(f32, f32); 4] {
    let texture_size = TEXTURE_SIZE;
    let texture_x = 0.0 / texture_size;
    let texture_y = 0.0 / texture_size;
    let texture_offset = TEXTURE_SIZE / texture_size;
    [
        (texture_x, texture_y),
        (texture_x, texture_y + texture_offset),
        (texture_x + texture_offset, texture_y + texture_offset),
        (texture_x + texture_offset, texture_y / texture_size),
    ]
}

/// EmojiUtil.splitGif result model: upstream emits (BufferedImage, delayTime)
/// pairs; the decode and metadata XML walk are platform glue, the delay bookkeeping
/// (frames repeated `delay` times when building the animation strip) is data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GifFrame {
    pub width: u32,
    pub height: u32,
    /// Upstream GraphicControlExtension delayTime, defaulting to 1 when absent.
    pub delay: u32,
}

/// GifFrame expansion used when building the frame strip: each frame repeats
/// `delay` times (upstream `for (Integer integer = 0; integer < delay; integer++)`).
pub fn gif_frame_strip(frames: &[GifFrame]) -> Vec<usize> {
    let mut strip = Vec::new();
    for (index, frame) in frames.iter().enumerate() {
        for _ in 0..frame.delay {
            strip.push(index);
        }
    }
    strip
}

// ---------------------------------------------------------------------------
// EmojiUtil.cleanStringForRegex
// ---------------------------------------------------------------------------

/// Upstream: escape `) ( | *` only, leaving every other character untouched.
pub fn clean_string_for_regex(string: &str) -> String {
    let mut out = String::with_capacity(string.len());
    for c in string.chars() {
        match c {
            ')' => out.push_str("\\)"),
            '(' => out.push_str("\\("),
            '|' => out.push_str("\\|"),
            '*' => out.push_str("\\*"),
            c => out.push(c),
        }
    }
    out
}

// ---------------------------------------------------------------------------
// ProfanityFilter
// ---------------------------------------------------------------------------

#[derive(Debug, Default, Clone)]
pub struct ProfanityFilter {
    pub words: HashMap<String, Vec<String>>,
    pub largest_word_length: usize,
}

/// Upstream loads from the fixed Google-Sheets CSV export; the HTTP fetch is a
/// game-side seam, the CSV parsing below is 1:1.
pub const PROFANITY_CSV_URL: &str = "https://docs.google.com/spreadsheets/d/1Ufoero85kpr4caXLLaPpcgwB4tX44GgoGJ4F-bVfdI8/export?format=csv";

impl ProfanityFilter {
    /// Returns the number of CSV lines read (upstream's counter), loading
    /// every word and its ignore-in-combination list.
    pub fn load_from_csv(&mut self, csv: &str) -> usize {
        let mut counter = 0usize;
        for line in csv.lines() {
            counter += 1;
            let content: Vec<&str> = line.split(',').collect();
            if content.is_empty() {
                continue;
            }
            let word = content[0];
            let mut ignore_in_combination_with_words: Vec<String> = Vec::new();
            if content.len() > 1 {
                ignore_in_combination_with_words = {
                    let mut parts: Vec<String> =
                        content[1].split('_').map(|s| s.to_string()).collect();
                    while parts.last().map(|p| p.is_empty()).unwrap_or(false) {
                        parts.pop();
                    }
                    parts
                };
            }
            if word.chars().count() > self.largest_word_length {
                self.largest_word_length = word.chars().count();
            }
            self.words
                .insert(word.replace(' ', ""), ignore_in_combination_with_words);
        }
        counter
    }

    /// Substring scan over each space-separated word with leet normalization.
    /// Indices are char-based in the port; upstream uses UTF-16 units, which
    /// only differs for inputs containing astral characters (noted in README).
    pub fn bad_words_found(&self, input: Option<&str>) -> Vec<String> {
        let input = match input {
            None => return Vec::new(),
            Some(i) => i,
        };
        let mut bad_words = Vec::new();
        for word in input.split(' ') {
            let chars: Vec<char> = word.chars().collect();
            let len = chars.len();
            for start in 0..len {
                let mut offset = 1usize;
                while offset < len + 1 - start && offset < self.largest_word_length {
                    let end = start + offset;
                    if word.is_char_boundary(start) && word.is_char_boundary(end) {
                        let substring: String = chars[start..end].iter().collect();
                        if let Some(original) = word.get(start..end) {
                            if self
                                .words
                                .contains_key(&normalize_profanity_key(&substring))
                            {
                                let ignore_check =
                                    &self.words[&normalize_profanity_key(&substring)];
                                let ignore = ignore_check.iter().any(|w| input.contains(w));
                                if !ignore {
                                    bad_words.push(original.to_string());
                                }
                            }
                        }
                    }
                    offset += 1;
                }
            }
        }
        bad_words
    }

    /// Upstream replaceAll(cleanStringForRegex(badWord), replacement). The port
    /// replaces the bad word literally: the upstream escape set `) ( | *` is
    /// handled identically, while words containing other regex metacharacters
    /// would have blown up into accidental regex semantics in upstream Java.
    pub fn filter_text(&self, input: &str, replacement: &str) -> String {
        let bad_words = self.bad_words_found(Some(input));
        let mut result = input.to_string();
        if !bad_words.is_empty() {
            for bad_word in bad_words {
                result = result.replace(clean_string_for_regex(&bad_word).as_str(), replacement);
            }
        }
        result
    }
}

fn normalize_profanity_key(word: &str) -> String {
    let mut normalized = String::with_capacity(word.len());
    for c in word.chars() {
        let mapped = match c {
            '1' => 'i',
            '!' => 'i',
            '3' => 'e',
            '4' => 'a',
            '@' => 'a',
            '5' => 's',
            '7' => 't',
            '0' => 'o',
            '9' => 'g',
            c => c,
        };
        normalized.push(mapped);
    }
    normalized
        .to_lowercase()
        .chars()
        .filter(|c| c.is_ascii_alphabetic())
        .collect()
}

// ---------------------------------------------------------------------------
// Minimal YAML readers for the emojiful-assets package format
// ---------------------------------------------------------------------------

/// Categories.yml: a list of quoted file names (`- "Discord.yml"`).
pub fn yaml_list_of_strings(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim();
        let Some(item) = trimmed.strip_prefix("- ") else {
            continue;
        };
        out.push(item.trim().trim_matches('"').trim_matches('\'').to_string());
    }
    out
}

/// A custom emoji package file: a YAML list of objects, each with scalar
/// fields (`name`, `location`, optional `version`/`sort`) and optional string
/// list fields (`strings`, `texts`).
pub fn yaml_read_emojis(text: &str) -> Vec<Emoji> {
    let mut emojis = Vec::new();
    let mut current: Option<Emoji> = None;
    let mut last_list_key: Option<String> = None;
    for raw in text.lines() {
        if raw.trim().is_empty() {
            continue;
        }
        if let Some(rest) = raw.strip_prefix("- ") {
            if let Some(emoji) = current.take() {
                emojis.push(emoji);
            }
            current = Some(Emoji::default());
            last_list_key = None;
            if let Some((key, value)) = split_yaml_kv(rest) {
                apply_yaml_field(current.as_mut().unwrap(), &key, &value, &mut last_list_key);
            }
        } else if let Some(item) = raw.trim_start().strip_prefix("- ") {
            // Nested list item under the last seen list key.
            if let (Some(emoji), Some(key)) = (current.as_mut(), last_list_key.clone()) {
                let value = item.trim().trim_matches('"').trim_matches('\'').to_string();
                match key.as_str() {
                    "strings" => emoji.strings.push(value),
                    "texts" => emoji.texts.push(value),
                    _ => {}
                }
            }
        } else if raw.starts_with(char::is_whitespace) {
            if let Some((key, value)) = split_yaml_kv(raw.trim()) {
                if let Some(emoji) = current.as_mut() {
                    apply_yaml_field(emoji, &key, &value, &mut last_list_key);
                }
            }
        }
    }
    if let Some(emoji) = current.take() {
        emojis.push(emoji);
    }
    emojis
}

fn split_yaml_kv(rest: &str) -> Option<(String, String)> {
    let (key, value) = rest.split_once(':')?;
    Some((key.trim().to_string(), value.trim().to_string()))
}

fn apply_yaml_field(emoji: &mut Emoji, key: &str, value: &str, last_list_key: &mut Option<String>) {
    let unquoted = value.trim_matches('"').trim_matches('\'');
    match key {
        "name" => emoji.name = unquoted.to_string(),
        "location" => emoji.location = unquoted.to_string(),
        "version" => emoji.version = unquoted.parse().unwrap_or(1),
        "sort" => emoji.sort = unquoted.parse().unwrap_or(0),
        "strings" => {
            *last_list_key = Some("strings".to_string());
            if !unquoted.is_empty() {
                emoji.strings.push(unquoted.to_string());
            }
        }
        "texts" => {
            *last_list_key = Some("texts".to_string());
            if !unquoted.is_empty() {
                emoji.texts.push(unquoted.to_string());
            }
        }
        _ => {}
    }
}

// ---------------------------------------------------------------------------
// Datapack recipe model (datapack/EmojiRecipe.java)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub struct EmojiRecipe {
    pub recipe_name: String,
    pub category: String,
    pub name: String,
    pub url: String,
}

// ---------------------------------------------------------------------------
// GLFW keys used by CommonClass.shouldKeyBeIgnored
// ---------------------------------------------------------------------------

pub const GLFW_KEY_TAB: i32 = 258;
pub const GLFW_KEY_UP: i32 = 265;
pub const GLFW_KEY_DOWN: i32 = 264;
pub const GLFW_KEY_LEFT: i32 = 263;
pub const GLFW_KEY_RIGHT: i32 = 262;

pub fn should_key_be_ignored(key_code: i32) -> bool {
    key_code == GLFW_KEY_TAB
        || key_code == GLFW_KEY_UP
        || key_code == GLFW_KEY_DOWN
        || key_code == GLFW_KEY_LEFT
        || key_code == GLFW_KEY_RIGHT
}

// ---------------------------------------------------------------------------
// ClientEmojiHandler
// ---------------------------------------------------------------------------

#[derive(Debug, Default)]
pub struct ClientEmojiHandler {
    pub config: Config,
    pub categories: Vec<EmojiCategory>,
    pub emoji_map: HashMap<String, Vec<Emoji>>,
    pub emoji_list: Vec<Emoji>,
    pub all_emojis: Vec<String>,
    /// Indices into `emoji_list` (upstream keeps the emoji references directly).
    pub emoji_with_texts: Vec<usize>,
    /// Category -> rows of 9 selection slots, in LinkedHashMap order.
    pub sorted_emojis_for_selection: Vec<(EmojiCategory, Vec<Vec<Option<Emoji>>>)>,
    pub line_amount: usize,
    pub error: bool,
    pub profanity: ProfanityFilter,
}

impl ClientEmojiHandler {
    /// ClientEmojiHandler.setup(): the initEmojis font/signed-renderer swap is
    /// the game-side seam; everything it guards (indexing, loading) runs here.
    pub fn setup(&mut self, fetch: &dyn Fn(&str) -> String) {
        self.pre_init_emojis(fetch);
        self.init_emojis();
        self.index_emojis();
    }

    pub fn index_emojis(&mut self) {
        self.all_emojis = self
            .emoji_list
            .iter()
            .flat_map(|e| e.strings.clone())
            .collect();
        self.sorted_emojis_for_selection = Vec::new();
        self.line_amount = 0;
        for category in &self.categories {
            self.line_amount += 1;
            let emojis = self
                .emoji_map
                .get(&category.name)
                .cloned()
                .unwrap_or_default();
            let mut rows: Vec<Vec<Option<Emoji>>> = Vec::new();
            let mut array: Vec<Option<Emoji>> = vec![None; SELECTOR_COLUMNS];
            let mut i = 0usize;
            for emoji in emojis {
                array[i] = Some(emoji);
                i += 1;
                if i >= array.len() {
                    rows.push(std::mem::replace(&mut array, vec![None; SELECTOR_COLUMNS]));
                    i = 0;
                    self.line_amount += 1;
                }
            }
            if i > 0 {
                rows.push(array);
                self.line_amount += 1;
            }
            self.sorted_emojis_for_selection
                .push((category.clone(), rows));
        }
    }

    fn pre_init_emojis(&mut self, fetch: &dyn Fn(&str) -> String) {
        for name in [
            "Smileys & Emotion",
            "Animals & Nature",
            "Food & Drink",
            "Activities",
            "Travel & Places",
            "Objects",
            "Symbols",
            "Flags",
        ] {
            self.categories.push(EmojiCategory::new(name, false));
        }
        if self.config.load_custom {
            self.load_custom_emojis(fetch);
        }
        if self.config.enable_load_twemoji {
            self.load_twemojis(&fetch(TWEMOJI_DATA_URL));
        }
        if self.config.profanity_filter {
            let csv = fetch(PROFANITY_CSV_URL);
            self.profanity.load_from_csv(&csv);
        }
    }

    /// On any failure upstream sets Constants.error and logs; here `error`
    /// flips and the load stops short, matching the guard in initEmojis.
    fn load_custom_emojis(&mut self, fetch: &dyn Fn(&str) -> String) {
        let categories_text = fetch(CATEGORIES_URL);
        let categories = yaml_list_of_strings(&categories_text);
        if categories.is_empty() {
            self.error = true;
            return;
        }
        for category in categories {
            let name = category.replace(".yml", "");
            self.categories.insert(0, EmojiCategory::new(&name, false));
            let emojis = yaml_read_emojis(&fetch(&format!("{}{}", ASSETS_BASE, category)));
            self.emoji_list.extend(emojis.clone());
            self.emoji_map.insert(name, emojis);
        }
    }

    /// Upstream loadTwemojis(): filter has_img_twitter, wrap short_names in
    /// colons, alias :face_with_symbols_on_mouth: to :swear:, keep texts, and
    /// sort every category plus EMOJI_WITH_TEXTS by sort_order.
    pub fn load_twemojis(&mut self, json_text: &str) {
        let Ok(data) = serde_json::from_str::<Json>(json_text) else {
            self.error = true;
            return;
        };
        let Json::Array(entries) = data else {
            self.error = true;
            return;
        };
        for element in &entries {
            let Some(obj) = element.as_object() else {
                continue;
            };
            let has_img_twitter = obj
                .get("has_img_twitter")
                .and_then(Json::as_bool)
                .unwrap_or(false);
            if !has_img_twitter {
                continue;
            }
            let mut emoji = Emoji::default();
            emoji.name = obj
                .get("short_name")
                .and_then(Json::as_str)
                .unwrap_or("")
                .to_string();
            emoji.location = obj
                .get("image")
                .and_then(Json::as_str)
                .unwrap_or("")
                .to_string();
            emoji.sort = obj.get("sort_order").and_then(Json::as_i64).unwrap_or(0);
            if let Some(Json::Array(short_names)) = obj.get("short_names") {
                for name in short_names {
                    if let Some(name) = name.as_str() {
                        emoji.strings.push(format!(":{}:", name));
                    }
                }
            }
            if emoji
                .strings
                .contains(&":face_with_symbols_on_mouth:".to_string())
            {
                emoji.strings.push(":swear:".to_string());
            }
            if let Some(texts) = obj.get("texts") {
                if !texts.is_null() {
                    if let Some(Json::Array(texts)) = Some(texts) {
                        for text in texts {
                            if let Some(text) = text.as_str() {
                                emoji.texts.push(text.to_string());
                            }
                        }
                    }
                }
            }
            let category = obj
                .get("category")
                .and_then(Json::as_str)
                .unwrap_or("")
                .to_string();
            self.emoji_map
                .entry(category)
                .or_default()
                .push(emoji.clone());
            self.emoji_list.push(emoji.clone());
            if !emoji.texts.is_empty() {
                self.emoji_with_texts.push(self.emoji_list.len() - 1);
            }
        }
        self.emoji_with_texts
            .sort_by_key(|idx| self.emoji_list[*idx].sort);
        for emojis in self.emoji_map.values_mut() {
            emojis.sort_by_key(|e| e.sort);
        }
    }

    /// CommonClass.onRecipesUpdated: drop world-based state, load datapack
    /// emojis as Github-sourced, re-index. `recipes` is the game-side lookup
    /// (RecipeManager.getAllRecipesFor) seam.
    pub fn on_recipes_updated(&mut self, recipes: &[EmojiRecipe]) {
        self.categories.retain(|c| !c.world_based);
        self.emoji_list.retain(|e| !e.world_based());
        if self.config.load_datapack {
            for recipe in recipes {
                let mut emoji = Emoji::default();
                emoji.name = recipe.name.clone();
                emoji.strings = vec![format!(":{}:", recipe.name)];
                emoji.location = recipe.name.clone();
                emoji.world_based = true;
                emoji.source = EmojiSource::Github {
                    url: recipe.url.clone(),
                };
                self.emoji_map
                    .entry(recipe.category.clone())
                    .or_default()
                    .push(emoji.clone());
                self.emoji_list.push(emoji);
                let category_lower = recipe.category.to_lowercase();
                let already = self
                    .categories
                    .iter()
                    .any(|c| c.name.to_lowercase() == category_lower);
                if !already {
                    self.categories
                        .insert(0, EmojiCategory::new(&recipe.category, true));
                }
            }
            self.index_emojis();
        }
    }

    /// initEmojis: the font renderer swap (chat font, entity dispatcher font,
    /// sign renderer) is the game-side seam; upstream guards it on !error.
    fn init_emojis(&mut self) {
        if !self.error {
            // game-side seam: EmojiFontRenderer swap + SignRenderer re-register
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_defaults_match_upstream() {
        let c = Config::default();
        assert!(c.render_emoji);
        assert!(c.emoji_selector);
        assert!(c.emoji_autocomplete);
        assert!(c.gif_emojis);
        assert!(c.emoji_replacement);
        assert!(c.enable_load_twemoji);
        assert!(c.load_custom);
        assert!(c.load_datapack);
        assert!(!c.profanity_filter);
        assert_eq!(c.profanity_filter_replacement, ":swear:");
    }

    #[test]
    fn config_json_roundtrip_and_defaults() {
        let c = Config::default();
        assert_eq!(Config::from_json(&c.to_json()), c);
        // Missing keys fall back to MidnightConfig defaults.
        assert_eq!(Config::from_json("{}"), Config::default());
        // Broken file falls back to defaults.
        assert_eq!(Config::from_json("not json"), Config::default());
    }

    #[test]
    fn emoji_test_is_case_insensitive_over_strings() {
        let emoji = Emoji {
            strings: vec![":Pog:".into(), ":pog2:".into()],
            ..Default::default()
        };
        assert!(emoji.test(":pog:"));
        assert!(emoji.test(":POG:"));
        assert!(!emoji.test(":pog3:"));
    }

    #[test]
    fn shorter_string_takes_first_of_equal_length() {
        let emoji = Emoji {
            strings: vec![
                ":abcd:".into(),
                ":ab:".into(),
                ":abc:".into(),
                ":ab2:".into(),
            ],
            ..Default::default()
        };
        assert_eq!(emoji.shorter_string(), ":ab:");
    }

    #[test]
    fn regex_string_word_boundaries() {
        let mut emoji = Emoji::default();
        emoji.strings = vec!["pog".into()];
        assert_eq!(emoji.regex_string(), "\\bpog\\b");
        emoji.strings = vec![":pog:".into()];
        assert_eq!(emoji.regex_string(), ":pog:");
        emoji.strings = vec!["pog:".into(), "(:smile:)".into()];
        assert_eq!(emoji.regex_string(), "\\bpog:|\\(:smile:\\)");
    }

    #[test]
    fn text_regex_pattern_is_verbatim() {
        let mut emoji = Emoji::default();
        emoji.texts = vec![":)".to_string(), ":D".to_string()];
        assert_eq!(emoji.text_regex(), "(?<=^|\\s)(:\\)|:D)(?=$|\\s)");
    }

    #[test]
    fn github_source_url_overrides_default() {
        let mut emoji = Emoji::default();
        emoji.location = "assets/discord/thonk.png".into();
        assert_eq!(
            emoji.url(),
            format!("{}assets/discord/thonk.png", ASSETS_BASE)
        );
        emoji.source = EmojiSource::Github {
            url: "https://example.com/a.png".into(),
        };
        assert_eq!(emoji.url(), "https://example.com/a.png");
    }

    #[test]
    fn cache_path_and_sanitized_name() {
        let mut emoji = Emoji::default();
        emoji.name = "Thonk! Face".into();
        emoji.version = 2;
        assert_eq!(emoji.cache_path(), "emojiful/cache/Thonk! Face-2");
        assert_eq!(emoji.sanitized_name(), "thonkface");
    }

    #[test]
    fn clean_string_for_regex_escapes_only_four_chars() {
        assert_eq!(clean_string_for_regex("a(b)c|d*e.f"), "a\\(b\\)c\\|d\\*e.f");
    }

    #[test]
    fn gif_frame_strip_repeats_by_delay() {
        let frames = vec![
            GifFrame {
                width: 16,
                height: 16,
                delay: 3,
            },
            GifFrame {
                width: 16,
                height: 16,
                delay: 1,
            },
        ];
        assert_eq!(gif_frame_strip(&frames), vec![0, 0, 0, 1]);
    }

    #[test]
    fn emoji_quad_uvs_cover_full_texture() {
        let uvs = emoji_quad_uvs();
        assert_eq!(uvs[0], (0.0, 0.0));
        assert_eq!(uvs[2], (1.0, 1.0));
    }

    #[test]
    fn profanity_load_parses_csv_and_tracks_largest_word() {
        let mut filter = ProfanityFilter::default();
        let lines = filter.load_from_csv("motherfucker\nass,bass_\nshit\nfuck,_");
        assert_eq!(lines, 4);
        assert_eq!(filter.largest_word_length, 12);
        assert_eq!(filter.words["ass"], vec!["bass"]);
        assert_eq!(filter.words["shit"], Vec::<String>::new());
        assert_eq!(filter.words["fuck"], Vec::<String>::new());
    }

    #[test]
    fn profanity_detects_words_and_respects_ignore_combinations() {
        let mut filter = ProfanityFilter::default();
        filter.load_from_csv("motherfucker\nass,bass_\nshit\n");
        let found = filter.bad_words_found(Some("what the shit"));
        assert_eq!(found, vec!["shit"]);
        // "bass" contains the ignore word so "ass" inside it is not flagged.
        assert!(filter.bad_words_found(Some("bass")).is_empty());
        // standalone "ass" is flagged
        assert_eq!(filter.bad_words_found(Some("ass")), vec!["ass"]);
    }

    #[test]
    fn profanity_normalizes_leetspeak() {
        let mut filter = ProfanityFilter::default();
        filter.load_from_csv("motherfucker\nshit\nass\n");
        assert_eq!(filter.bad_words_found(Some("5h1t")), vec!["5h1t"]);
        assert_eq!(filter.bad_words_found(Some("@55")), vec!["@55"]);
        // upstream has no word-boundary guard on the substring scan, so "glass" flags "ass"
        assert_eq!(filter.bad_words_found(Some("glass")), vec!["ass"]);
    }

    #[test]
    fn profanity_filter_text_replaces_each_bad_word() {
        let mut filter = ProfanityFilter::default();
        filter.load_from_csv("motherfucker\nshit\nass\n");
        assert_eq!(
            filter.filter_text("that shit is ass", ":swear:"),
            "that :swear: is :swear:"
        );
        assert_eq!(filter.filter_text("clean text", ":swear:"), "clean text");
    }

    #[test]
    fn profanity_null_input_is_empty() {
        let filter = ProfanityFilter::default();
        assert!(filter.bad_words_found(None).is_empty());
    }

    #[test]
    fn twemoji_parse_filters_and_wraps_bindings() {
        let json = r#"[
          {"has_img_twitter": true, "short_name": "grinning", "image": "1f600.png",
           "sort_order": 10, "short_names": ["grinning", "smiley"], "texts": [":D"],
           "category": "Smileys & Emotion"},
          {"has_img_twitter": false, "short_name": "hidden", "image": "x.png",
           "sort_order": 1, "short_names": ["hidden"], "texts": null, "category": "Objects"}
        ]"#;
        let mut handler = ClientEmojiHandler::default();
        handler.load_twemojis(json);
        assert_eq!(handler.emoji_list.len(), 1);
        let emoji = &handler.emoji_list[0];
        assert_eq!(emoji.name, "grinning");
        assert_eq!(emoji.strings, vec![":grinning:", ":smiley:"]);
        assert_eq!(emoji.texts, vec![":D"]);
        assert_eq!(emoji.location, "1f600.png");
        assert_eq!(emoji.sort, 10);
        assert_eq!(handler.emoji_map["Smileys & Emotion"].len(), 1);
        assert_eq!(handler.emoji_with_texts.len(), 1);
        assert!(!handler.error);
    }

    #[test]
    fn twemoji_swear_alias_and_sort_order() {
        let json = r#"[
          {"has_img_twitter": true, "short_name": "face_with_symbols_on_mouth", "image": "1f92c.png",
           "sort_order": 5, "short_names": ["face_with_symbols_on_mouth"], "texts": null, "category": "Smileys & Emotion"},
          {"has_img_twitter": true, "short_name": "grinning", "image": "1f600.png",
           "sort_order": 2, "short_names": ["grinning"], "texts": null, "category": "Smileys & Emotion"}
        ]"#;
        let mut handler = ClientEmojiHandler::default();
        handler.load_twemojis(json);
        assert!(
            handler.emoji_list[0]
                .strings
                .contains(&":swear:".to_string())
        );
        // category sorted by sort_order descending-first? upstream sort ascending:
        let names: Vec<&str> = handler.emoji_map["Smileys & Emotion"]
            .iter()
            .map(|e| e.name.as_str())
            .collect();
        assert_eq!(names, vec!["grinning", "face_with_symbols_on_mouth"]);
    }

    #[test]
    fn twemoji_broken_json_sets_error() {
        let mut handler = ClientEmojiHandler::default();
        handler.load_twemojis("{not json");
        assert!(handler.error);
        assert!(handler.emoji_list.is_empty());
    }

    #[test]
    fn index_grid_nine_per_row_and_line_amount() {
        let mut handler = ClientEmojiHandler::default();
        for name in ["Smileys & Emotion", "Objects"] {
            handler.categories.push(EmojiCategory::new(name, false));
        }
        // Mirror the real load flow: emojis land in both the list and the map.
        for i in 0..10 {
            let emoji = Emoji {
                name: format!("e{}", i),
                strings: vec![format!(":e{}:", i)],
                ..Default::default()
            };
            handler
                .emoji_map
                .entry("Smileys & Emotion".into())
                .or_default()
                .push(emoji.clone());
            handler.emoji_list.push(emoji);
        }
        let obj = Emoji {
            name: "o0".into(),
            strings: vec![":o0:".into()],
            ..Default::default()
        };
        handler
            .emoji_map
            .entry("Objects".into())
            .or_default()
            .push(obj.clone());
        handler.emoji_list.push(obj);
        handler.index_emojis();
        assert_eq!(handler.all_emojis.len(), 11);
        let (cat, rows) = &handler.sorted_emojis_for_selection[0];
        assert_eq!(cat.name, "Smileys & Emotion");
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].len(), 9);
        assert_eq!(rows[1].len(), 9);
        assert!(rows[0][8].is_some());
        assert!(rows[1][0].is_some());
        assert!(rows[1][1].is_none());
        // line_amount: 1 per category + 1 per full row + 1 per remainder row
        assert_eq!(handler.line_amount, 2 + 1 + 1 + 1);
    }

    #[test]
    fn yaml_categories_list_matches_asset_format() {
        let categories = yaml_list_of_strings("- \"Discord.yml\"\n- \"Blobs.yml\"\n- \"Pepe.yml\"");
        assert_eq!(categories, vec!["Discord.yml", "Blobs.yml", "Pepe.yml"]);
    }

    #[test]
    fn yaml_read_emojis_matches_asset_format() {
        let yaml = "- name: thonk\n  strings:\n    - \":thonk:\"\n  location: \"assets/discord/thonk.png\"\n- name: pog\n  strings:\n    - \":pog:\"\n    - \":poggers:\"\n  texts:\n    - \"pog\"\n  location: \"pog.png\"\n  version: 2\n  sort: 3\n";
        let emojis = yaml_read_emojis(yaml);
        assert_eq!(emojis.len(), 2);
        assert_eq!(emojis[0].name, "thonk");
        assert_eq!(emojis[0].strings, vec![":thonk:"]);
        assert_eq!(emojis[0].location, "assets/discord/thonk.png");
        assert_eq!(emojis[0].version, 1);
        assert_eq!(emojis[1].strings, vec![":pog:", ":poggers:"]);
        assert_eq!(emojis[1].texts, vec!["pog"]);
        assert_eq!(emojis[1].version, 2);
        assert_eq!(emojis[1].sort, 3);
    }

    #[test]
    fn should_key_be_ignored_matches_upstream_keys() {
        assert!(should_key_be_ignored(GLFW_KEY_TAB));
        assert!(should_key_be_ignored(GLFW_KEY_UP));
        assert!(should_key_be_ignored(GLFW_KEY_DOWN));
        assert!(should_key_be_ignored(GLFW_KEY_LEFT));
        assert!(should_key_be_ignored(GLFW_KEY_RIGHT));
        assert!(!should_key_be_ignored(65));
    }

    #[test]
    fn datapack_flow_adds_world_based_github_emojis_and_reindexes() {
        let mut handler = ClientEmojiHandler::default();
        handler.config.load_datapack = true;
        // pre-existing world-based state is dropped first
        handler
            .categories
            .push(EmojiCategory::new("OldWorld", true));
        handler.emoji_list.push(Emoji {
            name: "old".into(),
            world_based: true,
            ..Default::default()
        });
        let recipes = vec![EmojiRecipe {
            recipe_name: "myserver:custom".into(),
            category: "MyServer".into(),
            name: "custom".into(),
            url: "https://example.com/custom.png".into(),
        }];
        handler.on_recipes_updated(&recipes);
        assert!(handler.categories.iter().all(|c| c.name != "OldWorld"));
        assert!(handler.emoji_list.iter().all(|e| e.name != "old"));
        let emoji = &handler.emoji_list[0];
        assert_eq!(emoji.name, "custom");
        assert_eq!(emoji.strings, vec![":custom:"]);
        assert!(emoji.world_based);
        assert_eq!(emoji.url(), "https://example.com/custom.png");
        assert!(
            handler
                .categories
                .contains(&EmojiCategory::new("MyServer", true))
        );
        assert!(!handler.all_emojis.is_empty());
    }

    #[test]
    fn datapack_flow_respects_config_gate() {
        let mut handler = ClientEmojiHandler::default();
        handler.config.load_datapack = false;
        let before = handler.emoji_list.len();
        handler.on_recipes_updated(&[]);
        assert_eq!(handler.emoji_list.len(), before);
    }

    #[test]
    fn setup_flow_gates_on_config() {
        let mut handler = ClientEmojiHandler::default();
        handler.config.load_custom = false;
        handler.config.enable_load_twemoji = false;
        handler.config.profanity_filter = false;
        handler.setup(&|_url| String::new());
        // 8 vanilla categories, no network loads
        assert_eq!(handler.categories.len(), 8);
        assert!(handler.emoji_list.is_empty());
        assert!(!handler.error);
    }
}
