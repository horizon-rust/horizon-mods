//! Upstream versionless/mixinbase/ModelPartBase.java, 1:1: the mixin-base
//! surface the mixins bolt onto model parts.

/// versionless/mixinbase/ModelPartBase.java.
pub trait ModelPartBase {
    fn set_hidden(&self);
    fn show_again(&self);
    fn is_hidden(&self) -> bool;
}
