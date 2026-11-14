//! 1:1 of ContinuityClient.onInitializeClient's CtmLoaderRegistry loader registrations,
//! plus the createLoader/createCustomLoader helpers that build them
//! (impl/client/CtmLoaderRegistryImpl wiring). Upstream by PepperCode1 (LGPL-3.0-only,
//! https://github.com/PepperCode1/Continuity).

use std::cell::RefCell;
use std::rc::Rc;
use std::thread_local;

use crate::connecting_properties::{
    AtLeast, CtmProperties, CtmPropertiesFactory, Exactly, RepeatCtmProperties,
    RepeatValidator, TileAmountValidator, wrap_factory,
};
use crate::overlay_processors::{
    SimpleOverlayQuadProcessorFactory, StandardOverlayQuadProcessorFactory,
};
use crate::parsing::{parse_optifine_only, Properties};
use crate::processor_base::{
    AbstractProcessorProps, AbstractQuadProcessorFactory, BaseCachingPredicatesFactory,
    QuadProcessor, TextureGetter,
};
use crate::processor_compact::CompactCtmQuadProcessorFactory;
use crate::properties::BaseCtmProperties;
use crate::properties_loader::{CtmLoaderObj, CtmLoaderRegistry};
use crate::quad_processors::{SimpleQuadProcessorFactory, TopQuadProcessorFactory};
use crate::seams::{Identifier, RedirectHandler, ResourcePack, ResourceManager};
use crate::simple_sprite_provider::{
    CtmSpriteProviderFactory, FixedSpriteProviderFactory, HorizontalSpriteProviderFactory,
    HorizontalVerticalSpriteProviderFactory, RandomSpriteProviderFactory,
    RepeatSpriteProviderFactory, VerticalHorizontalSpriteProviderFactory,
    VerticalSpriteProviderFactory,
};

/// java: ContinuityClient.ID.
const ID: &str = "continuity";

thread_local! {
    static BUILTIN_RESOURCE_PACKS: RefCell<Vec<(String, String)>> =
        RefCell::new(Vec::new());
}

/// java: the anonymous CtmLoader<T> createLoader returns. The erased CtmLoaderObj impl
/// downcasts the shared properties back to T in the processor and predicates factories
/// (Rust dyn erasure takes the place of Java's generic factories).
pub struct CtmLoaderImpl<
    T: crate::properties_loader::CtmProperties + CtmProperties + AbstractProcessorProps + 'static,
> {
    /// java: getPropertiesFactory().
    properties_factory: CtmPropertiesFactory<T>,
    /// java: getProcessorFactory().
    processor_factory: Rc<dyn AbstractQuadProcessorFactory<T>>,
    /// java: getPredicatesFactory(); every upstream call site uses
    /// BaseCachingPredicates.Factory.
    predicates_factory: Rc<BaseCachingPredicatesFactory>,
}

impl<T> CtmLoaderObj for CtmLoaderImpl<T>
where
    T: crate::properties_loader::CtmProperties + CtmProperties + AbstractProcessorProps + 'static,
{
    /// java: CtmProperties.Factory<T>::createProperties(properties, resourceId, pack,
    /// packPriority, resourceManager, method); the loader's factory chain carries the
    /// validator and optifine-only wraps, and the port carries resourceManager as the
    /// redirect handler with packId read from the pack name.
    fn create_properties(
        &self,
        properties: &Properties,
        resource_id: &Identifier,
        pack: &ResourcePack,
        pack_priority: i32,
        resource_manager: &ResourceManager,
        method: &str,
    ) -> Option<Box<dyn crate::properties_loader::CtmProperties>> {
        // java: the properties resolve the handler off the resource manager; the port
        // hands the shared handler through the RedirectHandler seam.
        let redirect_handler: Option<Rc<dyn RedirectHandler>> = resource_manager
            .redirect_handler
            .as_ref()
            .map(|handler| {
                Rc::new(crate::resource_redirect::SharedRedirectHandler(Rc::new(
                    std::cell::RefCell::new(handler.clone()),
                ))) as Rc<dyn RedirectHandler>
            });
        (self.properties_factory)(
            properties.clone(),
            resource_id.clone(),
            pack.get_name().to_string(),
            pack_priority,
            redirect_handler,
            method.to_string(),
        )
        .map(|boxed| boxed as Box<dyn crate::properties_loader::CtmProperties>)
    }

    /// java: QuadProcessor.Factory<T>::createProcessor(properties, textureGetter).
    fn create_processor(
        &self,
        properties: &dyn crate::properties_loader::CtmProperties,
        texture_getter: &TextureGetter,
    ) -> Rc<dyn QuadProcessor> {
        // java: the typed factory receives the properties directly; the downcast keeps
        // that call per loader under dyn erasure.
        let t = properties
            .as_any()
            .downcast_ref::<T>()
            .expect("loader properties type mismatch");
        Rc::from(
            self.processor_factory
                .create_processor_with_textures(t, texture_getter),
        )
    }

    /// java: CachingPredicates.Factory<T>::createPredicates(properties, textureGetter);
    /// BaseCachingPredicates.Factory ignores the texture getter as upstream.
    fn create_predicates(
        &self,
        properties: &dyn crate::properties_loader::CtmProperties,
        _texture_getter: &TextureGetter,
    ) -> Rc<dyn crate::processor_base::CachingPredicates> {
        let t = properties
            .as_any()
            .downcast_ref::<T>()
            .expect("loader properties type mismatch");
        Rc::new(self.predicates_factory.create_predicates(t))
    }
}

/// java: the `Type::new` method reference handed to createLoader as the
/// CtmProperties.Factory.
fn props_factory_of<T, F>(ctor: F) -> CtmPropertiesFactory<T>
where
    T: 'static,
    F: Fn(
            Properties,
            Identifier,
            String,
            i32,
            Option<Rc<dyn RedirectHandler>>,
            String,
        ) -> T
        + 'static,
{
    Rc::new(
        move |properties, resource_id, pack_id, pack_priority, redirect_handler, method| {
            Some(Box::new(ctor(
                properties,
                resource_id,
                pack_id,
                pack_priority,
                redirect_handler,
                method,
            )))
        },
    )
}

/// java: private static <T extends CtmProperties> CtmProperties.Factory<T>
/// wrapWithOptifineOnlyCheck(CtmProperties.Factory<T> propertiesFactory).
fn wrap_optifine_only_check<T: 'static>(
    factory: CtmPropertiesFactory<T>,
) -> CtmPropertiesFactory<T> {
    Rc::new(
        move |properties, resource_id, pack_id, pack_priority, redirect_handler, method| {
            if parse_optifine_only(&properties, &resource_id) {
                return None;
            }
            factory(properties, resource_id, pack_id, pack_priority, redirect_handler, method)
        },
    )
}

/// java: private static <T extends CtmProperties> CtmLoader<T> createLoader(
/// CtmProperties.Factory<T> propertiesFactory, QuadProcessor.Factory<T> processorFactory,
/// CachingPredicates.Factory<T> predicatesFactory).
fn create_loader_predicates<T>(
    properties_factory: CtmPropertiesFactory<T>,
    processor_factory: Rc<dyn AbstractQuadProcessorFactory<T>>,
    predicates_factory: Rc<BaseCachingPredicatesFactory>,
) -> CtmLoaderImpl<T>
where
    T: crate::properties_loader::CtmProperties + CtmProperties + AbstractProcessorProps + 'static,
{
    CtmLoaderImpl {
        properties_factory,
        processor_factory,
        predicates_factory,
    }
}

/// java: createLoader(propertiesFactory, validator, processorFactory,
/// isValidForMultipass) = createLoader(wrapWithOptifineOnlyCheck(
/// TileAmountValidator.wrapFactory(BaseCtmProperties.wrapFactory(propertiesFactory),
/// validator)), processorFactory, new BaseCachingPredicates.Factory<>(isValidForMultipass)).
fn create_loader_validator_multipass<T>(
    properties_factory: CtmPropertiesFactory<T>,
    validator: Rc<dyn TileAmountValidator<T>>,
    processor_factory: Rc<dyn AbstractQuadProcessorFactory<T>>,
    is_valid_for_multipass: bool,
) -> CtmLoaderImpl<T>
where
    T: crate::properties_loader::CtmProperties + CtmProperties + AbstractProcessorProps + 'static,
{
    // rust carrying: upstream wraps the factory down to BaseCtmProperties; the port keeps
    // it typed and erases at the loader boundary instead.
    let wrapped = wrap_optifine_only_check(wrap_factory(properties_factory, validator));
    create_loader_predicates(
        wrapped,
        processor_factory,
        Rc::new(BaseCachingPredicatesFactory {
            is_valid_for_multipass,
        }),
    )
}

/// java: createLoader(propertiesFactory, validator, processorFactory) delegates with
/// isValidForMultipass true.
fn create_loader_validator<T>(
    properties_factory: CtmPropertiesFactory<T>,
    validator: Rc<dyn TileAmountValidator<T>>,
    processor_factory: Rc<dyn AbstractQuadProcessorFactory<T>>,
) -> CtmLoaderImpl<T>
where
    T: crate::properties_loader::CtmProperties + CtmProperties + AbstractProcessorProps + 'static,
{
    create_loader_validator_multipass(properties_factory, validator, processor_factory, true)
}

/// java: createLoader(propertiesFactory, processorFactory, isValidForMultipass) =
/// createLoader(wrapWithOptifineOnlyCheck(
/// BaseCtmProperties.wrapFactory(propertiesFactory)), processorFactory,
/// new BaseCachingPredicates.Factory<>(isValidForMultipass)).
fn create_loader_multipass<T>(
    properties_factory: CtmPropertiesFactory<T>,
    processor_factory: Rc<dyn AbstractQuadProcessorFactory<T>>,
    is_valid_for_multipass: bool,
) -> CtmLoaderImpl<T>
where
    T: crate::properties_loader::CtmProperties + CtmProperties + AbstractProcessorProps + 'static,
{
    let wrapped = wrap_optifine_only_check(properties_factory);
    create_loader_predicates(
        wrapped,
        processor_factory,
        Rc::new(BaseCachingPredicatesFactory {
            is_valid_for_multipass,
        }),
    )
}

/// java: createLoader(propertiesFactory, processorFactory) delegates with
/// isValidForMultipass true.
fn create_loader<T>(
    properties_factory: CtmPropertiesFactory<T>,
    processor_factory: Rc<dyn AbstractQuadProcessorFactory<T>>,
) -> CtmLoaderImpl<T>
where
    T: crate::properties_loader::CtmProperties + CtmProperties + AbstractProcessorProps + 'static,
{
    create_loader_multipass(properties_factory, processor_factory, true)
}

/// java: createCustomLoader(propertiesFactory, validator, processorFactory,
/// isValidForMultipass): the custom variants skip the optifine-only wrap.
fn create_custom_loader_multipass<T>(
    properties_factory: CtmPropertiesFactory<T>,
    validator: Rc<dyn TileAmountValidator<T>>,
    processor_factory: Rc<dyn AbstractQuadProcessorFactory<T>>,
    is_valid_for_multipass: bool,
) -> CtmLoaderImpl<T>
where
    T: crate::properties_loader::CtmProperties + CtmProperties + AbstractProcessorProps + 'static,
{
    let wrapped = wrap_factory(properties_factory, validator);
    create_loader_predicates(
        wrapped,
        processor_factory,
        Rc::new(BaseCachingPredicatesFactory {
            is_valid_for_multipass,
        }),
    )
}

/// java: createCustomLoader(propertiesFactory, validator, processorFactory) delegates
/// with isValidForMultipass true.
fn create_custom_loader<T>(
    properties_factory: CtmPropertiesFactory<T>,
    validator: Rc<dyn TileAmountValidator<T>>,
    processor_factory: Rc<dyn AbstractQuadProcessorFactory<T>>,
) -> CtmLoaderImpl<T>
where
    T: crate::properties_loader::CtmProperties + CtmProperties + AbstractProcessorProps + 'static,
{
    create_custom_loader_multipass(properties_factory, validator, processor_factory, true)
}

/// java: createCustomLoader(propertiesFactory, processorFactory) delegates with
/// isValidForMultipass true.
fn create_custom_loader_no_validator<T>(
    properties_factory: CtmPropertiesFactory<T>,
    processor_factory: Rc<dyn AbstractQuadProcessorFactory<T>>,
) -> CtmLoaderImpl<T>
where
    T: crate::properties_loader::CtmProperties + CtmProperties + AbstractProcessorProps + 'static,
{
    create_loader_predicates(
        properties_factory,
        processor_factory,
        Rc::new(BaseCachingPredicatesFactory {
            is_valid_for_multipass: true,
        }),
    )
}

/// java: ResourceManagerHelper.get(ResourceType.CLIENT_RESOURCES)
/// .registerBuiltinResourcePack(...): the port records the two built-in packs; the
/// resource-manager seam reads them when the real client resource pipeline lands.
fn register_builtin_resource_pack(path: &str, display_name_translation_key: &str) {
    BUILTIN_RESOURCE_PACKS.with(|packs| {
        packs
            .borrow_mut()
            .push((path.to_string(), display_name_translation_key.to_string()))
    });
}

/// java: the test's ResourcePack carry (name + namespace resources), mirroring the
/// properties_loader test pack.
#[cfg(test)]
fn test_pack() -> ResourcePack {
    let mut pack = ResourcePack::default();
    pack.name = "testpack".into();
    pack.namespaces = vec!["minecraft".into()];
    pack
}

/// java: asId(String path).
fn as_id(path: &str) -> Identifier {
    Identifier::new(ID, path).expect("valid continuity identifier")
}

/// java: ContinuityClient.onInitializeClient, the loader-registration tail (the init
/// calls ahead of it live in the runtime splice).
pub fn on_initialize_client() {
    let _ = as_id("dummy"); // java: the built-in pack ids go through asId.

    // java: FabricLoader.getInstance().getModContainer(ID).orElseThrow():
    register_builtin_resource_pack("default", "resourcePack.continuity.default.name");
    register_builtin_resource_pack(
        "glass_pane_culling_fix",
        "resourcePack.continuity.glass_pane_culling_fix.name",
    );

    let registry = CtmLoaderRegistry::get();

    let mut loader: Rc<dyn CtmLoaderObj>;

    // Standard simple methods
    // java: createLoader(OrientedConnectingCtmProperties::new,
    // new TileAmountValidator.AtLeast<>(47),
    // SimpleQuadProcessor.Factory<CtmSpriteProvider>).
    loader = Rc::new(create_loader_validator(
        props_factory_of(|properties, resource_id, pack_id, pack_priority, redirect_handler, method| {
            crate::connecting_properties::OrientedConnectingCtmProperties::new(
                properties, resource_id, pack_id, pack_priority, redirect_handler, method,
            )
        }),
        Rc::new(AtLeast::new(47)),
        Rc::new(SimpleQuadProcessorFactory::new(CtmSpriteProviderFactory)),
    ));
    registry.register_loader("ctm", loader.clone());
    registry.register_loader("glass", loader);

    // java: createLoader(CompactConnectingCtmProperties::new, new AtLeast<>(5),
    // CompactCtmQuadProcessor.Factory<...>, false).
    loader = Rc::new(create_loader_validator_multipass(
        props_factory_of(|properties, resource_id, pack_id, pack_priority, redirect_handler, method| {
            crate::connecting_properties::CompactConnectingCtmProperties::new(
                properties, resource_id, pack_id, pack_priority, redirect_handler, method,
            )
        }),
        Rc::new(AtLeast::new(5)),
        Rc::new(CompactCtmQuadProcessorFactory),
        false,
    ));
    registry.register_loader("ctm_compact", loader);

    // java: createLoader(OrientedConnectingCtmProperties::new, new Exactly<>(4),
    // SimpleQuadProcessor.Factory<HorizontalSpriteProvider>...).
    loader = Rc::new(create_loader_validator(
        props_factory_of(|properties, resource_id, pack_id, pack_priority, redirect_handler, method| {
            crate::connecting_properties::OrientedConnectingCtmProperties::new(
                properties, resource_id, pack_id, pack_priority, redirect_handler, method,
            )
        }),
        Rc::new(Exactly::new(4)),
        Rc::new(SimpleQuadProcessorFactory::new(HorizontalSpriteProviderFactory)),
    ));
    registry.register_loader("horizontal", loader.clone());
    registry.register_loader("bookshelf", loader);

    // java: createLoader(OrientedConnectingCtmProperties::new, new Exactly<>(4),
    // SimpleQuadProcessor.Factory<VerticalSpriteProvider>...).
    loader = Rc::new(create_loader_validator(
        props_factory_of(|properties, resource_id, pack_id, pack_priority, redirect_handler, method| {
            crate::connecting_properties::OrientedConnectingCtmProperties::new(
                properties, resource_id, pack_id, pack_priority, redirect_handler, method,
            )
        }),
        Rc::new(Exactly::new(4)),
        Rc::new(SimpleQuadProcessorFactory::new(VerticalSpriteProviderFactory)),
    ));
    registry.register_loader("vertical", loader);

    // java: createLoader(OrientedConnectingCtmProperties::new, new Exactly<>(7),
    // SimpleQuadProcessor.Factory<HorizontalVerticalSpriteProvider>...).
    loader = Rc::new(create_loader_validator(
        props_factory_of(|properties, resource_id, pack_id, pack_priority, redirect_handler, method| {
            crate::connecting_properties::OrientedConnectingCtmProperties::new(
                properties, resource_id, pack_id, pack_priority, redirect_handler, method,
            )
        }),
        Rc::new(Exactly::new(7)),
        Rc::new(SimpleQuadProcessorFactory::new(HorizontalVerticalSpriteProviderFactory)),
    ));
    registry.register_loader("horizontal+vertical", loader.clone());
    registry.register_loader("h+v", loader);

    // java: createLoader(OrientedConnectingCtmProperties::new, new Exactly<>(7),
    // SimpleQuadProcessor.Factory<VerticalHorizontalSpriteProvider>...).
    loader = Rc::new(create_loader_validator(
        props_factory_of(|properties, resource_id, pack_id, pack_priority, redirect_handler, method| {
            crate::connecting_properties::OrientedConnectingCtmProperties::new(
                properties, resource_id, pack_id, pack_priority, redirect_handler, method,
            )
        }),
        Rc::new(Exactly::new(7)),
        Rc::new(SimpleQuadProcessorFactory::new(VerticalHorizontalSpriteProviderFactory)),
    ));
    registry.register_loader("vertical+horizontal", loader.clone());
    registry.register_loader("v+h", loader);

    // java: createLoader(ConnectingCtmProperties::new, new Exactly<>(1),
    // TopQuadProcessor.Factory<...>).
    loader = Rc::new(create_loader_validator(
        props_factory_of(|properties, resource_id, pack_id, pack_priority, redirect_handler, method| {
            crate::connecting_properties::ConnectingCtmProperties::new(
                properties, resource_id, pack_id, pack_priority, redirect_handler, method,
            )
        }),
        Rc::new(Exactly::new(1)),
        Rc::new(TopQuadProcessorFactory),
    ));
    registry.register_loader("top", loader);

    // java: createLoader(RandomCtmProperties::new,
    // SimpleQuadProcessor.Factory<RandomSpriteProvider>...).
    loader = Rc::new(create_loader(
        props_factory_of(|properties, resource_id, pack_id, pack_priority, redirect_handler, method| {
            crate::connecting_properties::RandomCtmProperties::new(
                properties, resource_id, pack_id, pack_priority, redirect_handler, method,
            )
        }),
        Rc::new(SimpleQuadProcessorFactory::new(RandomSpriteProviderFactory)),
    ));
    registry.register_loader("random", loader);

    // java: createLoader(RepeatCtmProperties::new, new RepeatCtmProperties.Validator<>(),
    // SimpleQuadProcessor.Factory<RepeatSpriteProvider>...).
    loader = Rc::new(create_loader_validator(
        props_factory_of(|properties, resource_id, pack_id, pack_priority, redirect_handler, method| {
            crate::connecting_properties::RepeatCtmProperties::new(
                properties, resource_id, pack_id, pack_priority, redirect_handler, method,
            )
        }),
        Rc::new(RepeatValidator) as Rc<dyn TileAmountValidator<RepeatCtmProperties>>,
        Rc::new(SimpleQuadProcessorFactory::new(RepeatSpriteProviderFactory)),
    ));
    registry.register_loader("repeat", loader);

    // java: createLoader(BaseCtmProperties::new, new Exactly<>(1),
    // SimpleQuadProcessor.Factory<FixedSpriteProvider>...).
    loader = Rc::new(create_loader_validator(
        props_factory_of(|properties, resource_id, pack_id, pack_priority, redirect_handler, method| {
            BaseCtmProperties::new(
                properties, resource_id, pack_id, pack_priority, redirect_handler, method,
            )
        }),
        Rc::new(Exactly::new(1)),
        Rc::new(SimpleQuadProcessorFactory::new(FixedSpriteProviderFactory)),
    ));
    registry.register_loader("fixed", loader);

    // Standard overlay methods
    // java: createCustomLoader(StandardOverlayCtmProperties::new, new AtLeast<>(17),
    // StandardOverlayQuadProcessor.Factory<...>).
    loader = Rc::new(create_custom_loader(
        props_factory_of(|properties, resource_id, pack_id, pack_priority, redirect_handler, method| {
            crate::overlay_properties::StandardOverlayCtmProperties::new(
                properties, resource_id, pack_id, pack_priority, redirect_handler, method,
            )
        }),
        Rc::new(AtLeast::new(17)) as Rc<dyn TileAmountValidator<crate::overlay_properties::StandardOverlayCtmProperties>>,
        Rc::new(StandardOverlayQuadProcessorFactory),
    ));
    registry.register_loader("overlay", loader);

    // java: createCustomLoader(OrientedConnectingOverlayCtmProperties::new,
    // new AtLeast<>(47), SimpleOverlayQuadProcessor.Factory<CtmSpriteProvider>...).
    loader = Rc::new(create_custom_loader(
        props_factory_of(|properties, resource_id, pack_id, pack_priority, redirect_handler, method| {
            crate::overlay_properties::OrientedConnectingOverlayCtmProperties::new(
                properties, resource_id, pack_id, pack_priority, redirect_handler, method,
            )
        }),
        Rc::new(AtLeast::new(47)) as Rc<dyn TileAmountValidator<crate::overlay_properties::OrientedConnectingOverlayCtmProperties>>,
        Rc::new(SimpleOverlayQuadProcessorFactory::new(CtmSpriteProviderFactory)),
    ));
    registry.register_loader("overlay_ctm", loader);

    // java: createCustomLoader(RandomOverlayCtmProperties::new,
    // SimpleOverlayQuadProcessor.Factory<RandomSpriteProvider>...).
    loader = Rc::new(create_custom_loader_no_validator(
        props_factory_of(|properties, resource_id, pack_id, pack_priority, redirect_handler, method| {
            crate::overlay_properties::RandomOverlayCtmProperties::new(
                properties, resource_id, pack_id, pack_priority, redirect_handler, method,
            )
        }),
        Rc::new(SimpleOverlayQuadProcessorFactory::new(RandomSpriteProviderFactory)),
    ));
    registry.register_loader("overlay_random", loader);

    // java: createCustomLoader(RepeatOverlayCtmProperties::new,
    // new RepeatCtmProperties.Validator<>(),
    // SimpleOverlayQuadProcessor.Factory<RepeatSpriteProvider>...).
    loader = Rc::new(create_custom_loader(
        props_factory_of(|properties, resource_id, pack_id, pack_priority, redirect_handler, method| {
            crate::overlay_properties::RepeatOverlayCtmProperties::new(
                properties, resource_id, pack_id, pack_priority, redirect_handler, method,
            )
        }),
        Rc::new(RepeatValidator) as Rc<dyn TileAmountValidator<crate::overlay_properties::RepeatOverlayCtmProperties>>,
        Rc::new(SimpleOverlayQuadProcessorFactory::new(RepeatSpriteProviderFactory)),
    ));
    registry.register_loader("overlay_repeat", loader);

    // java: createCustomLoader(BaseOverlayCtmProperties::new, new Exactly<>(1),
    // SimpleOverlayQuadProcessor.Factory<FixedSpriteProvider>...).
    loader = Rc::new(create_custom_loader(
        props_factory_of(|properties, resource_id, pack_id, pack_priority, redirect_handler, method| {
            crate::overlay_properties::BaseOverlayCtmProperties::new(
                properties, resource_id, pack_id, pack_priority, redirect_handler, method,
            )
        }),
        Rc::new(Exactly::new(1)) as Rc<dyn TileAmountValidator<crate::overlay_properties::BaseOverlayCtmProperties>>,
        Rc::new(SimpleOverlayQuadProcessorFactory::new(FixedSpriteProviderFactory)),
    ));
    registry.register_loader("overlay_fixed", loader);

    // Custom methods
    // java: createCustomLoader(OrientedConnectingOverlayCtmProperties::new,
    // new Exactly<>(4), SimpleOverlayQuadProcessor.Factory<HorizontalSpriteProvider>...).
    loader = Rc::new(create_custom_loader(
        props_factory_of(|properties, resource_id, pack_id, pack_priority, redirect_handler, method| {
            crate::overlay_properties::OrientedConnectingOverlayCtmProperties::new(
                properties, resource_id, pack_id, pack_priority, redirect_handler, method,
            )
        }),
        Rc::new(Exactly::new(4)) as Rc<dyn TileAmountValidator<crate::overlay_properties::OrientedConnectingOverlayCtmProperties>>,
        Rc::new(SimpleOverlayQuadProcessorFactory::new(HorizontalSpriteProviderFactory)),
    ));
    registry.register_loader("overlay_horizontal", loader);

    // java: createCustomLoader(OrientedConnectingOverlayCtmProperties::new,
    // new Exactly<>(4), SimpleOverlayQuadProcessor.Factory<VerticalSpriteProvider>...).
    loader = Rc::new(create_custom_loader(
        props_factory_of(|properties, resource_id, pack_id, pack_priority, redirect_handler, method| {
            crate::overlay_properties::OrientedConnectingOverlayCtmProperties::new(
                properties, resource_id, pack_id, pack_priority, redirect_handler, method,
            )
        }),
        Rc::new(Exactly::new(4)) as Rc<dyn TileAmountValidator<crate::overlay_properties::OrientedConnectingOverlayCtmProperties>>,
        Rc::new(SimpleOverlayQuadProcessorFactory::new(VerticalSpriteProviderFactory)),
    ));
    registry.register_loader("overlay_vertical", loader);

    // java: createCustomLoader(OrientedConnectingOverlayCtmProperties::new,
    // new Exactly<>(7), SimpleOverlayQuadProcessor.Factory<HorizontalVerticalSpriteProvider>...).
    loader = Rc::new(create_custom_loader(
        props_factory_of(|properties, resource_id, pack_id, pack_priority, redirect_handler, method| {
            crate::overlay_properties::OrientedConnectingOverlayCtmProperties::new(
                properties, resource_id, pack_id, pack_priority, redirect_handler, method,
            )
        }),
        Rc::new(Exactly::new(7)) as Rc<dyn TileAmountValidator<crate::overlay_properties::OrientedConnectingOverlayCtmProperties>>,
        Rc::new(SimpleOverlayQuadProcessorFactory::new(HorizontalVerticalSpriteProviderFactory)),
    ));
    registry.register_loader("overlay_horizontal+vertical", loader.clone());
    registry.register_loader("overlay_h+v", loader);

    // java: createCustomLoader(OrientedConnectingOverlayCtmProperties::new,
    // new Exactly<>(7), SimpleOverlayQuadProcessor.Factory<VerticalHorizontalSpriteProvider>...).
    loader = Rc::new(create_custom_loader(
        props_factory_of(|properties, resource_id, pack_id, pack_priority, redirect_handler, method| {
            crate::overlay_properties::OrientedConnectingOverlayCtmProperties::new(
                properties, resource_id, pack_id, pack_priority, redirect_handler, method,
            )
        }),
        Rc::new(Exactly::new(7)) as Rc<dyn TileAmountValidator<crate::overlay_properties::OrientedConnectingOverlayCtmProperties>>,
        Rc::new(SimpleOverlayQuadProcessorFactory::new(VerticalHorizontalSpriteProviderFactory)),
    ));
    registry.register_loader("overlay_vertical+horizontal", loader.clone());
    registry.register_loader("overlay_v+h", loader);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cmp::Ordering;

    #[test]
    fn init_registers_all_loaders() {
        on_initialize_client();
        let registry = CtmLoaderRegistry::get();
        for method in [
            "ctm",
            "glass",
            "ctm_compact",
            "horizontal",
            "bookshelf",
            "vertical",
            "horizontal+vertical",
            "h+v",
            "vertical+horizontal",
            "v+h",
            "top",
            "random",
            "repeat",
            "fixed",
            "overlay",
            "overlay_ctm",
            "overlay_random",
            "overlay_repeat",
            "overlay_fixed",
            "overlay_horizontal",
            "overlay_vertical",
            "overlay_horizontal+vertical",
            "overlay_h+v",
            "overlay_vertical+horizontal",
            "overlay_v+h",
        ] {
            assert!(
                registry.get_loader(method).is_some(),
                "missing loader: {method}"
            );
        }
        // java: ctm and glass share one loader instance; aliases share theirs.
        assert!(Rc::ptr_eq(
            &registry.get_loader("ctm").unwrap(),
            &registry.get_loader("glass").unwrap()
        ));
        assert!(Rc::ptr_eq(
            &registry.get_loader("h+v").unwrap(),
            &registry.get_loader("horizontal+vertical").unwrap()
        ));
        assert!(Rc::ptr_eq(
            &registry.get_loader("overlay_v+h").unwrap(),
            &registry.get_loader("overlay_vertical+horizontal").unwrap()
        ));
        assert!(!Rc::ptr_eq(
            &registry.get_loader("ctm").unwrap(),
            &registry.get_loader("top").unwrap()
        ));
    }

    #[test]
    fn validator_wrap_rejects_wrong_tile_amount() {
        // java: TileAmountValidator.wrapFactory with Exactly(1) rejects when the tile
        // amount does not match.
        let factory = wrap_factory(
            props_factory_of(|properties, resource_id, pack_id, pack_priority, redirect_handler, method| {
                BaseCtmProperties::new(properties, resource_id, pack_id, pack_priority, redirect_handler, method)
            }),
            Rc::new(Exactly::new(1)),
        );
        // No tiles parsed: the tile amount is 0, so Exactly(1) rejects.
        let properties = Properties::new();
        let resource_id = Identifier::new("minecraft", "optifine/ctm/test.properties").unwrap();
        let created = factory(
            properties,
            resource_id,
            "testpack".to_string(),
            0,
            None,
            "fixed".to_string(),
        );
        assert!(created.is_none());
    }

    #[test]
    fn wrap_optifine_only_check_skips_optifine_only_files() {
        let inner = props_factory_of(|properties, resource_id, pack_id, pack_priority, redirect_handler, method| {
            BaseCtmProperties::new(properties, resource_id, pack_id, pack_priority, redirect_handler, method)
        });
        let wrapped = wrap_optifine_only_check(inner);

        let mut properties = Properties::new();
        properties.insert("optifineOnly".to_string(), "true".to_string());
        let resource_id = Identifier::new("minecraft", "optifine/ctm/test.properties").unwrap();
        let created = wrapped(
            properties,
            resource_id,
            "testpack".to_string(),
            0,
            None,
            "ctm".to_string(),
        );
        assert!(created.is_none());
    }

    #[test]
    fn erased_create_properties_drives_the_factory_chain() {
        on_initialize_client();
        let registry = CtmLoaderRegistry::get();
        let loader = registry.get_loader("ctm").unwrap();
        let mut properties = Properties::new();
        properties.insert("tiles".to_string(), "minecraft:block/stone".to_string());
        let resource_id = Identifier::new("minecraft", "optifine/ctm/test.properties").unwrap();
        let manager = ResourceManager {
            packs: Vec::new(),
            redirect_handler: None,
        };
        let created = loader.create_properties(
            &properties,
            &resource_id,
            &test_pack(),
            0,
            &manager,
            "ctm",
        );
        // java: AtLeast(47) rejects a single tile.
        assert!(created.is_none());
        let _ = Ordering::Equal;
    }
}
