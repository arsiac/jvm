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

For PowerShell users (Windows):

```powershell
jvm init powershell | Out-String | Invoke-Expression
```

Add the line above to your PowerShell profile (`$PROFILE`) to make it permanent.

### 6. Shell Completion

Enable tab-completion for `jvm` commands:

**bash**:
```bash
source <(jvm completion bash)
```
Add the line above to your `~/.bashrc` to make it permanent.

**zsh**:
```zsh
source <(jvm completion zsh)
```
Add the line above to your `~/.zshrc` to make it permanent.

**fish**:
```fish
jvm completion fish | source
```

**PowerShell**:
```powershell
jvm completion powershell | Out-String | Invoke-Expression
```

## Alias Rules

| JDK Version | Auto-generated Aliases |
|-------------|----------------------|
| 1.8.0_492 | `1.8.0_492`, `1.8`, `8` |
| 17.0.2 | `17.0.2`, `17.0`, `17` |
| 25.0.3 | `25.0.3`, `25.0`, `25` |

Custom aliases can be added via the `--alias` flag during `jvm add`.

## How It Works

### Directory Layout

| Purpose | Linux / macOS | Windows |
|---------|---------------|---------|
| Configuration (JDK metadata) | `${XDG_CONFIG_HOME:-$HOME/.config}/jvm/config.json` | `%APPDATA%\jvm\config.json` |
| Runtime (current symlink) | `${XDG_RUNTIME_DIR:-${XDG_DATA_HOME:-$HOME/.local/share}}/jvm/current` | `%APPDATA%\jvm\current` |

All paths can be overridden by setting `$JVM_DIR` (works on all platforms).  
The shell hook reads the `current` symlink/junction and sets `JAVA_HOME` and `PATH`.
