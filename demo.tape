# postlab TUI demo — renders a GIF for the README.
#
# Render with:  vhs demo.tape   →   produces demo.gif
# Requires:     charmbracelet/vhs  (https://github.com/charmbracelet/vhs)
#               and passwordless sudo (or run vhs as root), since postlab
#               must run as root.

Output demo.gif

Set Shell bash
Set FontSize 16
Set Width 1280
Set Height 720
Set Theme "Dracula"
Set Padding 20
Set TypingSpeed 60ms

# Launch the TUI
Type "sudo postlab"
Sleep 600ms
Enter
Sleep 3s

# 1 — Dashboard (default): live system gauges + processes
Sleep 2500ms

# 2 — Packages: cycle a couple of tabs (Installed → Search → Updates)
Type "2"
Sleep 2s
Right
Sleep 1500ms
Right
Sleep 1500ms

# 3 — Security: hardening audits
Type "3"
Sleep 2s

# 4 — Networking: Gateway / Tunnel / Tailscale
Type "4"
Sleep 2s
Right
Sleep 1500ms

# 5 — Docker: containers and managed workloads
Type "5"
Sleep 2500ms

# 7 — Agent: chat / tools / skills
Type "7"
Sleep 2500ms

# Back to the Dashboard to close on the hero screen
Type "1"
Sleep 2s

# Quit
Type "q"
Sleep 800ms
