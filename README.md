# jvm - JDK Version Manager

A CLI tool for managing multiple JDK installations on your system, similar to nvm.

## Installation

```bash
cargo install --path .
```

## Usage

### 1. Add a JDK

Register a JDK installation directory. The tool will automatically detect the version and generate aliases.

```bash
jvm add /usr/lib/jvm/java-8-temurin-jdk
jvm add /usr/lib/jvm/java-25-temurin-jdk --alias jdk25,latest
```

### 2. List registered JDKs

```bash
jvm list
```

Sample output:

```
        Path                         Version   Aliases
  ---  ---------------------------  ---------  ---
  *   /usr/lib/jvm/java-8-temurin  1.8.0_492  1.8, 1.8.0_492, 8
      /usr/lib/jvm/java-25-temurin 25.0.3     25, 25.0, 25.0.3
```

The entry marked with `*` is the currently active JDK.

### 3. Switch JDK

```bash
jvm use 8         # switch by alias
jvm use 25        # switch by major version
jvm use 25.0.3    # switch by full version
```

### 4. Remove a JDK

Unregister a JDK by version, alias, or path. This only removes the registration, the JDK directory itself is not deleted.

```bash
jvm remove 8         # remove by alias
jvm rm 25            # `rm` is a shorthand for `remove`
jvm remove 25.0.3    # remove by full version
jvm rm /usr/lib/jvm/java-8-temurin-jdk  # remove by path
```

If the JDK is currently active, removal will be rejected — switch to another version first.

### 5. Shell Integration

Add the following line to your `.bashrc` or `.zshrc` so the JDK environment is set automatically on each shell startup:

```bash
eval "$(jvm init bash)"   # bash / zsh
eval "$(jvm init zsh)"
```

For fish users:

```fish
jvm init fish | source
```

## Alias Rules

| JDK Version | Auto-generated Aliases |
|-------------|----------------------|
| 1.8.0_492 | `1.8.0_492`, `1.8`, `8` |
| 17.0.2 | `17.0.2`, `17.0`, `17` |
| 25.0.3 | `25.0.3`, `25.0`, `25` |

Custom aliases can be added via the `--alias` flag during `jvm add`.

## How It Works

### Directory Layout (XDG Base Directory Specification)

| Purpose | Path | Controlled By |
|---------|------|---------------|
| Configuration (JDK metadata) | `${XDG_CONFIG_HOME:-$HOME/.config}/jvm/config.json` | `JVM_DIR` / `XDG_CONFIG_HOME` |
| Runtime (current symlink) | `${XDG_RUNTIME_DIR:-...}/jvm/current` | `JVM_DIR` / `XDG_RUNTIME_DIR` |

- `$JVM_DIR` overrides all paths (backwards-compatible); when unset, paths follow XDG specs
- The `current` symlink is created in `$XDG_RUNTIME_DIR/jvm/` (e.g. `/run/user/$UID/jvm/current`), falling back to the config directory when `$XDG_RUNTIME_DIR` is not set
- The shell hook reads this symlink and sets `JAVA_HOME` and `PATH`
