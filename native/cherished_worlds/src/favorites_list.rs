//! 1:1 port of client/favorites/FavoritesList.

use crate::services::IPlatformHelper;

use crate::constants::{log, MOD_ID};
use crate::seams::{
    self, CompoundTag, ListTag, StringTag,
};
use std::collections::HashSet;
use std::sync::Mutex;
use std::sync::OnceLock;

// java: public class FavoritesList (static methods)
pub struct FavoritesList;

impl FavoritesList {
    pub fn contains(file_name: &str) -> bool {
        contains(file_name)
    }

    pub fn add(name: &str) {
        add(name)
    }

    pub fn remove(name: &str) {
        remove(name)
    }

    pub fn save() {
        save()
    }

    pub fn load() {
        load()
    }
}

// java: private static final Set<String> favorites = new HashSet<>();
fn favorites() -> &'static Mutex<HashSet<String>> {
    static FAVORITES: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
    FAVORITES.get_or_init(|| Mutex::new(HashSet::new()))
}

fn favorites_file() -> std::path::PathBuf {
    // java: new File(Services.PLATFORM.getGamePath().toString(),
    //                CherishedWorldsConstants.MOD_ID + "-favorites.dat")
    let mut path = crate::services::Services::platform().get_game_path();
    path.push(format!("{MOD_ID}-favorites.dat"));
    path
}

pub fn load() {
    // java: try { favorites.clear(); ... } catch (Exception exception) {
    //         LOG.error("Couldn't load favorites list", exception); }
    let result: Result<(), String> = (|| {
        favorites().lock().unwrap().clear();
        let file = favorites_file();
        // java: CompoundTag compound = NbtIo.read(file.toPath());
        let compound = match seams::nbt_io_read(&file) {
            Some(tag) => tag,
            // java: if (compound == null) { return; }
            None => return Ok(()),
        };
        // java: ListTag list = compound.getListOrEmpty("favorites");
        let list = compound.get_list_or_empty("favorites");

        // java: for (int i = 0; i < list.size(); ++i) {
        //           list.getString(i).ifPresent(favorites::add); }
        for i in 0..list.size() {
            if let Some(s) = list.get_string(i) {
                favorites().lock().unwrap().insert(s);
            }
        }
        Ok(())
    })();
    if let Err(e) = result {
        log().error(&format!("Couldn't load favorites list: {e}"));
    }
}

pub fn save() {
    let result: Result<(), String> = (|| {
        // java: ListTag list = new ListTag();
        let mut list = ListTag::new();

        // java: for (String s : favorites) { list.add(StringTag.valueOf(s)); }
        for s in favorites().lock().unwrap().iter() {
            list.add(StringTag::value_of(s));
        }
        // java: CompoundTag compound = new CompoundTag();
        //       compound.put("favorites", list);
        let mut compound = CompoundTag::default();
        compound.put("favorites", list);
        let file = favorites_file();

        // java: if (!file.exists()) { FileUtils.forceMkdirParent(file); }
        if !file.exists() {
            seams::force_mkdir_parent(&file);
        }
        // java: NbtIo.write(compound, file.toPath());
        seams::nbt_io_write(&compound, &file).map_err(|e| e.to_string())?;
        Ok(())
    })();
    if let Err(e) = result {
        log().error(&format!("Couldn't save favorites list: {e}"));
    }
}

// java: public static boolean contains(String fileName)
pub fn contains(file_name: &str) -> bool {
    favorites().lock().unwrap().contains(file_name)
}

// java: public static void add(String name)
pub fn add(name: &str) {
    favorites().lock().unwrap().insert(name.to_string());
}

// java: public static void remove(String name)
pub fn remove(name: &str) {
    favorites().lock().unwrap().remove(name);
}

// test seam: expose the raw set for assertions only.
pub fn snapshot_for_tests() -> Vec<String> {
    let mut v: Vec<String> = favorites().lock().unwrap().iter().cloned().collect();
    v.sort();
    v
}

// test seam: clear the set.
pub fn clear_for_tests() {
    favorites().lock().unwrap().clear();
}
