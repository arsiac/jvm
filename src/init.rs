fn generate_posix_hook(shell_name: &str) -> String {
    format!(
        r#"if [ -n "$JVM_DIR" ]; then
    __jvm_current="$JVM_DIR/current"
elif [ -n "$XDG_RUNTIME_DIR" ]; then
    __jvm_current="$XDG_RUNTIME_DIR/jvm/current"
elif [ -n "$XDG_DATA_HOME" ]; then
    __jvm_current="$XDG_DATA_HOME/jvm/current"
else
    __jvm_current="$HOME/.local/share/jvm/current"
fi
case ":$PATH:" in
    *":$__jvm_current/bin:"*) ;;
    *) export PATH="$__jvm_current/bin:$PATH" ;;
esac
if [ "$JAVA_HOME" != "$__jvm_current" ]; then
    export JAVA_HOME="$__jvm_current"
fi
hash -r 2>/dev/null || true
# Usage:
#   Add the following to your ~/.{shell_rc}:
#       eval "$(jvm init {shell_name})"#,
        shell_rc = if shell_name == "bash" { "bashrc" } else { "zshrc" },
        shell_name = shell_name,
    )
}

pub fn generate_bash_hook() -> String {
    generate_posix_hook("bash")
}

pub fn generate_zsh_hook() -> String {
    generate_posix_hook("zsh")
}

pub fn generate_fish_hook() -> String {
    r#"if set -q JVM_DIR
    set __jvm_current "$JVM_DIR/current"
else if set -q XDG_RUNTIME_DIR
    set __jvm_current "$XDG_RUNTIME_DIR/jvm/current"
else if set -q XDG_DATA_HOME
    set __jvm_current "$XDG_DATA_HOME/jvm/current"
else
    set __jvm_current "$HOME/.local/share/jvm/current"
end
if not contains "$__jvm_current/bin" $PATH
    set -x PATH "$__jvm_current/bin" $PATH
end
if test "$JAVA_HOME" != "$__jvm_current"
    set -x JAVA_HOME "$__jvm_current"
end
# Usage:
#   Add the following to your config.fish:
#       jvm init fish | source
"#
    .to_string()
}

pub fn generate_powershell_hook() -> String {
    r#"if ($env:JVM_DIR) {
    $__jvm_current = Join-Path $env:JVM_DIR "current"
} elseif ($env:XDG_RUNTIME_DIR) {
    $__jvm_current = [System.IO.Path]::Combine($env:XDG_RUNTIME_DIR, "jvm", "current")
} elseif ($env:APPDATA) {
    $__jvm_current = [System.IO.Path]::Combine($env:APPDATA, "jvm", "current")
} else {
    $__jvm_current = [System.IO.Path]::Combine($HOME, ".local", "share", "jvm", "current")
}

$__sep = [System.IO.Path]::PathSeparator
$__jvm_bin = Join-Path $__jvm_current "bin"
if ($env:PATH -split $__sep -notcontains $__jvm_bin) {
    $env:PATH = "$__jvm_bin$__sep$env:PATH"
}

if ($env:JAVA_HOME -ne $__jvm_current) {
    $env:JAVA_HOME = $__jvm_current
}

<#
Usage:
  Add the following to your PowerShell profile ($PROFILE):

      jvm init powershell | Out-String | Invoke-Expression
#>"#
    .to_string()
}

pub fn generate_hook(shell: &str) -> Result<String, String> {
    match shell {
        "bash" => Ok(generate_bash_hook()),
        "zsh" => Ok(generate_zsh_hook()),
        "fish" => Ok(generate_fish_hook()),
        "powershell" => Ok(generate_powershell_hook()),
        other => Err(format!("unsupported shell type: {}", other)),
    }
}
