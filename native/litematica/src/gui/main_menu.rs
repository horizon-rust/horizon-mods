use malilib::gui::{Button, GuiScreen, Label, Widget, Color};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MainMenuTab {
    #[default]
    SchematicPlacements,
    LoadedSchematics,
    SchematicBrowser,
    AreaSelection,
    Verifier,
    MaterialList,
    Configuration,
}

#[derive(Debug)]
pub struct LitematicaMainMenu {
    pub current_tab: MainMenuTab,
    pub title: String,
    pub widgets: Vec<Box<dyn Widget>>,
}

impl Default for LitematicaMainMenu {
    fn default() -> Self {
        let mut menu = Self {
            current_tab: MainMenuTab::SchematicPlacements,
            title: "Litematica Main Menu".to_string(),
            widgets: Vec::new(),
        };
        menu.rebuild_widgets();
        menu
    }
}

impl LitematicaMainMenu {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_tab(&mut self, tab: MainMenuTab) {
        self.current_tab = tab;
        self.rebuild_widgets();
    }

    pub fn rebuild_widgets(&mut self) {
        self.widgets.clear();

        // Title
        let title_label = Label::new(20.0, 15.0, format!("Litematica - {:?}", self.current_tab), Color::WHITE);
        self.widgets.push(Box::new(title_label));

        // Navigation Tab Buttons across the top
        let tabs = [
            (MainMenuTab::SchematicPlacements, "Placements"),
            (MainMenuTab::LoadedSchematics, "Loaded"),
            (MainMenuTab::SchematicBrowser, "Browser"),
            (MainMenuTab::AreaSelection, "Area Selection"),
            (MainMenuTab::Verifier, "Verifier"),
            (MainMenuTab::MaterialList, "Materials"),
            (MainMenuTab::Configuration, "Config"),
        ];

        let mut x = 20.0;
        let y = 35.0;
        let button_w = 90.0;
        let button_h = 20.0;

        for (tab, label) in tabs {
            let mut btn = Button::new(x, y, button_w, button_h, label);
            if tab == self.current_tab {
                btn.enabled = false;
            }
            self.widgets.push(Box::new(btn));
            x += button_w + 5.0;
        }

        // Add action buttons based on selected tab:
        match self.current_tab {
            MainMenuTab::SchematicPlacements => {
                self.widgets.push(Box::new(Button::new(20.0, 70.0, 120.0, 20.0, "Configure Selected")));
                self.widgets.push(Box::new(Button::new(145.0, 70.0, 100.0, 20.0, "Unload Selected")));
                self.widgets.push(Box::new(Button::new(250.0, 70.0, 100.0, 20.0, "Lock / Unlock")));
            }
            MainMenuTab::LoadedSchematics => {
                self.widgets.push(Box::new(Button::new(20.0, 70.0, 120.0, 20.0, "Create Placement")));
                self.widgets.push(Box::new(Button::new(145.0, 70.0, 100.0, 20.0, "Unload from Memory")));
            }
            MainMenuTab::SchematicBrowser => {
                self.widgets.push(Box::new(Button::new(20.0, 70.0, 120.0, 20.0, "Load Schematic")));
                self.widgets.push(Box::new(Button::new(145.0, 70.0, 100.0, 20.0, "Refresh Directory")));
            }
            MainMenuTab::AreaSelection => {
                self.widgets.push(Box::new(Button::new(20.0, 70.0, 120.0, 20.0, "Save to Schematic")));
                self.widgets.push(Box::new(Button::new(145.0, 70.0, 100.0, 20.0, "Clear Selection")));
            }
            MainMenuTab::Verifier => {
                self.widgets.push(Box::new(Button::new(20.0, 70.0, 120.0, 20.0, "Start Verification")));
                self.widgets.push(Box::new(Button::new(145.0, 70.0, 100.0, 20.0, "Clear Results")));
            }
            MainMenuTab::MaterialList => {
                self.widgets.push(Box::new(Button::new(20.0, 70.0, 120.0, 20.0, "Refresh Counts")));
                self.widgets.push(Box::new(Button::new(145.0, 70.0, 100.0, 20.0, "Export to File")));
            }
            MainMenuTab::Configuration => {
                self.widgets.push(Box::new(Button::new(20.0, 70.0, 120.0, 20.0, "Reset Defaults")));
                self.widgets.push(Box::new(Button::new(145.0, 70.0, 100.0, 20.0, "Save Config")));
            }
        }
    }
}

impl GuiScreen for LitematicaMainMenu {
    fn title(&self) -> &str {
        &self.title
    }

    fn widgets(&self) -> &[Box<dyn Widget>] {
        &self.widgets
    }

    fn widgets_mut(&mut self) -> &mut [Box<dyn Widget>] {
        &mut self.widgets
    }
}
