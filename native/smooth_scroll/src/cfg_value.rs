//! Upstream cfg/CfgValue.java, 1:1.
//!
//! Java stores the value as Object; here it is the Value enum. Booleans and
//! doubles are the only boxed types the translation map ever keys on, which
//! keeps the upstream quirk where its Double map keys never match the Float
//! values looked up (see try_translate).

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::{Rc, Weak};

use serde_json::Value as Json;

use crate::menu::custom_slider::CustomSlider;
use crate::seams::{Button, Component};
use crate::cfg_value_builder::CfgValueBuilder;

/// The widget handle kinds upstream checks with instanceof.
#[derive(Clone)]
pub enum WidgetHandle {
    Slider(Rc<RefCell<CustomSlider>>),
    Button(Rc<RefCell<Button>>),
    /// StringWidget: the section headers the config screen adds.
    Label(Component),
}

impl WidgetHandle {
    pub fn set_tooltip(&self, tooltip: Component) {
        match self {
            WidgetHandle::Slider(s) => *s.borrow().base.base.tooltip.borrow_mut() = Some(tooltip),
            WidgetHandle::Button(b) => b.borrow().set_tooltip(tooltip),
            WidgetHandle::Label(_) => {}
        }
    }
    pub fn set_active(&self, active: bool) {
        match self {
            WidgetHandle::Slider(s) => s.borrow().base.base.active.set(active),
            WidgetHandle::Button(b) => b.borrow().base.active.set(active),
            WidgetHandle::Label(_) => {}
        }
    }
    pub fn set_width(&self, width: i32) {
        match self {
            WidgetHandle::Slider(s) => s.borrow().base.base.width.set(width),
            WidgetHandle::Button(b) => b.borrow().set_width(width),
            WidgetHandle::Label(_) => {}
        }
    }
}

/// A CfgValue node, shared exactly like the Java references (children, parent,
/// widgets and screens all hold the same object).
pub struct CfgValue {
    pub(crate) default_value: Value,
    pub(crate) current_value: RefCell<Value>,
    pub(crate) value_name: String,
    pub(crate) is_fake: Cell<bool>,
    pub(crate) parent: RefCell<Option<Weak<RefCell<CfgValue>>>>,

    // GUI menu options
    pub(crate) min_val: Cell<f32>,
    pub(crate) max_val: Cell<f32>,
    pub(crate) step: Cell<f64>,
    pub(crate) temporary_value: RefCell<Value>,
    pub(crate) my_widget: RefCell<Option<WidgetHandle>>,
    pub(crate) my_reset_button: RefCell<Option<Rc<RefCell<Button>>>>,
    pub(crate) tooltiptxt: RefCell<Option<Component>>,
    pub(crate) translation_map: RefCell<HashMap<MapKey, String>>,
    pub(crate) format_key: RefCell<String>,
    pub(crate) translation_key: RefCell<Option<String>>,
    pub(crate) disable_when: RefCell<Option<Rc<dyn Fn(&Rc<RefCell<CfgValue>>) -> bool>>>,
}

/// Key of translationMap. Java boxes Boolean / Double objects; a Float value
/// looked up later can never equal a boxed Double key in Java, so Float values
/// keep their own kind here and miss on purpose (upstream quirk kept 1:1).
#[derive(Clone, Copy, Debug)]
pub enum MapKey {
    B(bool),
    D(f64),
}

impl PartialEq for MapKey {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (MapKey::B(a), MapKey::B(b)) => a == b,
            (MapKey::D(a), MapKey::D(b)) => a == b,
            _ => false,
        }
    }
}
impl Eq for MapKey {}

impl std::hash::Hash for MapKey {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        match self {
            MapKey::B(v) => {
                state.write_u8(0);
                state.write_u8(*v as u8);
            }
            MapKey::D(v) => {
                state.write_u8(1);
                state.write_u64(v.to_bits());
            }
        }
    }
}

#[derive(Clone, Default)]
pub enum Value {
    #[default]
    Null,
    Bool(bool),
    /// Upstream Number storage; parseJson and arithmetic store Float.
    Num(f32),
    Str(String),
    /// List<String> (the Notes block).
    Strs(Vec<String>),
    /// List<CfgValue>.
    Cfgs(Vec<Rc<RefCell<CfgValue>>>),
}

/// Java's equals()/toString() semantics for the boxed values. Lists compare
/// by element identity, the way holding references does.
impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Value::Null, Value::Null) => true,
            (Value::Bool(a), Value::Bool(b)) => a == b,
            (Value::Num(a), Value::Num(b)) => a == b,
            (Value::Str(a), Value::Str(b)) => a == b,
            (Value::Strs(a), Value::Strs(b)) => a == b,
            (Value::Cfgs(a), Value::Cfgs(b)) => {
                a.len() == b.len() && a.iter().zip(b).all(|(x, y)| Rc::ptr_eq(x, y))
            }
            _ => false,
        }
    }
}

impl std::fmt::Debug for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Value::Null => write!(f, "null"),
            Value::Bool(v) => write!(f, "{}", v),
            Value::Num(v) => write!(f, "{}", v),
            Value::Str(s) => write!(f, "{:?}", s),
            Value::Strs(l) => write!(f, "{:?}", l),
            Value::Cfgs(l) => write!(f, "Cfgs({})", l.len()),
        }
    }
}

/// Java value.getClass() equality: every List is the same class upstream.
pub(crate) fn class_of(v: &Value) -> u8 {
    match v {
        Value::Null => 4,
        Value::Bool(_) => 0,
        Value::Num(_) => 1,
        Value::Str(_) => 2,
        Value::Strs(_) | Value::Cfgs(_) => 3,
    }
}

impl CfgValue {
    pub fn new(name: &str, default_val: Value) -> Self {
        Self {
            current_value: RefCell::new(default_val.clone()),
            default_value: default_val.clone(),
            value_name: name.to_string(),
            is_fake: Cell::new(false),
            parent: RefCell::new(None),
            min_val: Cell::new(0.0),
            max_val: Cell::new(0.0),
            step: Cell::new(0.01),
            temporary_value: RefCell::new(default_val.clone()),
            my_widget: RefCell::new(None),
            my_reset_button: RefCell::new(None),
            tooltiptxt: RefCell::new(None),
            translation_map: RefCell::new(HashMap::from([
                (MapKey::B(true), "options.on".to_string()),
                (MapKey::B(false), "options.off".to_string()),
            ])),
            format_key: RefCell::new("smoothscroll.config.format.default".to_string()),
            translation_key: RefCell::new(None),
            disable_when: RefCell::new(None),
        }
    }

    /// Links the children of a Cfgs default to this node, as the Java
    /// constructor does. Runs after the node is behind an Rc.
    pub(crate) fn link_children(self_: &Rc<RefCell<CfgValue>>) {
        if let Some(list) = CfgValue::get_list(self_) {
            for cfg_value in list {
                *cfg_value.borrow().parent.borrow_mut() = Some(Rc::downgrade(self_));
            }
        }
    }

    pub fn builder(name: &str, default_val: Value) -> CfgValueBuilder {
        CfgValueBuilder::new(name, default_val)
    }

    /// get(String): ".." climbs to the parent, names look inside a CfgValue
    /// list, anything else returns a fake instance.
    pub fn get(self_: &Rc<RefCell<CfgValue>>, name: &str) -> Rc<RefCell<CfgValue>> {
        if name == ".." {
            let parent = self_.borrow().parent.borrow().clone();
            return match parent.and_then(|p| p.upgrade()) {
                Some(p) => p,
                None => CfgValue::make_fake(),
            };
        }

        let cfg_value_list = match CfgValue::get_list(self_) {
            Some(l) => l,
            None => return CfgValue::make_fake(),
        };

        for cfg_value in cfg_value_list {
            if cfg_value.borrow().value_name == name {
                return cfg_value;
            }
        }

        CfgValue::make_fake()
    }

    /// getList(): the list only when it is a CfgValue list.
    pub fn get_list(self_: &Rc<RefCell<CfgValue>>) -> Option<Vec<Rc<RefCell<CfgValue>>>> {
        let cur = self_.borrow().current_value.borrow().clone();
        match cur {
            Value::Cfgs(list) => Some(list),
            _ => None,
        }
    }

    pub fn get_name(self_: &Rc<RefCell<CfgValue>>) -> String {
        self_.borrow().value_name.clone()
    }

    pub fn get_value(self_: &Rc<RefCell<CfgValue>>) -> Value {
        self_.borrow().current_value.borrow().clone()
    }

    pub fn exists(self_: &Rc<RefCell<CfgValue>>) -> bool {
        !self_.borrow().is_fake.get()
    }

    pub fn make_fake() -> Rc<RefCell<CfgValue>> {
        let ret = Rc::new(RefCell::new(CfgValue::new("", Value::Null)));
        ret.borrow().is_fake.set(true);
        ret
    }

    pub fn set_value(self_: &Rc<RefCell<CfgValue>>, value: Value) {
        let b = self_.borrow_mut();
        *b.current_value.borrow_mut() = value.clone();
        *b.temporary_value.borrow_mut() = value;
    }

    pub fn set_temp_value(self_: &Rc<RefCell<CfgValue>>, value: Value) {
        *self_.borrow().temporary_value.borrow_mut() = value;
    }

    pub fn get_temp_value(self_: &Rc<RefCell<CfgValue>>) -> Value {
        self_.borrow().temporary_value.borrow().clone()
    }

    pub fn save_temp_value(self_: &Rc<RefCell<CfgValue>>) {
        let temp = self_.borrow().temporary_value.borrow().clone();
        *self_.borrow().current_value.borrow_mut() = temp;
    }

    pub fn reset_temp_value(self_: &Rc<RefCell<CfgValue>>) {
        let cur = self_.borrow().current_value.borrow().clone();
        *self_.borrow().temporary_value.borrow_mut() = cur;
    }

    pub fn default_to_temp(self_: &Rc<RefCell<CfgValue>>) {
        let default = self_.borrow().default_value.clone();
        *self_.borrow().temporary_value.borrow_mut() = default;
        let widget = self_.borrow().my_widget.borrow().clone();
        if let Some(widget) = widget {
            match widget {
                WidgetHandle::Slider(cs) => cs.borrow().refresh_value(),
                WidgetHandle::Button(bw) => {
                    let text = CfgValue::make_button_text(self_);
                    bw.borrow().set_message(text);
                }
                WidgetHandle::Label(_) => {}
            }
        }
    }

    pub fn recursive_save_temp_value(self_: &Rc<RefCell<CfgValue>>) {
        let a = CfgValue::get_list(self_);
        if let Some(a) = a {
            for cfg_value2 in a {
                CfgValue::recursive_save_temp_value(&cfg_value2);
            }
            return;
        }
        CfgValue::save_temp_value(self_);
    }

    pub fn generate_widget(
        self_: &Rc<RefCell<CfgValue>>,
    ) -> [Option<WidgetHandle>; 2] {
        let weak = Rc::downgrade(self_);
        let reset_button = Button::builder(
            Component::translatable("controls.reset", &[]),
            move |_button| {
                if let Some(self_) = weak.upgrade() {
                    CfgValue::default_to_temp(&self_);
                }
            },
        )
        .build();
        *self_.borrow().my_reset_button.borrow_mut() = Some(reset_button.clone());

        let value = CfgValue::get_value(self_);
        let my_widget = match value {
            Value::Num(_) => {
                let cs = CustomSlider::new(self_);
                let handle = WidgetHandle::Slider(cs.clone());
                CfgValue::assign_widget(self_, handle.clone());
                if let Some(tooltiptxt) = self_.borrow().tooltiptxt.borrow().clone() {
                    handle.set_tooltip(tooltiptxt);
                }
                Some(handle)
            }
            Value::Bool(_) => {
                let text = CfgValue::make_button_text(self_);
                let weak = Rc::downgrade(self_);
                let b = Button::builder(
                    text,
                    move |button| {
                        if let Some(self_) = weak.upgrade() {
                            let t = CfgValue::get_temp_value(&self_);
                            let toggled = match t {
                                Value::Bool(v) => Value::Bool(!v),
                                other => other,
                            };
                            CfgValue::set_temp_value(&self_, toggled);
                            button.borrow().set_message(CfgValue::make_button_text(&self_));
                        }
                    },
                )
                .build();
                let handle = WidgetHandle::Button(b);
                if let Some(tooltiptxt) = self_.borrow().tooltiptxt.borrow().clone() {
                    handle.set_tooltip(tooltiptxt);
                }
                CfgValue::assign_widget(self_, handle.clone());
                Some(handle)
            }
            _ => {
                // the fallthrough: a plain button with the current text and no action
                let b = Button::builder(CfgValue::make_button_text(self_), |_| {}).build();
                Some(WidgetHandle::Button(b))
            }
        };
        [my_widget, Some(WidgetHandle::Button(reset_button))]
    }

    fn make_button_text(self_: &Rc<RefCell<CfgValue>>) -> Component {
        let display = CfgValue::get_display_name(self_).text();
        let translated = CfgValue::try_translate(self_, &CfgValue::get_temp_value(self_));
        let format_key = self_.borrow().format_key.borrow().clone();
        Component::translatable(&format_key, &[&display, &translated.text()])
    }

    pub fn parse_json(name: &str, json_data: Option<Json>) -> Rc<RefCell<CfgValue>> {
        let a = Rc::new(RefCell::new(CfgValue::new(name, Value::Null)));
        let Some(json_data) = json_data else { return a };
        match json_data {
            Json::Object(obj_map) => {
                let mut b: Vec<Rc<RefCell<CfgValue>>> = Vec::new();
                for (element_name, element) in obj_map {
                    b.push(CfgValue::parse_json(&element_name, Some(element)));
                }
                CfgValue::set_value(&a, Value::Cfgs(b));
            }
            // TODO handle jsonarray (upstream leaves arrays unhandled too)
            Json::String(s) => CfgValue::set_value(&a, Value::Str(s)),
            Json::Bool(v) => CfgValue::set_value(&a, Value::Bool(v)),
            Json::Number(number) => {
                CfgValue::set_value(&a, Value::Num(number.as_f64().unwrap_or(0.0) as f32))
            }
            _ => {}
        }
        a
    }

    pub fn match_values(
        source: &Rc<RefCell<CfgValue>>,
        destination: &Rc<RefCell<CfgValue>>,
    ) {
        let source_value = CfgValue::get_value(source);
        if source_value == Value::Null {
            return;
        }
        let destination_value = CfgValue::get_value(destination);
        if class_of(&destination_value) != class_of(&source_value) {
            return;
        }
        let a = CfgValue::get_list(source);
        let b = CfgValue::get_list(destination);
        if a.is_none() && b.is_none() {
            CfgValue::set_value(destination, source_value);
        }

        if a.is_none() || b.is_none() {
            return;
        }
        for destval in b.unwrap() {
            for srcval in a.clone().unwrap() {
                if destval.borrow().value_name == srcval.borrow().value_name {
                    CfgValue::match_values(&srcval, &destval);
                }
            }
        }
    }

    pub fn export_json(self_: &Rc<RefCell<CfgValue>>) -> Json {
        if let Some(a) = CfgValue::get_list(self_) {
            let mut b = serde_json::Map::new();
            for cfg_value in a {
                let name = cfg_value.borrow().value_name.clone();
                b.insert(name, CfgValue::export_json(&cfg_value));
            }
            return Json::Object(b);
        }
        match CfgValue::get_value(self_) {
            Value::Null => Json::Null,
            Value::Bool(v) => Json::Bool(v),
            Value::Num(v) => serde_json::Number::from_f64(v as f64)
                .map(Json::Number)
                .unwrap_or(Json::Null),
            Value::Str(s) => Json::String(s),
            Value::Strs(list) => Json::Array(list.into_iter().map(Json::String).collect()),
            Value::Cfgs(_) => unreachable!("handled above"),
        }
    }

    pub fn to_string(self_: &Rc<RefCell<CfgValue>>) -> String {
        let b = self_.borrow();
        format!("({}:{:?})", b.value_name, *b.current_value.borrow())
    }

    pub fn get_step(self_: &Rc<RefCell<CfgValue>>) -> f64 {
        self_.borrow().step.get()
    }

    pub fn assign_widget(self_: &Rc<RefCell<CfgValue>>, cw: WidgetHandle) {
        *self_.borrow().my_widget.borrow_mut() = Some(cw);
    }

    pub fn get_max(self_: &Rc<RefCell<CfgValue>>) -> f64 {
        self_.borrow().max_val.get() as f64
    }

    pub fn get_min(self_: &Rc<RefCell<CfgValue>>) -> f64 {
        self_.borrow().min_val.get() as f64
    }

    pub fn get_format_key(self_: &Rc<RefCell<CfgValue>>) -> String {
        self_.borrow().format_key.borrow().clone()
    }

    pub fn try_translate(self_: &Rc<RefCell<CfgValue>>, value: &Value) -> Component {
        let key = match value {
            Value::Bool(v) => self_.borrow().translation_map.borrow().get(&MapKey::B(*v)).cloned(),
            // Float values cannot match the boxed Double keys upstream.
            Value::Num(_) => None,
            _ => None,
        };
        if let Some(key) = key {
            return Component::translatable(&key, &[]);
        }
        Component::literal(&format!("{}", ValueDisplay(value)))
    }

    pub fn set_translation_key(self_: &Rc<RefCell<CfgValue>>, key: &str) {
        *self_.borrow().translation_key.borrow_mut() = Some(key.to_string());
    }

    pub fn set_format_key(self_: &Rc<RefCell<CfgValue>>, key: &str) {
        *self_.borrow().format_key.borrow_mut() = key.to_string();
    }

    pub fn get_display_name(self_: &Rc<RefCell<CfgValue>>) -> Component {
        let key = self_.borrow().translation_key.borrow().clone();
        if let Some(key) = key {
            return Component::translatable(&key, &[]);
        }
        let name = self_.borrow().value_name.clone();
        Component::literal(&name)
    }

    pub fn refresh_disable_recursive(self_: &Rc<RefCell<CfgValue>>) {
        let list = CfgValue::get_list(self_);
        if list.is_none() {
            let disable_when = self_.borrow().disable_when.borrow().clone();
            let Some(disable_when) = disable_when else { return };
            let self_rc = self_.clone();
            let should_be_disabled = disable_when(&self_rc);
            let widget = self_.borrow().my_widget.borrow().clone();
            if let Some(widget) = widget {
                widget.set_active(!should_be_disabled);
            }
            let reset = self_.borrow().my_reset_button.borrow().clone();
            if let Some(reset) = reset {
                reset.borrow().base.active.set(!should_be_disabled);
            }
            return;
        }
        for cfg_value in list.unwrap() {
            CfgValue::refresh_disable_recursive(&cfg_value);
        }
    }

    pub fn en_helf_step(self_: &Rc<RefCell<CfgValue>>, val: f64) -> i64 {
        (val / CfgValue::get_step(self_)).round() as i64
    }
    pub fn en_step(self_: &Rc<RefCell<CfgValue>>, val: f64) -> f64 {
        CfgValue::en_helf_step(self_, val) as f64 * CfgValue::get_step(self_)
    }
    pub fn en_min_max(self_: &Rc<RefCell<CfgValue>>, val: f64) -> f64 {
        val * (CfgValue::get_max(self_) - CfgValue::get_min(self_)) + CfgValue::get_min(self_)
    }
    pub fn de_min_max(self_: &Rc<RefCell<CfgValue>>, val: f64) -> f64 {
        (val - CfgValue::get_min(self_)) / CfgValue::get_max(self_)
    }
}

/// Object.toString-style rendering for values inside tryTranslate.
struct ValueDisplay<'a>(&'a Value);
impl std::fmt::Display for ValueDisplay<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.0 {
            Value::Null => write!(f, "null"),
            Value::Bool(v) => write!(f, "{}", v),
            Value::Num(v) => write!(f, "{}", v),
            Value::Str(s) => write!(f, "{}", s),
            Value::Strs(_) | Value::Cfgs(_) => write!(f, "[list]"),
        }
    }
}
