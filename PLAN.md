# Multi-Profile ("Slack Workspaces") Implementation Plan

## Concept

Each profile is a fully isolated environment with its own config, database, cache, and secrets.
Profiles are discovered by scanning a profiles directory — each subdirectory is a profile.
Only one profile is active at a time; switching requires a restart.

## Directory Layout

```
~/.config/clareon/profiles/<name>/config.json    # per-profile config (includes metadata)
~/.local/share/clareon/profiles/<name>/clareon.db # per-profile database
~/.cache/clareon/profiles/<name>/conversations/   # per-profile workspace cache
```

No backward compatibility needed — existing flat layout will be abandoned.

## Changes Overview

### 1. New: `Profile` type in `clareon-core` (`config/profile.rs`)

```rust
/// Identifies a profile by name (directory name)
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ProfileId(String);

/// Runtime-resolved profile with all paths computed
pub struct Profile {
    pub id: ProfileId,
    pub config_path: PathBuf,
    pub database_path: PathBuf,
    pub database_url: String,
    pub cache_root: PathBuf,
    pub workspace_cache_dir: PathBuf,
    pub shared_cache_dir: PathBuf,
}
```

`Profile` provides path computation (currently scattered across `Config` static methods).
`Profile::new(id)` computes all paths from the profile name.

### 2. New: `ProfileManager` in `clareon-core` (`config/profile.rs`)

Responsible for discovering and managing profiles:

```rust
pub struct ProfileManager;

impl ProfileManager {
    /// List all available profiles (scan profiles directory)
    pub fn list_profiles() -> Result<Vec<ProfileId>>;

    /// Get a resolved profile (computes paths, creates dirs if needed)
    pub fn get_profile(id: &ProfileId) -> Result<Profile>;

    /// Create a new profile with default config
    pub fn create_profile(id: &ProfileId) -> Result<Profile>;

    /// Check if a profile exists
    pub fn profile_exists(id: &ProfileId) -> bool;

    /// Get or create a profile
    pub fn get_or_create_profile(id: &ProfileId) -> Result<Profile>;

    /// Get the profiles root directory
    fn profiles_dir() -> Result<PathBuf>;
}
```

No state needed — this is a stateless utility that works with the filesystem.

### 3. Modify: `Config` in `clareon-core` (`config/settings.rs`)

**Add profile metadata fields to Config:**

```rust
pub struct Config {
    // NEW: Profile display metadata (stored in each profile's config.json)
    #[serde(default)]
    pub profile_name: Option<String>,       // human-friendly display name
    #[serde(default)]
    pub profile_description: Option<String>, // optional description

    // ... existing fields unchanged ...
}
```

**Remove static path methods from Config** — these move to `Profile`:
- `Config::config_path()` → removed
- `Config::database_path()` → removed
- `Config::database_url()` → removed
- `Config::cache_root()` → removed
- `Config::workspace_cache_dir()` → removed
- `Config::shared_cache_dir()` → removed

**Modify load/save to take explicit paths:**
- `Config::load()` → `Config::load_from(path)` (already exists, `load()` is removed)
- `Config::save()` → `Config::save_to(path)` (already exists, `save()` is removed)

The `project_dirs()` helper also moves to `Profile` / `ProfileManager`.

### 4. Modify: `ConfigManager` in `clareon-core` (`config/manager.rs`)

**Stop being a singleton.** Take a `Profile` at construction time:

```rust
pub struct ConfigManager {
    profile: Profile,
    config: Arc<Mutex<Config>>,
}

impl ConfigManager {
    /// Create a new ConfigManager for a specific profile
    pub fn new(profile: Profile) -> Result<Self>;

    /// Get the profile
    pub fn profile(&self) -> &Profile;

    /// Get a clone of the current configuration (unchanged)
    pub fn config(&self) -> Config;

    /// Update, save, reload — now use self.profile paths
    pub fn update_config<F>(&self, f: F) -> Result<()>;
    pub fn save(&self) -> Result<()>;
    pub fn reload(&self) -> Result<()>;
    pub fn replace_config(&self, new_config: Config);
}
```

The `static INSTANCE: OnceLock<ConfigManager>` is removed. Callers pass `ConfigManager`
(or `&Profile`) explicitly instead of calling `ConfigManager::get()`.

### 5. Modify: `SecretStore` in `clareon-core` (`config/secrets.rs`)

Add profile to secret attributes for isolation:

```rust
impl SecretStore {
    // Change attribute from:
    //   [("application", "clareon"), ("key", key)]
    // To:
    //   [("application", "clareon"), ("profile", profile_id), ("key", key)]

    pub async fn store_secret(&self, profile: &ProfileId, key: &str, value: &str) -> Result<()>;
    pub async fn get_secret(&self, profile: &ProfileId, key: &str) -> Result<String>;
    pub async fn delete_secret(&self, profile: &ProfileId, key: &str) -> Result<()>;
    pub async fn has_secret(&self, profile: &ProfileId, key: &str) -> bool;
}
```

### 6. Modify: `AnthropicBackend` in `clareon-core` (`backend/anthropic.rs`)

`from_config` needs profile awareness for secret retrieval:

```rust
pub async fn from_config(
    config: &AnthropicConfig,
    profile_id: &ProfileId,
) -> Result<Self, BackendError>;
```

### 7. Modify: `create_backend_from_config` in `clareon-core` (`backend/mod.rs`)

Pass profile through:

```rust
pub async fn create_backend_from_config(
    config: &Config,
    profile_id: &ProfileId,
) -> Result<Arc<dyn LlmBackend>, String>;
```

### 8. Modify: `ClareonService` in `clareon` (`service/mod.rs`)

Accept a `Profile` (or `ConfigManager`) instead of using the global singleton:

```rust
impl ClareonService {
    pub fn new(config_manager: Arc<ConfigManager>) -> Result<Self> {
        let config = config_manager.config();
        let profile = config_manager.profile();
        // Use profile for database_url, workspace paths, etc.
        // Use config for backend settings, tools, etc.
        // Pass profile_id to create_backend_from_config
    }
}
```

### 9. Modify: `ServiceController` in `clareon` (`service_controller.rs`)

**Remove global `SERVICE_HANDLE` OnceLock.** Instead, receive the handle during initialization:

- Remove `static SERVICE_HANDLE: OnceLock<ServiceHandle>`
- Remove `init_service_handle()`, `get_service_handle()`, `try_get_service_handle()`
- Store `ServiceHandle` in `ServiceControllerRust` struct fields
- Keep `#[qml_singleton]` for now (QML UI changes come later)
- Pass the handle through the Qt initialization path

```rust
pub struct ServiceControllerRust {
    handle: Option<ServiceHandle>,  // Set during init
}
```

The `init_service_handle` approach still works but stores into the struct
rather than a global OnceLock. For now, we can use a module-level store
that `Initialize` reads from (since cxx-qt controls construction).

### 10. Modify: `qt/mod.rs` in `clareon`

- Remove `pub use crate::service_controller::init_service_handle`
- Update however the handle is passed

### 11. Modify: `main.rs` in `clareon`

```rust
#[derive(Parser, Debug)]
pub struct Args {
    #[arg(short, long)]
    pub quick_input: bool,

    /// Profile to use (creates if doesn't exist)
    #[arg(short, long, default_value = "default")]
    pub profile: String,
}

fn main() {
    let args = Args::parse();

    // Resolve profile
    let profile_id = ProfileId::new(&args.profile);
    let profile = ProfileManager::get_or_create_profile(&profile_id)
        .expect("Failed to initialize profile");

    // Create ConfigManager for this profile (no longer a singleton)
    let config_manager = Arc::new(ConfigManager::new(profile)?);

    // Initialize logging from profile's config
    let config = config_manager.config();
    let _guard = init_logging(&config)?;

    // Unique instance check (profile-aware)
    // ...

    // Create service with profile-aware config
    let service = ClareonService::new(Arc::clone(&config_manager))?;
    // ...
}
```

### 12. Modify: `unique_app.rs` in `clareon`

Make socket path profile-aware so different profiles don't collide:

```rust
fn get_socket_path(profile_id: &ProfileId) -> PathBuf {
    // e.g., /run/user/1000/clareon-<profile>.sock
}
```

Since "only one instance of the process" is the constraint, we actually want
the socket to remain global (not per-profile) so that launching with a
different `--profile` activates the existing instance. The activation message
should include the requested profile so the running instance can report
"already running with profile X".

### 13. Modify: `config_manager.rs` (QML bridge) in `clareon`

Update to not use `ConfigManager::get()` but instead access the config manager
through the service/profile infrastructure. For now, we can use the same
module-level initialization pattern as ServiceController.

### 14. Modify: `clareon-core/src/lib.rs`

Update public exports:
- Add `ProfileId`, `Profile`, `ProfileManager`
- Keep `ConfigManager` export but it's no longer a singleton

### 15. Modify: `logging.rs` in `clareon-core`

No changes needed — `init_logging` already takes `&Config` directly.

## Files Changed (Summary)

| File | Change Type |
|------|-------------|
| `clareon-core/src/config/profile.rs` | **NEW** |
| `clareon-core/src/config/mod.rs` | Add profile module export |
| `clareon-core/src/config/settings.rs` | Remove static path methods, add metadata fields |
| `clareon-core/src/config/manager.rs` | Remove singleton, take Profile |
| `clareon-core/src/config/secrets.rs` | Add profile param to all methods |
| `clareon-core/src/backend/mod.rs` | Pass ProfileId through |
| `clareon-core/src/backend/anthropic.rs` | Accept ProfileId for secrets |
| `clareon-core/src/lib.rs` | Update exports |
| `clareon/src/main.rs` | Add --profile arg, wire profile through init |
| `clareon/src/service/mod.rs` | Accept ConfigManager instead of using global |
| `clareon/src/service_controller.rs` | Remove global SERVICE_HANDLE |
| `clareon/src/qt/mod.rs` | Update handle passing |
| `clareon/src/config_manager.rs` | Use non-singleton ConfigManager |
| `clareon/src/unique_app.rs` | Keep global socket, include profile in activation |

## Implementation Order

1. `config/profile.rs` — new Profile and ProfileManager types
2. `config/settings.rs` — remove static path methods, add metadata fields
3. `config/manager.rs` — remove singleton, accept Profile
4. `config/secrets.rs` — add profile parameter
5. `config/mod.rs` + `lib.rs` — update exports
6. `backend/anthropic.rs` + `backend/mod.rs` — pass ProfileId through
7. `service/mod.rs` — accept ConfigManager
8. `service_controller.rs` + `qt/mod.rs` — remove global handle
9. `config_manager.rs` (QML bridge) — update
10. `unique_app.rs` — profile in activation message
11. `main.rs` — wire everything together with --profile arg
12. Fix all compilation errors, run `cargo fmt` + `cargo clippy`

## Risks & Notes

- **cxx-qt constraint**: `ServiceController` uses `#[qml_singleton]` and `cxx_qt::Initialize`.
  The Initialize trait receives `Pin<&mut Self>` — we can't pass constructor args through cxx-qt.
  Solution: use a module-level `OnceLock` to stage the `ServiceHandle` before QML init (similar
  to current pattern but scoped better — not a true singleton, just a one-time init channel).

- **Test updates**: Tests that use `ConfigManager::get()` or `Config::database_url()` will need
  updating to use explicit Profile construction.

- **No migration**: Existing config/database at flat paths will be ignored after this change.
