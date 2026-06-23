pub fn generate_bash_hook() -> String {
    r#"__jvm_dir="${JVM_DIR:-$HOME/.config/jvm}"
__jvm_current="$__jvm_dir/current"
if [ -L "$__jvm_current" ] && [ -d "$__jvm_current" ]; then
    export JAVA_HOME="$__jvm_current"
    case ":$PATH:" in
        *":$JAVA_HOME/bin:"*) ;;
        *) export PATH="$JAVA_HOME/bin:$PATH" ;;
    esac
fi"#
    .to_string()
}

pub fn generate_zsh_hook() -> String {
    r#"__jvm_dir="${JVM_DIR:-$HOME/.config/jvm}"
__jvm_current="$__jvm_dir/current"
if [ -L "$__jvm_current" ] && [ -d "$__jvm_current" ]; then
    export JAVA_HOME="$__jvm_current"
    case ":$PATH:" in
        *":$JAVA_HOME/bin:"*) ;;
        *) export PATH="$JAVA_HOME/bin:$PATH" ;;
    esac
fi"#
    .to_string()
}

pub fn generate_fish_hook() -> String {
    r#"set -q JVM_DIR; or set JVM_DIR "$HOME/.config/jvm"
set __jvm_current "$JVM_DIR/current"
if test -L "$__jvm_current" -a -d "$__jvm_current"
    set -x JAVA_HOME "$__jvm_current"
    if not contains "$JAVA_HOME/bin" $PATH
        set -x PATH "$JAVA_HOME/bin" $PATH
    end
end"#
    .to_string()
}

pub fn generate_hook(shell: &str) -> Result<String, String> {
    match shell {
        "bash" => Ok(generate_bash_hook()),
        "zsh" => Ok(generate_zsh_hook()),
        "fish" => Ok(generate_fish_hook()),
        other => Err(format!("unsupported shell type: {}", other)),
    }
}
