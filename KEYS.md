# GPG Signing Keys for AXGA Releases

## Signing Key

```
Key Type:     Ed25519
Fingerprint:  (generate with: gpg --quick-generate-key "AXGA Release <axga@example.com>" ed25519 sign)
```

## Verify a Release

```sh
# Import the key
curl -fsSL https://raw.githubusercontent.com/KJ-AIML/axga-harness-agent-rs/main/KEYS | gpg --import

# Verify the signature
gpg --verify axga-v0.1.0-x86_64-linux-musl.asc axga-v0.1.0-x86_64-linux-musl

# Verify the checksum
sha256sum -c axga-v0.1.0-x86_64-linux-musl.sha256
```

## Signing Process

```sh
# 1. Create a signed tag
git tag -s v0.1.0 -m "Release v0.1.0"

# 2. Sign the release binary
gpg --detach-sign --armor axga-v0.1.0-x86_64-linux-musl

# 3. Push tag
git push origin v0.1.0
```
