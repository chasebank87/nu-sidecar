# nu-sidecar: bashify keybinding + llmchat context capture + wrapper defs.
#
# Sourced from config.nu by install.nu. Assumes both `nu_plugin_bashify` and
# `nu_plugin_llmchat` have already been registered (`plugin add` + `plugin use`).

# ---------------------------------------------------------------------------
# Plugin 1: bashify -- translate-then-confirm on Enter
# ---------------------------------------------------------------------------

def sidecar-bashify-enter [] {
    let buf = (commandline)
    if ($buf | str trim) == "" {
        commandline edit --replace $buf --accept
        return
    }

    if ($env.SIDECAR_BASHIFY_PENDING? == $buf) {
        # Second Enter on an already-translated, unedited buffer: run it.
        $env.SIDECAR_BASHIFY_PENDING = null
        commandline edit --replace $buf --accept
        return
    }

    let result = (bashify $buf)
    if $result == $buf {
        # Nothing to translate -- run immediately, no extra Enter required.
        $env.SIDECAR_BASHIFY_PENDING = null
        commandline edit --replace $buf --accept
    } else {
        # Show the translation; a second Enter (unedited) will run it. If the
        # user edits the translated buffer first, this branch re-fires and
        # re-translates the edited text instead of running stale output.
        $env.SIDECAR_BASHIFY_PENDING = $result
        commandline edit --replace $result
    }
}

$env.config.keybindings ++= [
    {
        name: sidecar_bashify_enter
        modifier: none
        keycode: enter
        mode: [emacs, vi_insert, vi_normal]
        event: { send: executehostcommand, cmd: "sidecar-bashify-enter" }
    }
]

# ---------------------------------------------------------------------------
# Plugin 2: llmchat -- ergonomic wrapper defs + context-capture hooks
# ---------------------------------------------------------------------------

# `ask`/`plan` are plain `def`s (not the raw plugin commands) so they can
# inject the current session's $nu.pid automatically -- the plugin commands
# themselves (`sidecar ask`/`sidecar plan`) require it explicitly and aren't
# meant to be called directly.
def ask [
    question: string
    --last: int # only include the last N terminal turns as context
] {
    if $last == null {
        sidecar ask $question --session-pid $nu.pid
    } else {
        sidecar ask $question --session-pid $nu.pid --last $last
    }
}

def plan [
    goal: string
    --last: int # only include the last N terminal turns as context
] {
    if $last == null {
        sidecar plan $goal --session-pid $nu.pid
    } else {
        sidecar plan $goal --session-pid $nu.pid --last $last
    }
}

# Context capture: a 3-hook handoff, because `display_output` alone never
# fires for external-command output (nushell issue #14137) -- most real
# terminal usage (git, cat, docker, curl, rg, ...) is external commands, so
# relying on display_output in isolation would silently miss almost
# everything. pre_execution stashes the raw command text; display_output
# (when it does fire, for internal commands) stashes the rendered output;
# pre_prompt -- which fires after *every* command, unlike env_change, which
# only fires when a watched value changes and would silently drop repeated
# identical exit codes -- does the one write and clears both.
$env.config.hooks.pre_execution ++= [
    { ||
        try {
            $env.SIDECAR_PENDING_CMD = (commandline)
            $env.SIDECAR_PENDING_OUTPUT = null
        }
    }
]

# Replaces the display_output hook with a version that also stashes a
# plain-text copy for the transcript. NOTE: this overwrites any
# display_output hook set earlier in config.nu -- keep the rendering logic
# below in sync with whatever you'd otherwise set there (currently: plain
# `table`, matching this config's own `{|| table }`).
$env.config.hooks.display_output = { ||
    let rendered = ($in | table)
    try {
        $env.SIDECAR_PENDING_OUTPUT = ($rendered | into string | ansi strip)
    }
    $rendered
}

$env.config.hooks.pre_prompt ++= [
    { ||
        try {
            if $env.SIDECAR_PENDING_CMD? != null {
                # The Enter keybinding above runs via `executehostcommand`,
                # which nushell treats as its own top-level command -- so it
                # gets its own pre_execution/pre_prompt cycle with `commandline`
                # literally equal to "sidecar-bashify-enter", *before* the
                # real line it accepts runs through its own cycle. Skip
                # logging that internal marker so the transcript holds only
                # commands you actually typed.
                if $env.SIDECAR_PENDING_CMD != "sidecar-bashify-enter" {
                    let output = ($env.SIDECAR_PENDING_OUTPUT? | default "[external command -- stdout not captured]")
                    sidecar log-turn --session-pid $nu.pid --cmd $env.SIDECAR_PENDING_CMD --output $output --exit-code (
                        $env.LAST_EXIT_CODE? | default 0
                    )
                }
                $env.SIDECAR_PENDING_CMD = null
                $env.SIDECAR_PENDING_OUTPUT = null
            }
        }
    }
]
