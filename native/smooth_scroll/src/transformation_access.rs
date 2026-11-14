//! Upstream TransformationAccess.java: the access-injected transformation
//! parameters on the host renderer plus its context.

use std::rc::Rc;

use crate::seams::{GuiGraphicsSeam, ScreenRectangle};

/// Upstream net.minecraft.client.gui.ActiveTextCollector.Parameters: the
/// fields TransformationMixin shadows (pose, opacity, scissor).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Parameters {
    /// Upstream Matrix3x2fc pose.
    pub pose: [f32; 6],
    pub opacity: f32,
    pub scissor: Option<ScreenRectangle>,
}

impl Parameters {
    /// Upstream new Parameters(pose, opacity, scissor).
    pub fn new(pose: [f32; 6], opacity: f32, scissor: Option<ScreenRectangle>) -> Self {
        Self { pose, opacity, scissor }
    }

    /// The host's original withScissor, the operation the mixin wraps: a copy
    /// of these parameters carrying the new scissor.
    pub fn with_scissor(&self, scissor: Option<ScreenRectangle>) -> Self {
        Parameters::new(self.pose, self.opacity, scissor)
    }
}

pub trait TransformationAccess {
    fn get_transformation(&self) -> Parameters;
    fn set_transformation(&mut self, transformation: Parameters);
    fn get_context(&self) -> Rc<dyn GuiGraphicsSeam>;
}
