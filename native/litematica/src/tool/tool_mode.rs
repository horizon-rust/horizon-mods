use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum ToolMode {
    #[default]
    AreaSelection,
    SchematicPlacement,
    EditSchematic,
    PasteSchematic,
    Fill,
    Delete,
}

impl ToolMode {
    pub fn name(&self) -> &'static str {
        match self {
            ToolMode::AreaSelection => "Area Selection",
            ToolMode::SchematicPlacement => "Schematic Placement",
            ToolMode::EditSchematic => "Edit Schematic",
            ToolMode::PasteSchematic => "Paste Schematic",
            ToolMode::Fill => "Fill Area",
            ToolMode::Delete => "Delete Area",
        }
    }

    pub fn next(&self) -> Self {
        match self {
            ToolMode::AreaSelection => ToolMode::SchematicPlacement,
            ToolMode::SchematicPlacement => ToolMode::EditSchematic,
            ToolMode::EditSchematic => ToolMode::PasteSchematic,
            ToolMode::PasteSchematic => ToolMode::Fill,
            ToolMode::Fill => ToolMode::Delete,
            ToolMode::Delete => ToolMode::AreaSelection,
        }
    }

    pub fn prev(&self) -> Self {
        match self {
            ToolMode::AreaSelection => ToolMode::Delete,
            ToolMode::SchematicPlacement => ToolMode::AreaSelection,
            ToolMode::EditSchematic => ToolMode::SchematicPlacement,
            ToolMode::PasteSchematic => ToolMode::EditSchematic,
            ToolMode::Fill => ToolMode::PasteSchematic,
            ToolMode::Delete => ToolMode::Fill,
        }
    }
}
