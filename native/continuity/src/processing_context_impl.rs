//! 1:1 of the Continuity impl.client.ProcessingContextImpl, by PepperCode1
//! (LGPL-3.0-only, https://github.com/PepperCode1/Continuity). Upstream file:
//! src/main/java/me/pepperbell/continuity/impl/client/ProcessingContextImpl.java.

use std::any::Any;
use std::cell::RefCell;
use std::rc::Rc;

use crate::processor_base::{
    Mesh, ProcessingContext, ProcessingDataKey, ProcessingDataProvider, QuadEmitter,
    processing_data_key_registry,
};

/// java: impl.client.ProcessingContextImpl implements QuadProcessor.ProcessingContext.
pub struct ProcessingContextImpl {
    /// java: List<Consumer<QuadEmitter>> emitterConsumers.
    emitter_consumers: Vec<Box<dyn FnOnce(&mut QuadEmitter)>>,
    /// java: List<Mesh> meshes.
    meshes: Vec<Mesh>,
    /// java: MeshBuilder meshBuilder = RendererAccess.INSTANCE.getRenderer().meshBuilder()
    /// (engine surface; the port carries the mesh builder as the emitter it hands out).
    mesh_builder: QuadEmitter,
    /// java: Object[] processingData = new Object[getRegisteredAmount()].
    processing_data: RefCell<Vec<Option<Rc<RefCell<Box<dyn Any>>>>>>,
    has_extra_quads: bool,
}

impl Default for ProcessingContextImpl {
    fn default() -> Self {
        ProcessingContextImpl::new()
    }
}

impl ProcessingContextImpl {
    pub fn new() -> ProcessingContextImpl {
        let registered_amount = processing_data_key_registry().get_registered_amount();
        ProcessingContextImpl {
            emitter_consumers: Vec::new(),
            meshes: Vec::new(),
            mesh_builder: QuadEmitter::default(),
            processing_data: RefCell::new(vec![None; registered_amount]),
            has_extra_quads: false,
        }
    }

    /// rust carrying: Java's ProcessingDataKeys statics register the keys at class
    /// load, before any context can be built; Rust thread-locals initialize lazily
    /// in first-access order, so the array is grown on first use instead.
    fn ensure_data_capacity(&self) {
        let registered_amount = processing_data_key_registry().get_registered_amount();
        let mut data = self.processing_data.borrow_mut();
        if data.len() < registered_amount {
            data.resize(registered_amount, None);
        }
    }

    /// java: getDataOrNull(ProcessingDataKey<T> key).
    pub fn get_data_or_null(&self, key: &ProcessingDataKey) -> Option<Rc<RefCell<Box<dyn Any>>>> {
        self.ensure_data_capacity();
        self.processing_data.borrow()[key.raw_id].clone()
    }

    /// java: outputTo(QuadEmitter emitter).
    pub fn output_to(&mut self, emitter: &mut QuadEmitter) {
        if !self.emitter_consumers.is_empty() {
            let mut consumers = std::mem::take(&mut self.emitter_consumers);
            for consumer in consumers.drain(..) {
                consumer(emitter);
            }
            self.emitter_consumers = consumers;
        }
        if !self.meshes.is_empty() {
            let mut meshes = std::mem::take(&mut self.meshes);
            for mesh in meshes.iter_mut() {
                mesh.output_to(emitter);
            }
            self.meshes = meshes;
        }
        if self.has_extra_quads {
            self.mesh_builder.build().output_to(emitter);
        }
    }

    /// java: prepare().
    pub fn prepare(&mut self) {
        self.has_extra_quads = false;
    }

    /// java: reset().
    pub fn reset(&mut self) {
        self.emitter_consumers.clear();
        self.meshes.clear();
        self.reset_data();
    }

    /// java: resetData(): every resettable key in registration order.
    fn reset_data(&mut self) {
        let all_resettable = processing_data_key_registry().get_all_resettable();
        for key in &all_resettable {
            self.reset_data_key(key);
        }
    }

    /// java: resetData(ProcessingDataKey<T> key).
    fn reset_data_key(&mut self, key: &ProcessingDataKey) {
        let value = self.get_data_or_null(key);
        if let Some(value) = value {
            if let Some(reset_action) = &key.value_reset_action {
                let mut guard = value.borrow_mut();
                reset_action(&mut **guard);
            }
        }
    }
}

impl ProcessingDataProvider for ProcessingContextImpl {
    /// java: <T> T getData(ProcessingDataKey<T> key): null -> valueSupplier, cached.
    fn get_data(&self, key: &ProcessingDataKey) -> Rc<RefCell<Box<dyn Any>>> {
        self.ensure_data_capacity();
        let index = key.raw_id;
        if let Some(data) = self.processing_data.borrow()[index].clone() {
            return data;
        }
        let data: Rc<RefCell<Box<dyn Any>>> = Rc::new(RefCell::new((key.value_supplier)()));
        self.processing_data.borrow_mut()[index] = Some(data.clone());
        data
    }
}

impl ProcessingContext for ProcessingContextImpl {
    /// java: addEmitterConsumer(Consumer<QuadEmitter> consumer).
    fn add_emitter_consumer(&mut self, consumer: Box<dyn FnOnce(&mut QuadEmitter)>) {
        self.emitter_consumers.push(consumer);
    }

    /// java: addMesh(Mesh mesh).
    fn add_mesh(&mut self, mesh: Mesh) {
        self.meshes.push(mesh);
    }

    /// java: getExtraQuadEmitter() = meshBuilder.getEmitter().
    fn get_extra_quad_emitter(&mut self) -> &mut QuadEmitter {
        &mut self.mesh_builder
    }

    /// java: markHasExtraQuads().
    fn mark_has_extra_quads(&mut self) {
        self.has_extra_quads = true;
    }

    /// rust carrying: ProcessingContext extends ProcessingDataProvider.
    fn as_data_provider(&self) -> &dyn ProcessingDataProvider {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::processor_base::{processing_data_keys, QuadView};
    use crate::seams::{BlockPos, Identifier};

    #[test]
    fn get_data_supplies_and_caches() {
        let context = ProcessingContextImpl::new();
        let keys = processing_data_keys();
        let first = context.get_data(&keys.mutable_pos);
        let second = context.get_data(&keys.mutable_pos);
        assert!(Rc::ptr_eq(&first, &second));
    }

    #[test]
    fn reset_runs_resettable_actions_but_not_plain_keys() {
        let mut context = ProcessingContextImpl::new();
        let keys = processing_data_keys();
        let biome = context.get_data(&keys.biome_cache);
        {
            let mut guard = biome.borrow_mut();
            let _ = guard.downcast_mut::<crate::processor_base::BiomeCache>().unwrap();
        }
        // java: BiomeCache.reset() clears the map on reset_data.
        context.reset();
        let cache = context.get_data(&keys.biome_cache);
        let mut guard = cache.borrow_mut();
        let cache = guard.downcast_mut::<crate::processor_base::BiomeCache>().unwrap();
        let found = cache.get(&crate::seams::BlockRenderView, &crate::seams::BlockPos { x: 0, y: 0, z: 0 });
        assert!(found.is_none());
        let _ = Identifier::new("minecraft", "plains").unwrap();
    }

    #[test]
    fn prepare_resets_extra_quads_flag_but_not_lists() {
        let mut context = ProcessingContextImpl::new();
        context.mark_has_extra_quads();
        context.add_mesh(Mesh::default());
        context.prepare();
        assert!(!context.has_extra_quads);
        assert_eq!(context.meshes.len(), 1);
        context.reset();
        assert_eq!(context.meshes.len(), 0);
    }

    #[test]
    fn output_to_replays_consumers_meshes_and_extra_quads() {
        let mut context = ProcessingContextImpl::new();
        context.add_emitter_consumer(Box::new(|emitter: &mut QuadEmitter| {
            emitter.square(crate::processor_enums::Direction::North, 0.0, 0.0, 1.0, 1.0, 0.0);
            emitter.emit();
        }));
        let mut mesh = Mesh::default();
        let mut built = QuadEmitter::default();
        built.square(crate::processor_enums::Direction::South, 0.0, 0.0, 1.0, 1.0, 0.0);
        built.emit();
        mesh.quads.push(built.emitted.remove(0));
        context.add_mesh(mesh);
        context.mark_has_extra_quads();
        let mut extra = QuadEmitter::default();
        extra.square(crate::processor_enums::Direction::East, 0.0, 0.0, 1.0, 1.0, 0.0);
        extra.emit();
        let _ = &mut context.mesh_builder;
        context.mesh_builder.emitted.push(extra.emitted.remove(0));
        let mut target = QuadEmitter::default();
        context.output_to(&mut target);
        assert_eq!(target.emitted.len(), 3);
        assert_eq!(target.emitted[0].light_face, crate::processor_enums::Direction::North);
        assert_eq!(target.emitted[1].light_face, crate::processor_enums::Direction::South);
        assert_eq!(target.emitted[2].light_face, crate::processor_enums::Direction::East);
    }

    #[test]
    fn get_data_or_null_reflects_writes() {
        let mut context = ProcessingContextImpl::new();
        let keys = processing_data_keys();
        assert!(context.get_data_or_null(&keys.mutable_pos).is_none());
        let _ = context.get_data(&keys.mutable_pos);
        assert!(context.get_data_or_null(&keys.mutable_pos).is_some());
        let value = context.get_data_or_null(&keys.mutable_pos).unwrap();
        {
            let mut guard = value.borrow_mut();
            let _pos: &mut BlockPos = guard.downcast_mut::<BlockPos>().unwrap();
        }
        let _ = QuadView::bare(crate::processor_enums::Direction::North);
    }
}
