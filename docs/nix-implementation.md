# Nix Flake Implementation Details

This document explains how the Nix flake implements `install.sh` features with fallback support.

## Fallback Implementation

### The Challenge

Nix's evaluation model differs from shell scripts:

```bash
# install.sh (runtime fallback)
download_variant "cuda-sm86"    # Try this first
if failed; then
  download_variant "cuda-sm75"  # Fallback
  if failed; then
    download_variant "cpu"      # Last resort
  fi
fi
```

Nix can't "try" downloads at build time - it needs hashes upfront. Our solution uses **declarative fallback** at the package selection level.

### Solution: Multi-Layer Fallback

#### 1. **Build Time: Hash Registry**

All variant hashes declared upfront:

```nix
variantHashes = {
  cpu = {
    x86_64-linux = "sha256-...";
    aarch64-linux = "sha256-...";
  };
  cuda-cudnn-sm86 = {
    x86_64-linux = "sha256-...";
    aarch64-linux = null;  # Not available
  };
};
```

If hash is `null` or `fakeSha256`, variant isn't available.

#### 2. **Evaluation Time: Fallback Chain**

```nix
getFallbackChain = variant:
  if hasPrefix "cuda-cudnn-sm" variant then
    [ variant "cuda-cudnn-sm75" "cpu" ]
  else if hasPrefix "cuda-sm" variant then
    [ variant "cuda-sm75" "cpu" ]
  else
    [ variant ];

# Example: cuda-cudnn-sm90 -> [ "cuda-cudnn-sm90" "cuda-cudnn-sm75" "cpu" ]
```

#### 3. **Home Manager: Package Selection**

```nix
selectPackageWithFallback = variant:
  let
    fallbackChain = getFallbackChain variant;

    tryVariants = variants:
      if variants == [] then
        # Ultimate fallback: CPU
        self.packages.${system}.default
      else
        let
          current = head variants;
          pkg = "super-stt-${current}";
          result = tryEval (self.packages.${system}.${pkg});
        in
        if result.success then result.value
        else tryVariants (tail variants);
  in
  tryVariants fallbackChain;
```

Uses `tryEval` to catch build failures and try next variant.

## Comparison: install.sh vs Nix

| Feature | install.sh | Nix Flake |
|---------|-----------|-----------|
| **Fallback Method** | Runtime download retry | Compile-time package selection |
| **When Fallback Occurs** | During download | During evaluation/build |
| **User Feedback** | "Trying fallback..." | Build error or silent fallback |
| **Flexibility** | Can try any URL | Limited to declared packages |
| **Reproducibility** | ❌ Depends on server | ✅ Fully reproducible |
| **Network Efficiency** | ❌ Multiple downloads | ✅ Single download |

## Usage Examples

### 1. **Auto-detection with Fallback (Home Manager)**

```nix
services.super-stt = {
  enable = true;
  variant = "auto";  # Detects GPU -> tries chain -> falls back to CPU
};
```

Behavior:
1. Detects NVIDIA GPU → selects `cuda-cudnn-sm86`
2. Tries to build `super-stt-cuda-cudnn-sm86`
3. If that fails (hash missing/wrong) → tries `super-stt-cuda-cudnn-sm75`
4. If that fails → uses `super-stt-cpu`

### 2. **Explicit Variant with Fallback**

```nix
services.super-stt = {
  enable = true;
  variant = "cuda-cudnn-sm90";  # Request Hopper H100
  package = null;  # Enable fallback
};
```

Fallback chain: `sm90 → sm75 → cpu`

### 3. **No Fallback (Explicit Package)**

```nix
services.super-stt = {
  enable = true;
  package = pkgs.super-stt-cuda-cudnn-sm86;  # Hard requirement
};
```

Build fails if package unavailable - no fallback.

## Error Messages

### Build Failure (Hash Mismatch)

```
error: hash mismatch in fixed-output derivation
  specified: sha256-AAAA...
  got:       sha256-BBBB...
```

Solution: Update hash in `variantHashes`

### Variant Not Available

```
Variant "cuda-cudnn-sm90" is not available for x86_64-linux.
Available variants: cpu, cuda-cudnn-sm75, cuda-cudnn-sm86
Fallback chain would be: cuda-cudnn-sm90 -> cuda-cudnn-sm75 -> cpu
```

Solution: Either:
1. Add hash for the variant
2. Let fallback handle it (if `package = null`)
3. Use explicit available variant

## Updating Hashes

When a new release is published:

```bash
# Get hash for each variant
nix-prefetch-url https://github.com/jorge-menjivar/super-stt/releases/download/v0.2.0/super-stt-x86_64-unknown-linux-gnu-cpu.tar.gz
# Output: sha256-xyz123...

nix-prefetch-url https://github.com/jorge-menjivar/super-stt/releases/download/v0.2.0/super-stt-x86_64-unknown-linux-gnu-cuda-cudnn-sm86.tar.gz
# Output: sha256-abc456...

# Update flake.nix
variantHashes = {
  cpu = {
    x86_64-linux = "sha256-xyz123...";
  };
  cuda-cudnn-sm86 = {
    x86_64-linux = "sha256-abc456...";
  };
};
```

Or use the helper script (TODO):

```bash
./scripts/update-hashes.sh v0.2.0
```

## Advantages of This Approach

### 1. **Graceful Degradation**

User requests RTX 40xx variant but hash isn't updated yet:
- ✅ Falls back to RTX 20xx variant (still uses GPU)
- ✅ Eventually falls back to CPU (still works)
- ❌ Never silently uses wrong variant

### 2. **Explicit Errors**

If user explicitly sets package and it fails:
- Clear error message
- Shows available alternatives
- Shows what fallback chain would be

### 3. **Reproducible**

Same config always produces same result:
- Hashes ensure exact binaries
- Fallback logic is deterministic
- No network dependencies after first build

### 4. **Nix-Native**

Uses Nix's built-in features:
- `tryEval` for safe evaluation
- `fetchurl` for reproducible downloads
- Derivation system for caching

## Limitations vs install.sh

### Cannot Do

1. **Network-based fallback**: Can't try downloading unknown URLs
2. **Runtime GPU detection**: Must specify variant at config time
3. **Interactive prompts**: All decisions made declaratively

### Can Do Better

1. **Rollback**: `nixos-rebuild switch --rollback`
2. **Testing**: `nixos-rebuild build` before applying
3. **Per-user variants**: Different users can use different GPU variants
4. **Atomic updates**: All-or-nothing installation

## Future Improvements

### 1. **GPU Auto-Detection**

```nix
# Detect actual GPU compute capability
autoVariant = let
  gpuInfo = builtins.readFile /proc/driver/nvidia/gpus/.../information;
  computeCap = extractComputeCap gpuInfo;
in "cuda-cudnn-sm${computeCap}";
```

### 2. **Hash Automation**

```nix
# Auto-fetch hash if not in registry
src = if hash != null then fetchurl { inherit url; sha256 = hash; }
      else fetchurl { inherit url; };  # Impure, but works
```

### 3. **Better Error Messages**

```nix
# Show which variant was selected
home.activation.super-stt-info = ''
  echo "Super STT installed: ${selectedPackage.name}"
  echo "Variant: ${selectedPackage.actualVariant or "unknown"}"
'';
```

### 4. **Variant Recommendation**

```nix
# Analyze hardware and recommend variant
warnings = if hasNvidia && variant == "cpu" then
  [ "NVIDIA GPU detected but using CPU variant. Consider: cuda-cudnn-sm86" ]
else [];
```

## Summary

The Nix implementation achieves **similar robustness** to `install.sh`'s fallback logic while maintaining **Nix's reproducibility guarantees**. Instead of runtime download retry, it uses compile-time package selection with deterministic fallback chains.

The trade-off:
- **Lost**: Runtime flexibility, automatic URL discovery
- **Gained**: Reproducibility, rollback, atomic updates, per-user variants
