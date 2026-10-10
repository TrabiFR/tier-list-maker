# Sourced by the build scripts. Rust embeds source file paths in release binaries
# (for panic messages), which would include this computer's user name (/Users/<name>/...).
# Remap them to neutral placeholders so no personal folder names ship in the downloads.
# When several prefixes match, rustc uses the last one, so the general $HOME rule comes first.
remap_flags=(
  "--remap-path-prefix=$HOME=/home"
  "--remap-path-prefix=$HOME/.cargo=/cargo"
  "--remap-path-prefix=$HOME/.rustup=/rustup"
  "--remap-path-prefix=$PWD=/build"
)
CARGO_ENCODED_RUSTFLAGS="$(IFS=$'\x1f'; echo "${remap_flags[*]}")"
export CARGO_ENCODED_RUSTFLAGS
