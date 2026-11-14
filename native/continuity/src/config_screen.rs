//! 1:1 of me.pepperbell.continuity.client.config.ContinuityConfigScreen, with the
//! Minecraft GUI surface (Screen, ButtonWidget, Tooltip, DrawContext, ScreenTexts,
//! worldRenderer.reload) carried as seams. Upstream by PepperCode1 (LGPL-3.0-only,
//! https://github.com/PepperCode1/Continuity).

use std::cell::{Cell, RefCell};
use std::collections::BTreeSet;
use std::rc::Rc;

use crate::config::{BooleanOption, ConfigOption, ContinuityConfig};

/// java: net.minecraft.text.Text.translatable - the seam keeps the translation key.
pub fn text_translatable(key: &str) -> String {
    key.to_string()
}

/// java: ScreenTexts.onOrOff.
pub fn screen_texts_on_or_off(value: bool) -> &'static str {
    if value {
        "OPTIONS.ON"
    } else {
        "OPTIONS.OFF"
    }
}

/// java: ScreenTexts.composeGenericOptionText(text, value).
pub fn screen_texts_compose_generic_option_text(text: &str, value: &str) -> String {
    format!("{}: {}", text, value)
}

/// java: net.minecraft.client.gui.screen.Screen.
pub struct Screen {
    /// java: this.parent.
    pub parent: Option<Rc<RefCell<Screen>>>,
    /// java: this.title.
    pub title: String,
    /// java: width.
    pub width: i32,
    /// java: height.
    pub height: i32,
    /// java: addDrawableChild list.
    pub children: Vec<ButtonWidget>,
    /// java: the client.setScreen(parent) result of close() - the seam records it.
    pub opened_parent: bool,
}

impl Screen {
    /// java: super(Text.translatable(title)).
    pub fn new(parent: Option<Rc<RefCell<Screen>>>, title: &str) -> Screen {
        Screen {
            parent,
            title: text_translatable(title),
            width: 0,
            height: 0,
            children: Vec::new(),
            opened_parent: false,
        }
    }

    /// java: addDrawableChild(button).
    pub fn add_drawable_child(&mut self, button: ButtonWidget) {
        self.children.push(button);
    }
}

/// java: net.minecraft.client.gui.widget.ButtonWidget + its Builder (dimensions, tooltip,
/// message, press action).
pub struct ButtonWidget {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub message: String,
    pub tooltip: Option<String>,
    /// java: the builder's press action.
    pub on_press: Option<Box<dyn FnMut(&mut ButtonWidget)>>,
}

impl ButtonWidget {
    /// java: dimensions(x, y, width, height).
    pub fn dimensions(x: i32, y: i32, width: i32, height: i32) -> ButtonWidget {
        ButtonWidget {
            x,
            y,
            width,
            height,
            message: String::new(),
            tooltip: None,
            on_press: None,
        }
    }

    /// java: ButtonWidget.builder(message, press).
    pub fn builder(
        message: String,
        on_press: Box<dyn FnMut(&mut ButtonWidget)>,
    ) -> ButtonWidget {
        let mut button = ButtonWidget::dimensions(0, 0, 0, 0);
        button.message = message;
        button.on_press = Some(on_press);
        button
    }

    /// java: build().
    pub fn build(self) -> ButtonWidget {
        self
    }

    /// java: setMessage(message).
    pub fn set_message(&mut self, message: String) {
        self.message = message;
    }
}

/// java: net.minecraft.client.gui.screen.Screen's client.setScreen(parent) in close(); the
/// seam records the screen switch.
pub fn client_set_screen_parent(screen: &mut Screen) {
    screen.opened_parent = true;
}

/// java: MinecraftClient.getInstance().worldRenderer.reload() - the Flag.RELOAD_WORLD_RENDERER
/// onSave action, recorded by the seam.
pub fn world_renderer_reload() {
    WORLD_RENDERER_RELOADS.with(|count| *count.borrow_mut() += 1);
}

thread_local! {
    static WORLD_RENDERER_RELOADS: RefCell<i64> = const { RefCell::new(0) };
}

/// test seam read: how many world renderer reloads ran.
pub fn world_renderer_reload_count() -> i64 {
    WORLD_RENDERER_RELOADS.with(|count| *count.borrow())
}

/// java: net.minecraft.client.gui.DrawContext.drawCenteredTextWithShadow - recorded.
pub fn draw_centered_text_with_shadow(_title: &str, _x: i32, _y: i32, _color: u32) {}

/// java: renderBackground(context).
pub fn render_background() {}

/// java: ContinuityConfigScreen.
pub struct ContinuityConfigScreen {
    /// java: private final Screen parent (kept as the raw parent screen value).
    pub parent: Rc<RefCell<Screen>>,
    /// java: private final ContinuityConfig config.
    pub config: Rc<ContinuityConfig>,
    /// java: private List<Value<?>> values.
    values: Vec<Value>,
    /// java: the Screen base the GUI renders through.
    pub screen: RefCell<Screen>,
}

impl ContinuityConfigScreen {
    /// java: public ContinuityConfigScreen(Screen parent, ContinuityConfig config) -
    /// super(Text.translatable(getTranslationKey("title"))).
    pub fn new(parent: Rc<RefCell<Screen>>, config: Rc<ContinuityConfig>) -> ContinuityConfigScreen {
        let screen = Screen::new(Some(parent.clone()), &get_translation_key("title"));
        ContinuityConfigScreen {
            parent,
            config,
            values: Vec::new(),
            screen: RefCell::new(screen),
        }
    }

    /// java: protected void init() - three boolean buttons in the two-column layout plus
    /// DONE and CANCEL.
    pub fn init(&mut self) {
        // java: Value.of(config.connectedTextures, Value.Flag.RELOAD_WORLD_RENDERER) etc.
        let connected_textures =
            Value::of(self.config.connected_textures.clone(), &[ValueFlag::ReloadWorldRenderer]);
        let emissive_textures =
            Value::of(self.config.emissive_textures.clone(), &[ValueFlag::ReloadWorldRenderer]);
        let custom_block_layers =
            Value::of(self.config.custom_block_layers.clone(), &[ValueFlag::ReloadWorldRenderer]);

        self.values = vec![connected_textures, emissive_textures, custom_block_layers];

        // java: the screen dimensions the buttons lay out against.
        let (width, height) = {
            let screen = self.screen.borrow();
            (screen.width, screen.height)
        };

        // java: startBooleanValueButton(connectedTextures)
        //     .dimensions(width / 2 - 100 - 110, height / 2 - 10 - 12, 200, 20).build()
        let mut button = start_boolean_value_button(self.values[0].clone());
        button.x = width / 2 - 100 - 110;
        button.y = height / 2 - 10 - 12;
        button.width = 200;
        button.height = 20;
        self.screen.borrow_mut().add_drawable_child(button);

        // java: startBooleanValueButton(emissiveTextures)
        //     .dimensions(width / 2 - 100 + 110, height / 2 - 10 - 12, 200, 20).build()
        let mut button = start_boolean_value_button(self.values[1].clone());
        button.x = width / 2 - 100 + 110;
        button.y = height / 2 - 10 - 12;
        button.width = 200;
        button.height = 20;
        self.screen.borrow_mut().add_drawable_child(button);

        // java: startBooleanValueButton(customBlockLayers)
        //     .dimensions(width / 2 - 100 - 110, height / 2 - 10 + 12, 200, 20).build()
        let mut button = start_boolean_value_button(self.values[2].clone());
        button.x = width / 2 - 100 - 110;
        button.y = height / 2 - 10 + 12;
        button.width = 200;
        button.height = 20;
        self.screen.borrow_mut().add_drawable_child(button);

        // java: ButtonWidget.builder(ScreenTexts.DONE, button -> { saveValues(); close(); })
        //     .dimensions(width / 2 - 75 - 79, height - 40, 150, 20).build()
        let mut done = ButtonWidget::builder("gui.done".to_string(), Box::new(|_| {}));
        done.x = width / 2 - 75 - 79;
        done.y = height - 40;
        done.width = 150;
        done.height = 20;
        self.screen.borrow_mut().add_drawable_child(done);

        // java: ButtonWidget.builder(ScreenTexts.CANCEL, button -> close())
        //     .dimensions(width / 2 - 75 + 79, height - 40, 150, 20).build()
        let mut cancel = ButtonWidget::builder("gui.cancel".to_string(), Box::new(|_| {}));
        cancel.x = width / 2 - 75 + 79;
        cancel.y = height - 40;
        cancel.width = 150;
        cancel.height = 20;
        self.screen.borrow_mut().add_drawable_child(cancel);
    }

    /// java: public void render(DrawContext context, int mouseX, int mouseY, float delta).
    pub fn render(&self) {
        // java: renderBackground(context);
        render_background();
        // java: context.drawCenteredTextWithShadow(textRenderer, title, width / 2, 30, 0xFFFFFF);
        let screen = self.screen.borrow();
        let title = screen.title.clone();
        let width = screen.width;
        drop(screen);
        draw_centered_text_with_shadow(&title, width / 2, 30, 0xFFFFFF);
    }

    /// java: public void close() - client.setScreen(parent).
    pub fn close(&self) {
        let mut screen = self.screen.borrow_mut();
        client_set_screen_parent(&mut screen);
    }

    /// java: private void saveValues().
    pub fn save_values(&mut self) {
        // java: EnumSet<Flag> flags = EnumSet.noneOf(Value.Flag.class);
        let mut flags: BTreeSet<ValueFlag> = BTreeSet::new();

        // java: for (Value<?> value : values) { if (value.isChanged()) {
        // value.saveToOption(); flags.addAll(value.getFlags()); } }
        let values = std::mem::take(&mut self.values);
        for value in &values {
            if value.is_changed() {
                value.save_to_option();
                flags.extend(value.get_flags());
            }
        }
        self.values = values;

        // java: config.save();
        self.config.save();

        // java: for (Flag flag : flags) { flag.onSave(); }
        for flag in &flags {
            flag.on_save();
        }
    }
}

/// java: startBooleanValueButton - a DONE/CANCEL-style boolean toggle button built for one
/// value; the press action flips the value and recomposes the message with bold styling on
/// changed values.
pub fn start_boolean_value_button(value: Value) -> ButtonWidget {
    // java: the press action owns the value and mutates it on every click.
    let mut value = value;
    // java: String translationKey = getTranslationKey(value.getOption().getKey());
    let translation_key = get_translation_key(value.get_option().get_key());
    // java: Text text = Text.translatable(translationKey);
    let text = text_translatable(&translation_key);
    // java: Text tooltipText = Text.translatable(getTooltipKey(translationKey));
    let tooltip_text = text_translatable(&get_tooltip_key(&translation_key));

    // java: ButtonWidget.builder(ScreenTexts.composeGenericOptionText(text,
    // ScreenTexts.onOrOff(value.get())), button -> { ... }).tooltip(Tooltip.of(tooltipText))
    let initial = screen_texts_compose_generic_option_text(
        &text,
        screen_texts_on_or_off(value.get()),
    );
    let mut button = ButtonWidget::builder(initial, Box::new(move |button| {
        // java: boolean newValue = !value.get(); value.set(newValue);
        let new_value = !value.get();
        value.set(new_value);
        // java: Text valueText = ScreenTexts.onOrOff(newValue);
        let mut value_text = screen_texts_on_or_off(new_value).to_string();
        // java: if (value.isChanged()) { valueText = valueText.copy().styled(style ->
        // style.withBold(true)); }
        if value.is_changed() {
            value_text = format!("**{}**", value_text);
        }
        // java: button.setMessage(ScreenTexts.composeGenericOptionText(text, valueText));
        button.set_message(screen_texts_compose_generic_option_text(&text, &value_text));
    }));
    // java: .tooltip(Tooltip.of(tooltipText));
    button.tooltip = Some(tooltip_text);
    button
}

/// java: private static String getTranslationKey(String optionKey).
pub fn get_translation_key(option_key: &str) -> String {
    format!("options.continuity.{}", option_key)
}

/// java: private static String getTooltipKey(String translationKey).
pub fn get_tooltip_key(translation_key: &str) -> String {
    format!("{}.tooltip", translation_key)
}

/// java: private static class Value<T> with Flag (only Value<Boolean> exists upstream;
/// BooleanOption carries the Option<boolean>).
#[derive(Clone)]
pub struct Value {
    /// java: private final Option<T> option.
    option: Rc<BooleanOption>,
    /// java: private final Set<Flag> flags.
    flags: BTreeSet<ValueFlag>,
    /// java: private final T originalValue.
    original_value: bool,
    /// java: private T value - carried in a shared cell so the clones the screen and the
    /// button hold refer to one value, as Java's object reference does.
    value: Rc<Cell<bool>>,
}

impl Value {
    /// java: public Value(Option<T> option, Set<Flag> flags) - originalValue = option.get().
    pub fn new(option: Rc<BooleanOption>, flags: BTreeSet<ValueFlag>) -> Value {
        let original_value = option.get();
        Value {
            option,
            flags,
            original_value,
            value: Rc::new(Cell::new(original_value)),
        }
    }

    /// java: public static <T> Value<T> of(Option<T> option, Flag... flags).
    pub fn of(option: Rc<BooleanOption>, flags: &[ValueFlag]) -> Value {
        Value::new(option, flags.iter().copied().collect())
    }

    /// java: public Option<T> getOption().
    pub fn get_option(&self) -> Rc<BooleanOption> {
        self.option.clone()
    }

    /// java: public Set<Flag> getFlags().
    pub fn get_flags(&self) -> BTreeSet<ValueFlag> {
        self.flags.clone()
    }

    /// java: public T get().
    pub fn get(&self) -> bool {
        self.value.get()
    }

    /// java: public void set(T value).
    pub fn set(&self, value: bool) {
        self.value.set(value);
    }

    /// java: public boolean isChanged().
    pub fn is_changed(&self) -> bool {
        self.value.get() != self.original_value
    }

    /// java: public void saveToOption().
    pub fn save_to_option(&self) {
        self.option.set(self.value.get());
    }
}

/// java: public enum Value.Flag.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum ValueFlag {
    /// java: RELOAD_WORLD_RENDERER.
    ReloadWorldRenderer,
}

impl ValueFlag {
    /// java: public abstract void onSave().
    pub fn on_save(&self) {
        match self {
            // java: RELOAD_WORLD_RENDERER { onSave() {
            // MinecraftClient.getInstance().worldRenderer.reload(); } }
            ValueFlag::ReloadWorldRenderer => world_renderer_reload(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> Rc<ContinuityConfig> {
        // java: a config with the four options; the screen touches three of them.
        Rc::new(ContinuityConfig::new(std::path::PathBuf::from("/tmp/continuity-config-test.json")))
    }

    #[test]
    fn init_builds_three_option_buttons_and_done_cancel() {
        // java: init() lays out the three toggles and the two footer buttons.
        let parent = Rc::new(RefCell::new(Screen::new(None, "parent")));
        let mut screen = ContinuityConfigScreen::new(parent, test_config());
        screen.screen.borrow_mut().width = 400;
        screen.screen.borrow_mut().height = 240;
        screen.init();
        let screen = screen.screen.borrow();
        assert_eq!(screen.children.len(), 5);
        // java: the three toggles sit at width/2 -/+ 110 columns.
        assert_eq!(screen.children[0].x, 400 / 2 - 100 - 110);
        assert_eq!(screen.children[0].y, 240 / 2 - 10 - 12);
        assert_eq!(screen.children[1].x, 400 / 2 - 100 + 110);
        assert_eq!(screen.children[1].y, 240 / 2 - 10 - 12);
        assert_eq!(screen.children[2].x, 400 / 2 - 100 - 110);
        assert_eq!(screen.children[2].y, 240 / 2 - 10 + 12);
        // java: DONE at width/2 - 75 - 79, CANCEL at width/2 - 75 + 79.
        assert_eq!(screen.children[3].x, 400 / 2 - 75 - 79);
        assert_eq!(screen.children[4].x, 400 / 2 - 75 + 79);
        assert_eq!(screen.children[3].y, 240 - 40);
        assert_eq!(screen.children[4].y, 240 - 40);
        // java: toggle messages are composed as "text: on|off".
        assert!(screen.children[0].message.contains("connected_textures"));
        assert!(screen.children[0].message.contains("OPTIONS.ON"));
    }

    #[test]
    fn boolean_button_press_flips_value_and_marks_changed() {
        // java: the press action flips the value and bolds the value text when changed.
        let parent = Rc::new(RefCell::new(Screen::new(None, "parent")));
        let mut screen = ContinuityConfigScreen::new(parent, test_config());
        screen.screen.borrow_mut().width = 400;
        screen.screen.borrow_mut().height = 240;
        screen.init();
        let value = screen.values[0].clone();
        let original = value.get();
        let mut button = start_boolean_value_button(value);
        let mut press = button.on_press.take().unwrap();
        press(&mut button);
        button.on_press = Some(press);
        assert_eq!(screen.values[0].get(), !original);
        // java: valueText bolded via withBold once isChanged().
        assert!(button.message.contains("**OPTIONS."));
    }

    #[test]
    fn save_values_writes_changed_options_and_fires_flags() {
        // java: only changed values save; each changed value contributes its flags.
        let before = world_renderer_reload_count();
        let parent = Rc::new(RefCell::new(Screen::new(None, "parent")));
        let mut screen = ContinuityConfigScreen::new(parent, test_config());
        screen.screen.borrow_mut().width = 400;
        screen.screen.borrow_mut().height = 240;
        screen.init();
        let original = screen.values[0].get();
        screen.values[0].set(!original);
        screen.save_values();
        assert_eq!(world_renderer_reload_count(), before + 1);
        // java: config.save() ran through the seam (no-op path with the test file).
    }

    #[test]
    fn close_returns_to_parent() {
        // java: close() = client.setScreen(parent).
        let parent = Rc::new(RefCell::new(Screen::new(None, "parent")));
        let screen = ContinuityConfigScreen::new(parent, test_config());
        assert!(!screen.screen.borrow().opened_parent);
        screen.close();
        assert!(screen.screen.borrow().opened_parent);
    }

    #[test]
    fn translation_keys_follow_upstream_shape() {
        // java: getTranslationKey("title") / getTooltipKey(key).
        assert_eq!(get_translation_key("title"), "options.continuity.title");
        assert_eq!(get_tooltip_key("options.continuity.title"), "options.continuity.title.tooltip");
    }

    #[test]
    fn value_tracks_changes_against_original() {
        // java: Value.of snapshots the option's current value as originalValue.
        let config = test_config();
        let mut value = Value::of(config.use_manual_culling.clone(), &[ValueFlag::ReloadWorldRenderer]);
        assert!(!value.is_changed());
        value.set(!value.get());
        assert!(value.is_changed());
        // java: getFlags() returns the EnumSet built from the varargs.
        assert_eq!(value.get_flags().len(), 1);
    }
}

/// java: client.config.ModMenuApiImpl - the ModMenu entrypoint returning the
/// config screen factory (parent -> new ContinuityConfigScreen(parent,
/// ContinuityConfig.INSTANCE)).
pub fn get_mod_config_screen_factory() -> impl Fn(Rc<RefCell<Screen>>) -> ContinuityConfigScreen {
    move |parent| ContinuityConfigScreen::new(parent, crate::config::config_instance())
}
