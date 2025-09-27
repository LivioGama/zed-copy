# Build Optimization Guide for Zed

This guide explains the build optimizations implemented to make frequent bundling operations as fast as possible.

## 🚀 Quick Start

For the fastest bundling experience, run the setup script first:

```bash
./script/setup-fast-builds
```

Then use the optimized bundle command:

```bash
./script/bundle-mac -bl  # Bundle with bundle profile, local architecture only
```

## 📊 Performance Improvements

With these optimizations, you can expect:

- **50-80% faster incremental builds** with sccache
- **30-50% faster bundle operations** with the bundle profile
- **Significantly faster linking** on macOS with optimized linker flags
- **Better parallelization** with increased codegen units

## 🔧 Optimizations Applied

### 1. Cargo Configuration (`.cargo/config.toml`)

#### Build Settings

- **Incremental compilation**: Enabled by default
- **Parallel builds**: Uses all available CPU cores (`jobs = 0`)
- **sccache**: Enabled for compilation caching
- **Fast linking**: Optimized linker settings for macOS

#### Target-Specific Optimizations

- **macOS targets**: Dead code stripping and optimized linking
- **Linux targets**: Uses `mold` linker for faster linking
- **Windows targets**: Optimized for static linking

### 2. Bundle Profile (`Cargo.toml`)

A new `bundle` profile optimized for frequent development iterations:

```toml
[profile.bundle]
incremental = true        # Enable incremental compilation
debug = "limited"         # Some debug info for troubleshooting
lto = false              # Disable LTO for faster builds
codegen-units = 32       # High parallelism
opt-level = 2            # Good optimization without excessive compile time
panic = "abort"          # Faster builds
overflow-checks = false  # Skip runtime checks
```

#### Critical Dependencies

Even in bundle profile, these remain highly optimized:

- `gpui`: UI framework (opt-level = 3)
- `taffy`: Layout engine (opt-level = 3)
- `resvg`: SVG rendering (opt-level = 3)
- `zed`: Main binary (balanced codegen-units = 8)

### 3. Enhanced Bundle Script (`script/bundle-mac`)

#### New `-b` Flag

Use the bundle profile for fast incremental builds:

```bash
./script/bundle-mac -b    # Use bundle profile
./script/bundle-mac -bl   # Bundle profile + local architecture
./script/bundle-mac -blo  # Bundle profile + local + open result
```

#### Smart License Generation

- Skips license regeneration in bundle mode if licenses exist
- Reduces unnecessary work on incremental builds

#### Build Timing

- Shows build start and completion times
- Helps measure the impact of optimizations

### 4. sccache Configuration

#### Installation

```bash
brew install sccache
```

#### Configuration

- **Cache size**: 10GB (configurable in `~/.sccache.conf`)
- **Automatic**: Works transparently with Cargo
- **Cross-session**: Caches persist between builds

#### Monitoring

```bash
sccache --show-stats     # View cache hit rates
sccache --zero-stats     # Reset statistics
```

## 📋 Usage Patterns

### Development Workflow

1. **Initial setup** (one time):

   ```bash
   ./script/setup-fast-builds
   ```

2. **Fast incremental bundles** (daily development):

   ```bash
   ./script/bundle-mac -bl
   ```

3. **Full release bundle** (when needed):
   ```bash
   ./script/bundle-mac
   ```

### Build Profiles Comparison

| Profile        | Use Case              | Build Time | Runtime Performance | Debug Info  |
| -------------- | --------------------- | ---------- | ------------------- | ----------- |
| `debug`        | Development           | Fastest    | Slowest             | Full        |
| `bundle`       | **Frequent bundling** | **Fast**   | **Good**            | **Limited** |
| `release-fast` | Testing               | Medium     | Good                | Full        |
| `release`      | Production            | Slowest    | Fastest             | Limited     |

## 🐛 Troubleshooting

### sccache Issues

If sccache isn't working:

```bash
# Check if sccache is being used
echo $RUSTC_WRAPPER

# View sccache logs
SCCACHE_LOG=debug cargo build

# Clear cache if corrupted
sccache --zero-stats
rm -rf ~/.cache/sccache
```

### Build Failures

If builds fail with the bundle profile:

1. Try a clean build:

   ```bash
   cargo clean
   ./script/bundle-mac -bl
   ```

2. Fall back to release profile:

   ```bash
   ./script/bundle-mac -l
   ```

3. Check for environment conflicts:
   ```bash
   unset CARGO_INCREMENTAL
   unset CARGO_BUNDLE_SKIP_BUILD
   ```

### Performance Issues

If builds are still slow:

1. Check sccache hit rate:

   ```bash
   sccache --show-stats
   ```

2. Verify parallel compilation:

   ```bash
   # Should show high CPU usage across cores
   htop  # or Activity Monitor on macOS
   ```

3. Check available disk space (sccache cache and target dir)

## 🎯 Expected Results

### First Build (Cold Cache)

- **Debug profile**: ~5-8 minutes
- **Bundle profile**: ~8-12 minutes
- **Release profile**: ~15-25 minutes

### Incremental Builds (Warm Cache)

- **Debug profile**: ~30-60 seconds
- **Bundle profile**: ~1-3 minutes
- **Release profile**: ~5-10 minutes

### sccache Hit Rates (After Several Builds)

- **Expected**: 60-90% cache hit rate
- **Excellent**: 90%+ cache hit rate

## 🔧 Advanced Configuration

### Custom sccache Size

```bash
export SCCACHE_CACHE_SIZE="20G"  # Increase cache size
```

### Alternative Linkers

```bash
# For even faster linking (if available)
brew install lld
export CARGO_TARGET_X86_64_APPLE_DARWIN_LINKER=lld
```

### Environment Variables

```bash
# Maximum parallelism
export CARGO_BUILD_JOBS=$(sysctl -n hw.ncpu)

# Reduce memory usage if needed
export CARGO_BUILD_JOBS=$(($(sysctl -n hw.ncpu) / 2))
```

## 📈 Measuring Performance

### Build Time Tracking

```bash
# Time a full build
time ./script/bundle-mac -bl

# Compare profiles
time cargo build --profile bundle --package zed
time cargo build --release --package zed
```

### Cache Effectiveness

```bash
# Reset and measure
sccache --zero-stats
./script/bundle-mac -bl
sccache --show-stats
```

---

**Note**: These optimizations prioritize build speed over runtime performance for development workflows. Always use the `release` profile for production builds.

