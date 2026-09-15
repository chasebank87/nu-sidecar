# Idempotent installer: builds + registers both plugins, then wires the
# keybinding/hooks into your config.nu. Safe to re-run after an upgrade.
#
# Usage: nu install.nu

let repo_root = ($env.FILE_PWD | default (pwd))
let sidecar_nu = ($repo_root | path join "nu" "sidecar.nu" | path expand)

print "Building and installing nu_plugin_bashify and nu_plugin_llmchat..."
cargo install --path ($repo_root | path join "crates" "nu_plugin_bashify") --locked
cargo install --path ($repo_root | path join "crates" "nu_plugin_llmchat") --locked

let cargo_bin = ($env.CARGO_HOME? | default ($nu.home-dir | path join ".cargo") | path join "bin")
let bashify_bin = ($cargo_bin | path join "nu_plugin_bashify")
let llmchat_bin = ($cargo_bin | path join "nu_plugin_llmchat")

print "Registering plugins with nushell..."
plugin add $bashify_bin
plugin add $llmchat_bin
plugin use bashify
plugin use llmchat

let marker_start = "# >>> nu-sidecar >>>"
let marker_end = "# <<< nu-sidecar <<<"
let config_path = $nu.config-path

let existing = (
    if ($config_path | path exists) {
        open --raw $config_path
    } else {
        ""
    }
)

if ($existing | str contains $marker_start) {
    print $"Already wired into ($config_path) -- skipping (edit the nu-sidecar block there, or nu/sidecar.nu, directly)."
} else {
    let block = $"\n($marker_start)\nsource \"($sidecar_nu)\"\n($marker_end)\n"
    $"($existing)($block)" | save --force $config_path
    print $"Wired the bashify keybinding and llmchat hooks into ($config_path)."
}

print ""
print "Done. Restart nushell, then:"
print "  llm setup      # one-time: pick a provider (ollama, lmStudio, openRouter, hermes, openClaw)"
print "  ask \"hello\"    # try it"
print "  export FOO=bar && echo $FOO   # try the bashify translate-then-confirm keybinding"
