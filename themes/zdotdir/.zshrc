# OpenCode Manager shell wrapper (auto-generated, do not edit)
# Restore original ZDOTDIR so user's config paths resolve correctly
if [[ -n "$OPENCODE_ORIG_ZDOTDIR" ]]; then
  export ZDOTDIR="$OPENCODE_ORIG_ZDOTDIR"
  unset OPENCODE_ORIG_ZDOTDIR
else
  unset ZDOTDIR
fi

# Source user's original zshenv (if not already sourced by zsh startup)
[[ -f "/Users/aditya.upadhyay/.zshenv" ]] && source "/Users/aditya.upadhyay/.zshenv"

# Source global zshrc
[[ -f "/etc/zshrc" ]] && source "/etc/zshrc"

# Source user's original zshrc
if [[ -n "$ZDOTDIR" ]] && [[ -f "$ZDOTDIR/.zshrc" ]]; then
  source "$ZDOTDIR/.zshrc"
elif [[ -f "/Users/aditya.upadhyay/.zshrc" ]]; then
  source "/Users/aditya.upadhyay/.zshrc"
fi

# Apply OpenCode theme (LAST, so our colors override)
[[ -f "/Users/aditya.upadhyay/.config/opencode-manager/themes/opencode.zsh" ]] && source "/Users/aditya.upadhyay/.config/opencode-manager/themes/opencode.zsh"

# Shell integration: emit OSC 133 sequences for command state tracking
__opencode_preexec() { printf '\x1b]133;B\x07' }
__opencode_precmd() {
  local ec=$?
  printf '\x1b]133;D;%d\x07' "$ec"
  printf '\x1b]133;A\x07'
}
autoload -Uz add-zsh-hook
add-zsh-hook precmd __opencode_precmd
add-zsh-hook preexec __opencode_preexec
