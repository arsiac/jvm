pub fn generate_bash_hook() -> String {
    r#"if [ -n "$JVM_DIR" ]; then
    __jvm_current="$JVM_DIR/current"
elif [ -n "$XDG_RUNTIME_DIR" ]; then
    __jvm_current="$XDG_RUNTIME_DIR/jvm/current"
else
    __jvm_current="${XDG_CONFIG_HOME:-$HOME/.config}/jvm/current"
fi
case ":$PATH:" in
    *":$__jvm_current/bin:"*) ;;
    *) export PATH="$__jvm_current/bin:$PATH" ;;
esac
if [ "$JAVA_HOME" != "$__jvm_current" ]; then
    export JAVA_HOME="$__jvm_current"
fi
hash -r 2>/dev/null || true"#
    .to_string()
}

pub fn generate_zsh_hook() -> String {
    r#"if [ -n "$JVM_DIR" ]; then
    __jvm_current="$JVM_DIR/current"
elif [ -n "$XDG_RUNTIME_DIR" ]; then
    __jvm_current="$XDG_RUNTIME_DIR/jvm/current"
else
    __jvm_current="${XDG_CONFIG_HOME:-$HOME/.config}/jvm/current"
fi
case ":$PATH:" in
    *":$__jvm_current/bin:"*) ;;
    *) export PATH="$__jvm_current/bin:$PATH" ;;
esac
if [ "$JAVA_HOME" != "$__jvm_current" ]; then
    export JAVA_HOME="$__jvm_current"
fi
hash -r 2>/dev/null || true"#
    .to_string()
}

pub fn generate_fish_hook() -> String {
    r#"if set -q JVM_DIR
    set __jvm_current "$JVM_DIR/current"
else if set -q XDG_RUNTIME_DIR
    set __jvm_current "$XDG_RUNTIME_DIR/jvm/current"
else
    if set -q XDG_CONFIG_HOME
        set __jvm_current "$XDG_CONFIG_HOME/jvm/current"
    else
        set __jvm_current "$HOME/.config/jvm/current"
    end
end
if not contains "$__jvm_current/bin" $PATH
    set -x PATH "$__jvm_current/bin" $PATH
end
if test "$JAVA_HOME" != "$__jvm_current"
    set -x JAVA_HOME "$__jvm_current"
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
