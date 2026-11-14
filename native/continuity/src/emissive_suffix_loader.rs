//! 1:1 of the Continuity client.resource.EmissiveSuffixLoader, by PepperCode1
//! (LGPL-3.0).

use std::cell::RefCell;

use crate::seams::{get_resource, parse_properties_stream, Identifier, ResourceManager};

fn warn(msg: &str) {
    // java: ContinuityClient.LOGGER.error(...).
    eprintln!("[continuity] {msg}");
}

/// java: EmissiveSuffixLoader.LOCATION = new Identifier("optifine/emissive.properties").
pub fn location() -> Identifier {
    Identifier::new("optifine", "emissive.properties").unwrap()
}

thread_local! {
    /// java: private static String emissiveSuffix.
    static EMISSIVE_SUFFIX: RefCell<Option<String>> = const { RefCell::new(None) };
}

/// java: EmissiveSuffixLoader.getEmissiveSuffix() - @Nullable String.
pub fn get_emissive_suffix() -> Option<String> {
    EMISSIVE_SUFFIX.with(|suffix| suffix.borrow().clone())
}

/// java: EmissiveSuffixLoader.load(manager).
pub fn load(_resource_manager: &ResourceManager) {
    // java: emissiveSuffix = null.
    EMISSIVE_SUFFIX.with(|suffix| *suffix.borrow_mut() = None);

    // java: Optional<Resource> optionalResource = manager.getResource(LOCATION);
    // if (optionalResource.isPresent()) { Resource resource = optionalResource.get();
    // try (InputStream inputStream = resource.getInputStream()) { Properties properties
    // = new Properties(); properties.load(inputStream); emissiveSuffix =
    // properties.getProperty("suffix.emissive"); } catch (IOException e) {
    // LOGGER.error(...); } }
    if let Some(resource) = get_resource(&location()) {
        match crate::seams::resource_get_input_stream(&resource) {
            Some(contents) => {
                let properties = parse_properties_stream(&contents);
                let suffix = properties.get("suffix.emissive").cloned();
                EMISSIVE_SUFFIX.with(|cell| *cell.borrow_mut() = suffix);
            }
            None => {
                warn(&format!(
                    "Failed to load emissive suffix from file '{}'",
                    location().path
                ));
            }
        }
    }
}

#[cfg(test)]
pub(crate) fn set_emissive_suffix_for_test(suffix: Option<String>) {
    EMISSIVE_SUFFIX.with(|cell| *cell.borrow_mut() = suffix);
}

#[cfg(test)]
mod emissive_suffix_loader_tests {
    use super::*;

    #[test]
    fn suffix_unset_when_no_properties() {
        EMISSIVE_SUFFIX.with(|cell| *cell.borrow_mut() = None);
        assert!(get_emissive_suffix().is_none());
    }

    #[test]
    fn suffix_round_trips() {
        EMISSIVE_SUFFIX.with(|cell| *cell.borrow_mut() = Some("_e".to_string()));
        assert_eq!(get_emissive_suffix(), Some("_e".to_string()));
        EMISSIVE_SUFFIX.with(|cell| *cell.borrow_mut() = None);
        assert!(get_emissive_suffix().is_none());
    }
}
