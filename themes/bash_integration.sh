# OpenCode Manager bash integration (auto-generated, do not edit)
# Source user's bashrc first
[ -f ~/.bashrc ] && source ~/.bashrc

# OSC 133 shell integration for command state tracking
__opencode_prompt_command() {
    local ec=$?
    printf '\033]133;D;%d\007' "$ec"
    printf '\033]133;A\007'
}
PROMPT_COMMAND="__opencode_prompt_command${PROMPT_COMMAND:+;$PROMPT_COMMAND}"
trap 'printf "\033]133;B\007"' DEBUG
