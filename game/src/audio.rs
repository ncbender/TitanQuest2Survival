//! Sound through TQ2's own FMOD Studio runtime: `fmodstudio.dll`/`fmod.dll` and the
//! `.bank` files are loaded straight from the player's Steam install at runtime, so the
//! game plays the original events (with their variations and mixing) and nothing is
//! extracted or shipped. Without the DLLs the game simply stays silent.
use bevy::prelude::*;
use libloading::Library;
use std::collections::HashMap;
use std::ffi::{c_char, c_int, c_void, CStr, CString};
use std::sync::Mutex;

const TQ2_DIR: &str = r"C:\Program Files (x86)\Steam\steamapps\common\Titan Quest II";
const FMOD_DIR: &str = r"Engine\Plugins\GrimThirdParty\FMODStudio\Binaries\Win64";
const BANK_DIR: &str = r"TQ2\Content\FMOD\Desktop";
/// TQ2 ships FMOD 2.02.22; the Studio API refuses other header versions.
const FMOD_VERSION: u32 = 0x0002_0222;
const FMOD_OK: c_int = 0;
const FMOD_INIT_3D_RIGHTHANDED: u32 = 0x0000_0004;
const BANKS: &[&str] = &[
    "Master.bank",
    "Master.strings.bank",
    "creature.bank",
    "weapons.bank",
    "effects.bank",
    "abilities.bank",
    "ui.bank",
    "loot.bank",
    "music.bank",
    "ambient.bank",
];

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct FmodVector {
    x: f32,
    y: f32,
    z: f32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct Attributes3d {
    position: FmodVector,
    velocity: FmodVector,
    forward: FmodVector,
    up: FmodVector,
}

type Handle = *mut c_void;

/// The handful of FMOD Studio C functions the game uses.
struct Api {
    _core: Library,
    _studio: Library,
    create: unsafe extern "C" fn(*mut Handle, u32) -> c_int,
    initialize: unsafe extern "C" fn(Handle, c_int, u32, u32, *mut c_void) -> c_int,
    load_bank: unsafe extern "C" fn(Handle, *const c_char, u32, *mut Handle) -> c_int,
    get_event: unsafe extern "C" fn(Handle, *const c_char, *mut Handle) -> c_int,
    create_instance: unsafe extern "C" fn(Handle, *mut Handle) -> c_int,
    start: unsafe extern "C" fn(Handle) -> c_int,
    release: unsafe extern "C" fn(Handle) -> c_int,
    set_3d: unsafe extern "C" fn(Handle, *const Attributes3d) -> c_int,
    set_volume: unsafe extern "C" fn(Handle, f32) -> c_int,
    stop: unsafe extern "C" fn(Handle, c_int) -> c_int,
    set_listener: unsafe extern "C" fn(Handle, c_int, *const Attributes3d, *const FmodVector) -> c_int,
    update: unsafe extern "C" fn(Handle) -> c_int,
    bank_event_count: unsafe extern "C" fn(Handle, *mut c_int) -> c_int,
    bank_event_list: unsafe extern "C" fn(Handle, *mut Handle, c_int, *mut c_int) -> c_int,
    event_path: unsafe extern "C" fn(Handle, *mut c_char, c_int, *mut c_int) -> c_int,
}

impl Api {
    fn load() -> Result<Self, String> {
        let dir = std::path::Path::new(TQ2_DIR).join(FMOD_DIR);
        // SAFETY: loading TQ2's own FMOD DLLs; fmod.dll first so fmodstudio.dll can bind to it.
        unsafe {
            let core = Library::new(dir.join("fmod.dll")).map_err(|e| e.to_string())?;
            let studio = Library::new(dir.join("fmodstudio.dll")).map_err(|e| e.to_string())?;
            macro_rules! f {
                ($name:literal) => {
                    *studio.get(concat!($name, "\0").as_bytes()).map_err(|e| format!("{}: {e}", $name))?
                };
            }
            Ok(Self {
                create: f!("FMOD_Studio_System_Create"),
                initialize: f!("FMOD_Studio_System_Initialize"),
                load_bank: f!("FMOD_Studio_System_LoadBankFile"),
                get_event: f!("FMOD_Studio_System_GetEvent"),
                create_instance: f!("FMOD_Studio_EventDescription_CreateInstance"),
                start: f!("FMOD_Studio_EventInstance_Start"),
                release: f!("FMOD_Studio_EventInstance_Release"),
                set_3d: f!("FMOD_Studio_EventInstance_Set3DAttributes"),
                set_volume: f!("FMOD_Studio_EventInstance_SetVolume"),
                stop: f!("FMOD_Studio_EventInstance_Stop"),
                set_listener: f!("FMOD_Studio_System_SetListenerAttributes"),
                update: f!("FMOD_Studio_System_Update"),
                bank_event_count: f!("FMOD_Studio_Bank_GetEventCount"),
                bank_event_list: f!("FMOD_Studio_Bank_GetEventList"),
                event_path: f!("FMOD_Studio_EventDescription_GetPath"),
                _core: core,
                _studio: studio,
            })
        }
    }
}

struct Studio {
    api: Api,
    system: Handle,
    banks: Vec<Handle>,
    events: HashMap<String, Handle>,
    /// When each event last started (for throttling rapid repeats like hits).
    last_played: HashMap<&'static str, std::time::Instant>,
    music: Option<(&'static str, Handle, f32)>,
    /// Player settings (0..1), multiplied into every event's volume.
    sfx_volume: f32,
    music_volume: f32,
}

// SAFETY: the FMOD Studio API is thread-safe in its default (asynchronous) mode.
unsafe impl Send for Studio {}

/// Plays TQ2 sound events. All calls are no-ops when FMOD could not be loaded.
#[derive(Resource, Default)]
pub struct Audio(Option<Mutex<Studio>>);

impl Audio {
    pub fn load() -> Self {
        match Self::try_load() {
            Ok(studio) => {
                info!("FMOD {:x}: {} banks loaded", FMOD_VERSION, studio.banks.len());
                Self(Some(Mutex::new(studio)))
            }
            Err(e) => {
                warn!("no sound: {e}");
                Self(None)
            }
        }
    }

    fn try_load() -> Result<Studio, String> {
        let api = Api::load()?;
        let mut system: Handle = std::ptr::null_mut();
        // SAFETY: plain FMOD C API calls with valid out-pointers.
        unsafe {
            check((api.create)(&mut system, FMOD_VERSION), "create")?;
            check((api.initialize)(system, 512, 0, FMOD_INIT_3D_RIGHTHANDED, std::ptr::null_mut()), "initialize")?;
            let mut banks = vec![];
            for name in BANKS {
                let path = CString::new(std::path::Path::new(TQ2_DIR).join(BANK_DIR).join(name).to_string_lossy().as_bytes()).unwrap();
                let mut bank: Handle = std::ptr::null_mut();
                match check((api.load_bank)(system, path.as_ptr(), 0, &mut bank), name) {
                    Ok(()) => banks.push(bank),
                    Err(e) => warn!("{e}"),
                }
            }
            Ok(Studio { api, system, banks, events: HashMap::new(), last_played: HashMap::new(), music: None, sfx_volume: 1.0, music_volume: 1.0 })
        }
    }

    /// Starts a one-shot event, positioned in the world if `at` is given. Repeats of the same
    /// event within `min_gap` seconds are dropped, so a horde does not stack hundreds of hits.
    pub fn play(&self, event: &'static str, at: Option<Vec3>, volume: f32, min_gap: f32) {
        let Some(studio) = &self.0 else { return };
        let mut s = studio.lock().unwrap();
        let now = std::time::Instant::now();
        if s.last_played.get(event).is_some_and(|t| now.duration_since(*t).as_secs_f32() < min_gap) {
            return;
        }
        s.last_played.insert(event, now);
        let Some(inst) = s.instance(event) else { return };
        // SAFETY: FMOD handles obtained from this system; the instance is released right away
        // and FMOD frees it when it finishes playing.
        unsafe {
            if let Some(p) = at {
                let attrs = Attributes3d {
                    position: FmodVector { x: p.x, y: p.y, z: p.z },
                    forward: FmodVector { x: 0.0, y: 0.0, z: 1.0 },
                    up: FmodVector { x: 0.0, y: 1.0, z: 0.0 },
                    ..default()
                };
                (s.api.set_3d)(inst, &attrs);
            }
            (s.api.set_volume)(inst, volume * s.sfx_volume);
            (s.api.start)(inst);
            (s.api.release)(inst);
        }
    }

    /// Switches the background music (fading the previous track out); same track keeps playing.
    pub fn music(&self, event: &'static str, volume: f32) {
        let Some(studio) = &self.0 else { return };
        let mut s = studio.lock().unwrap();
        if s.music.is_some_and(|(current, _, _)| current == event) {
            return;
        }
        // SAFETY: stopping/starting instances created by this system.
        unsafe {
            if let Some((_, old, _)) = s.music.take() {
                (s.api.stop)(old, 0); // FMOD_STUDIO_STOP_ALLOWFADEOUT
                (s.api.release)(old);
            }
            if let Some(inst) = s.instance(event) {
                (s.api.set_volume)(inst, volume * s.music_volume);
                (s.api.start)(inst);
                s.music = Some((event, inst, volume));
            }
        }
    }

    /// Player volume settings (0..1); applies to the running music immediately.
    pub fn set_volumes(&self, sfx: f32, music: f32) {
        let Some(studio) = &self.0 else { return };
        let mut s = studio.lock().unwrap();
        s.sfx_volume = sfx;
        s.music_volume = music;
        if let Some((_, inst, base)) = s.music {
            // SAFETY: the music instance belongs to this system and is still alive.
            unsafe {
                (s.api.set_volume)(inst, base * music);
            }
        }
    }

    /// Every event path in the loaded banks (development aid: `--list-sounds`).
    pub fn event_paths(&self) -> Vec<String> {
        let Some(studio) = &self.0 else { return vec![] };
        let s = studio.lock().unwrap();
        let mut paths = vec![];
        // SAFETY: enumerating descriptions of loaded banks into correctly sized buffers.
        unsafe {
            for &bank in &s.banks {
                let mut count = 0;
                (s.api.bank_event_count)(bank, &mut count);
                let mut list = vec![std::ptr::null_mut(); count.max(0) as usize];
                let mut got = 0;
                (s.api.bank_event_list)(bank, list.as_mut_ptr(), count, &mut got);
                for &desc in &list[..got.max(0) as usize] {
                    let mut buf = [0 as c_char; 512];
                    let mut len = 0;
                    if (s.api.event_path)(desc, buf.as_mut_ptr(), buf.len() as c_int, &mut len) == FMOD_OK {
                        paths.push(CStr::from_ptr(buf.as_ptr()).to_string_lossy().into_owned());
                    }
                }
            }
        }
        paths.sort();
        paths
    }
}

impl Studio {
    fn instance(&mut self, path: &str) -> Option<Handle> {
        let desc = self.event(path)?;
        let mut inst: Handle = std::ptr::null_mut();
        // SAFETY: valid event description from this system.
        (unsafe { (self.api.create_instance)(desc, &mut inst) } == FMOD_OK).then_some(inst)
    }

    fn event(&mut self, path: &str) -> Option<Handle> {
        if let Some(&d) = self.events.get(path) {
            return (!d.is_null()).then_some(d);
        }
        let mut desc: Handle = std::ptr::null_mut();
        let c = CString::new(path).ok()?;
        // SAFETY: valid system handle and C string.
        let ok = unsafe { (self.api.get_event)(self.system, c.as_ptr(), &mut desc) } == FMOD_OK;
        if !ok {
            warn!("unknown sound event {path}");
            desc = std::ptr::null_mut();
        }
        self.events.insert(path.to_string(), desc);
        ok.then_some(desc)
    }
}

fn check(result: c_int, what: &str) -> Result<(), String> {
    if result == FMOD_OK { Ok(()) } else { Err(format!("FMOD {what} failed ({result})")) }
}

pub struct AudioPlugin;

impl Plugin for AudioPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Audio::load()).add_systems(Last, update_listener);
    }
}

/// Moves the FMOD listener with the camera and ticks the Studio system once per frame.
fn update_listener(audio: Res<Audio>, camera: Query<&GlobalTransform, With<Camera3d>>) {
    let Some(studio) = &audio.0 else { return };
    let s = studio.lock().unwrap();
    // SAFETY: valid system handle; attributes live for the call.
    unsafe {
        if let Ok(t) = camera.single() {
            let (p, f, u) = (t.translation(), t.forward(), t.up());
            let attrs = Attributes3d {
                position: FmodVector { x: p.x, y: p.y, z: p.z },
                forward: FmodVector { x: f.x, y: f.y, z: f.z },
                up: FmodVector { x: u.x, y: u.y, z: u.z },
                ..default()
            };
            (s.api.set_listener)(s.system, 0, &attrs, std::ptr::null());
        }
        (s.api.update)(s.system);
    }
}
