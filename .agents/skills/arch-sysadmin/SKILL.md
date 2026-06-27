---
name: arch-sysadmin
description: Arch Linux system administration. Use when the user asks to manage services, install packages, configure networking, inspect logs, manage users, tune kernel parameters, secure a server, or diagnose system issues on Arch. Covers pacman, AUR helpers (yay/paru), systemd, firewalld/nftables, systemd-networkd, journalctl, mkinitcpio, reflector, SSH hardening, and common diagnostic workflows.
---

# Arch Linux System Administration

Arch is a rolling-release distribution. There are no version upgrades — packages are continuously updated. This changes key workflows compared to fixed-release distros.

## Execution Model

Pi executes tool calls sequentially, even when you emit multiple calls in one turn. Batch independent calls in a single turn to save round-trips:

| Pattern | Use for |
|---------|---------|
| Multiple bash calls in one turn | Independent diagnostics (df & free & uptime) |
| `read /path & read /other` via bash | Read multiple files concurrently |

## Step 1: Classify the Request

| Type | Trigger | Primary Approach |
|------|---------|-----------------|
| **Package Ops** | Install, remove, update, search packages | pacman + yay/paru for AUR |
| **Service Ops** | Start, stop, restart, enable, check status | systemctl + journalctl |
| **Diagnostic** | "Why is X slow?", "What's using Y?", "Check Z" | top/free/df/ss + targeted log inspection |
| **Network** | "Configure IP", "Fix DNS", "Check port" | ip/ss/systemd-networkd/resolvectl |
| **Security** | "Harden", "Audit", "Check firewall", "SSH setup" | firewalld/nftables/fail2ban/SSH config |
| **User/Access** | "Add user", "Groups", "Permissions" | useradd/usermod/chown/chmod |
| **Storage** | "Add disk", "Mount", "Check space", "LVM" | lsblk/df/mount/fdisk/LVM tools |
| **Kernel/Tuning** | "Tune sysctl", "Check modules", "Boot issues" | sysctl/modprobe/dmesg/mkinitcpio |
| **Mirrors** | "Slow downloads", "Update mirrors" | reflector/pacman-mirrors |
| **AUR** | "Install from AUR", "AUR package broken" | yay/paru + makepkg for manual builds |

## Step 2: Run Commands

Always use `sudo` for privileged operations. Check the current user first with `whoami`. For AUR builds, do NOT use sudo — makepkg refuses to run as root.

### Package Management (pacman)

```bash
# System update (always update before installing)
pacman -Syu                         # Full system upgrade
pacman -Syu --noconfirm             # Skip confirmation prompts

# Searching and info
pacman -Ss <keyword>                # Search repos
pacman -Si <pkg>                    # Package info
pacman -Qi <pkg>                    # Info for installed package
pacman -Qs <keyword>                # Search installed packages
pacman -Ql <pkg>                    # List files owned by package
pacman -Qo /path/to/file            # Which package owns a file
pacman -Qdt                         # Orphans (unused dependencies)
pacman -Qe                          # Explicitly installed packages
pacman -Qm                          # Foreign packages (AUR/manual)

# Install / remove
pacman -S <pkg>                     # Install
pacman -S --needed <pkg>            # Install only if not already installed
pacman -R <pkg>                     # Remove (keep deps)
pacman -Rs <pkg>                    # Remove + unused deps
pacman -Rns <pkg>                   # Remove + deps + config files
pacman -Rdd <pkg>                   # Force remove (ignore deps)

# Cache management
pacman -Sc                          # Remove old cached packages
pacman -Scc                         # Clear entire package cache
paccache -rk1                       # Keep 1 version, remove rest (pacman-contrib)
paccache -ruk0                      # Remove all uninstalled package caches
du -sh /var/cache/pacman/pkg/       # Cache size

# Database
pacman -Fy                          # Update file database
pacman -F <filename>                # Search which package provides a file
pacman -Dk                          # Check for broken packages
```

### AUR (yay / paru)

```bash
# yay (most common AUR helper)
yay -Syu                            # Full update (repos + AUR)
yay -S <pkg>                        # Install from AUR
yay -Ss <keyword>                   # Search repos + AUR
yay -Si <pkg>                       # Package info (repos + AUR)
yay -Rns <pkg>                      # Remove + deps + config
yay -Yc                             # Clean unneeded deps
yay -Ps                             # System stats (AUR packages, orphans)

# paru (alternative)
paru -Syu                           # Full update
paru -S <pkg>                       # Install
paru -Ss <keyword>                  # Search
paru -Rns <pkg>                     # Remove

# Manual AUR build (fallback)
git clone https://aur.archlinux.org/<pkg>.git
cd <pkg>
makepkg -si                         # Build and install (no sudo!)
```

### Mirror Management

```bash
# reflector — rank mirrors by speed
reflector --latest 20 --sort rate --protocol https --save /etc/pacman.d/mirrorlist
reflector --country US,CA --age 12 --sort rate --save /etc/pacman.d/mirrorlist
systemctl enable --now reflector.timer      # Auto-refresh weekly
systemctl status reflector.timer

# Manual mirrorlist check
cat /etc/pacman.d/mirrorlist
```

### Service Management

```bash
systemctl status <service>
systemctl start|stop|restart|reload <service>
systemctl enable|disable <service>
systemctl enable --now <service>    # Enable + start in one command
systemctl list-units --state=failed
systemctl list-unit-files --state=enabled
systemctl daemon-reload
journalctl -u <service> -n 50 --no-pager
journalctl -u <service> --since "10 min ago" --no-pager
journalctl -xe | tail -30          # Recent system-wide errors
journalctl --boot -p err --no-pager | tail -30
```

### System Diagnostics

```bash
# Resource usage
free -h
df -h
du -sh /* 2>/dev/null | sort -rh | head -15
top -b -n1 | head -20

# Process investigation
ps auxf --sort=-%mem | head -20
ps auxf --sort=-%cpu | head -20
lsof -p <pid>
strace -p <pid> -c -S time   # Syscall summary (30s sample)
iotop -b -n1 -o 2>/dev/null  # Disk I/O (requires iotop)

# Load and uptime
uptime
vmstat 1 5
dmesg --level=err,warn | tail -30
dmesg | grep -i "out of memory"
```

### Network

```bash
ip addr show
ip route show
ss -tlnp                    # Listening TCP
ss -ulnp                    # Listening UDP
ss -s                       # Socket summary
resolvectl status
resolvectl query <domain>
curl -sI https://example.com --max-time 5
mtr -r <host>               # Traceroute stats (requires mtr)

# systemd-networkd (common on Arch servers)
networkctl list
networkctl status <iface>
cat /etc/systemd/network/*.network

# NetworkManager (common on Arch desktops)
nmcli device status
nmcli connection show
nmcli connection show --active
```

### Firewall (firewalld / nftables)

```bash
# firewalld (recommended on Arch)
systemctl status firewalld
firewall-cmd --state
firewall-cmd --list-all
firewall-cmd --list-services
firewall-cmd --list-ports
firewall-cmd --add-service=http --permanent
firewall-cmd --add-port=8080/tcp --permanent
firewall-cmd --remove-port=8080/tcp --permanent
firewall-cmd --reload

# nftables (kernel-native)
nft list ruleset
nft list table inet filter
nft list ruleset > /etc/nftables.conf   # Save ruleset
systemctl enable --now nftables

# iptables (legacy, not recommended on Arch)
iptables -L -n -v
```

### User & Permissions

```bash
id <user>
getent passwd <user>
getent group <group>
useradd -m -s /bin/bash <user>
usermod -aG wheel,docker <user>     # wheel = sudo on Arch
passwd -l <user>                    # Lock account
passwd -S <user>                    # Status
last -n 20
lastlog | grep -v "Never"

# Sudo — Arch uses wheel group
cat /etc/sudoers
visudo -c                           # Validate sudoers
# Standard Arch sudoers: %wheel ALL=(ALL:ALL) ALL
```

### Storage

```bash
lsblk -o NAME,SIZE,TYPE,MOUNTPOINT,FSTYPE
blkid                               # UUIDs and filesystems
cat /etc/fstab
mount | column -t
findmnt                             # Mount tree
swapon --show
```

### Logs

```bash
journalctl --boot -p err --no-pager | tail -30
journalctl --since "1 hour ago" --no-pager | tail -50
journalctl --since today | tail -50
journalctl -u sshd --no-pager | tail -30
cat /var/log/pacman.log | tail -20
journalctl --disk-usage             # Journal size
journalctl --vacuum-size=500M       # Limit journal size
```

### Security Audit

```bash
# Firewall check
firewall-cmd --list-all 2>/dev/null || nft list ruleset 2>/dev/null || iptables -L -n

# Open ports
ss -tlnp

# Password and account audit
passwd -S -a 2>/dev/null
awk -F: '($2 == ""){print}' /etc/shadow   # Empty password accounts
awk -F: '($3 == 0){print}' /etc/passwd    # UID 0 accounts

# SUID/SGID binaries
find / -perm -4000 -o -perm -2000 -ls 2>/dev/null | head -20

# SSH hardening
cat /etc/ssh/sshd_config | grep -v '^#' | grep -v '^$'

# Failed logins
lastb -n 20 2>/dev/null
journalctl -u sshd | grep "Failed password" | tail -20

# Pacnews — config files with upstream changes (rolling release issue)
pacdiff -o                              # List files that need merging
find /etc -name "*.pacnew" -o -name "*.pacsave" -o -name "*.pacorig"
```

### Kernel & Hardware

```bash
uname -a
hostnamectl
cat /etc/os-release
lscpu
lspci -k | grep -A3 -i net
lsmod | grep <module>
modinfo <module>

# Kernel parameters
sysctl -a | grep <param>
sysctl -w <param>=<value>
sysctl -p                               # Reload /etc/sysctl.d/*.conf
cat /etc/sysctl.d/*.conf

# mkinitcpio (Arch initramfs)
mkinitcpio -P                           # Rebuild all initramfs images
ls /boot/initramfs-*.img
ls /boot/vmlinuz-*

# Kernel management
pacman -Q linux                         # Installed kernel version
ls /usr/lib/modules/                    # Installed kernel modules
# Keep LTS kernel as fallback: pacman -S linux-lts
```

### Boot Management (systemd-boot / GRUB)

```bash
# systemd-boot (common on UEFI Arch)
bootctl status
bootctl list
ls /boot/loader/entries/

# GRUB
grub-mkconfig -o /boot/grub/grub.cfg
cat /etc/default/grub
```

### SSH Hardening

```bash
# Key-based blocks in /etc/ssh/sshd_config:
# PermitRootLogin no
# PasswordAuthentication no
# PubkeyAuthentication yes
# MaxAuthTries 3
# AllowUsers <user>
ssh-keygen -t ed25519 -C "comment" -f ~/.ssh/id_ed25519
ssh-copy-id -i ~/.ssh/id_ed25519.pub user@host
chmod 700 ~/.ssh
chmod 600 ~/.ssh/authorized_keys
chmod 644 ~/.ssh/known_hosts
systemctl restart sshd
```

### Cron & Scheduling (systemd timers)

```bash
# Arch uses systemd timers; cron is optional
systemctl list-timers
systemctl list-timers --all
cat /etc/systemd/system/*.timer
cat ~/.config/systemd/user/*.timer

# If cron is installed (not default)
crontab -l
cat /etc/crontab
ls /etc/cron.*/
```

## Step 3: Interpret & Advise

When reporting results:
- Prioritize actionable issues (failed services, full disks, OOM kills, auth failures)
- Show raw output first, then summarize key findings
- For errors: check journalctl for the service, then suggest fixes
- For performance: correlate CPU/memory/IO with process list
- For security: flag open ports, empty passwords, and weak SSH config
- **For Arch-specific**: check for `.pacnew` files after updates, partial upgrades (`pacman -Qk`), and AUR package breakage after system updates

## Common Workflows

### "System update"

```bash
# Check for pacnews first
find /etc -name "*.pacnew" -o -name "*.pacsave" | head -10
# Update mirrors if slow
reflector --latest 10 --sort rate --protocol https --save /etc/pacman.d/mirrorlist
# Full update
pacman -Syu
# AUR update
yay -Syu
# Post-update: check for orphans and broken packages
pacman -Qdt
pacman -Dk
```

### "Server is slow"

```bash
free -h && uptime && df -h && top -b -n1 | head -20
```

Then: check for OOM kills (`dmesg | grep -i oom`), high I/O (`iotop -b -n1 -o 2>/dev/null`), or swap thrashing (`vmstat 1 5`).

### "Service won't start"

```bash
systemctl status <service> --no-pager -l
journalctl -u <service> -n 100 --no-pager
```

### "Disk is full"

```bash
df -h && du -sh /* 2>/dev/null | sort -rh | head -15
# Check pacman cache
du -sh /var/cache/pacman/pkg/
```

Then drill into the largest directory. On Arch, package cache is the most common culprit — run `paccache -rk1` or `pacman -Sc`.

### "AUR package broke after update"

```bash
yay -S <pkg> --rebuild     # Rebuild against current libraries
# Or clean build
yay -S <pkg> --cleanbuild
# Manual approach
cd /tmp && git clone https://aur.archlinux.org/<pkg>.git && cd <pkg>
makepkg -si
```

### "Unauthorized access suspected"

```bash
last -n 30 && lastb -n 20 2>/dev/null && journalctl -u sshd | grep "Failed" | tail -50
```

Check for: unknown users, logins at odd hours, repeated failures from same IP.

### "Pacman database is corrupted"

```bash
pacman -Dk                    # Check integrity
sudo pacman -Syy              # Force refresh all package databases
# If package files are missing
pacman -Qk <pkg>              # Check package file integrity
```

## Guidelines

- Run `sudo` only when needed; check `whoami` first
- **Never run makepkg as root** — it will refuse and corrupt your build environment
- Always do a full system update (`pacman -Syu`) before installing new packages on a rolling release
- Prefer `pacman -Rs` over `-R` to clean up unused dependencies
- Use `paccache` from `pacman-contrib` for cache management; don't blindly `pacman -Scc`
- Arch uses `wheel` group for sudo, not `sudo` group like Ubuntu
- Arch uses systemd-networkd or NetworkManager, not netplan
- Arch uses firewalld or nftables, not ufw (though ufw is available in repos)
- After large updates, check for `.pacnew` files with `pacdiff` or `find /etc -name "*.pacnew"`
- Partial upgrades are not supported — always use `pacman -Syu` not `pacman -Sy && pacman -S <pkg>`
- Back up config files before modifying: `cp file file.bak.$(date +%s)`
- The AUR is user-submitted content; always review PKGBUILDs before installing (`yay -Si <pkg>` shows votes and popularity)
- For boot issues: keep `linux-lts` kernel installed as a fallback
- Use `reflector` to keep mirrors fast; stale mirrors cause download failures
