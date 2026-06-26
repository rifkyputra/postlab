---
name: fedora-sysadmin
description: Fedora Linux system administration. Use when the user asks to manage services, install packages, configure networking, inspect logs, manage users, tune kernel parameters, secure a server, or diagnose system issues on Fedora. Covers dnf, systemd, firewalld, NetworkManager, SELinux, journalctl, SSH hardening, performance tuning, and common diagnostic workflows.
---

# Fedora System Administration

## Execution Model

Pi executes tool calls sequentially, even when you emit multiple calls in one turn. Batch independent calls in a single turn to save round-trips:

| Pattern | Use for |
|---------|---------|
| Multiple bash calls in one turn | Independent diagnostics (df & free & uptime) |
| `read /path & read /other` via bash | Read multiple files concurrently |

## Step 1: Classify the Request

| Type | Trigger | Primary Approach |
|------|---------|-----------------|
| **Package Ops** | Install, remove, update, search packages | dnf/rpm commands |
| **Service Ops** | Start, stop, restart, enable, check status | systemctl + journalctl |
| **Diagnostic** | "Why is X slow?", "What's using Y?", "Check Z" | top/free/df/ss + targeted log inspection |
| **Network** | "Configure IP", "Fix DNS", "Check port" | nmcli/ip/ss/resolvectl |
| **Security** | "Harden", "Audit", "Check firewall", "SSH setup" | firewalld/selinux/SSH config/audit |
| **User/Access** | "Add user", "Groups", "Permissions" | useradd/usermod/chown/chmod |
| **Storage** | "Add disk", "Mount", "Check space", "LVM" | lsblk/df/mount/fdisk/LVM tools |
| **Kernel/Tuning** | "Tune sysctl", "Check modules", "Boot issues" | sysctl/modprobe/dmesg/grub |

## Step 2: Run Commands

Always use `sudo` for privileged operations. Check the current user first with `whoami`.

### Package Management

```bash
dnf check-update
dnf search <keyword>
dnf info <pkg>
dnf install -y <pkg>
dnf remove <pkg>
dnf autoremove
dnf history list
dnf history undo <id>
dnf repolist
dnf config-manager --add-repo <url>        # Add a repo
dnf config-manager --set-enabled <repo>
dnf module list                              # Modular streams (Fedora)
dnf module install <module>:<stream>
rpm -qa | grep <pkg>                         # Installed packages
rpm -ql <pkg>                                # Files owned by package
rpm -qi <pkg>                                # Package info
rpm -V <pkg>                                 # Verify package integrity

# COPR (community repos)
dnf copr enable <user>/<project>
dnf copr disable <user>/<project>

# Flatpak
flatpak search <app>
flatpak install flathub <app>
flatpak list
flatpak update
```

### Service Management

```bash
systemctl status <service>
systemctl start|stop|restart|reload <service>
systemctl enable|disable <service>
systemctl list-units --state=failed
systemctl list-unit-files --state=enabled
systemctl daemon-reload
journalctl -u <service> -n 50 --no-pager
journalctl -u <service> --since "10 min ago" --no-pager
journalctl -xe | tail -30          # Recent system-wide errors
journalctl --boot -p err --no-pager
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
iotop -b -n1 -o 2>/dev/null  # Disk I/O

# Load and uptime
uptime
vmstat 1 5
dmesg | tail -30
```

### Network

```bash
# NetworkManager
nmcli device status
nmcli connection show
nmcli connection show <name>
nmcli connection add con-name <name> ifname <iface> type ethernet ip4 <ip>/<prefix> gw4 <gw>
nmcli connection modify <name> ipv4.dns "<dns1> <dns2>"
nmcli connection up|down <name>
nmcli connection delete <name>
nmcli radio wifi on|off

# Low-level
ip addr show
ip route show
ss -tlnp                    # Listening TCP
ss -ulnp                    # Listening UDP
ss -s                       # Socket summary
resolvectl status
resolvectl query <domain>
curl -sI https://example.com --max-time 5
```

### Firewall (firewalld)

```bash
firewall-cmd --state
firewall-cmd --list-all
firewall-cmd --list-all-zones
firewall-cmd --get-default-zone
firewall-cmd --get-active-zones
firewall-cmd --add-port=<port>/<proto> --permanent
firewall-cmd --add-service=<service> --permanent
firewall-cmd --add-rich-rule='rule family="ipv4" source address="<ip>" port port="<port>" protocol="<proto>" accept' --permanent
firewall-cmd --remove-port=<port>/<proto> --permanent
firewall-cmd --reload
firewall-cmd --runtime-to-permanent
```

### SELinux

```bash
getenforce                                  # Enforcing / Permissive / Disabled
sestatus                                    # Full SELinux status
setenforce 0|1                              # Temporarily toggle
# Permanent: edit /etc/selinux/config

# Troubleshooting
ausearch -m avc -ts recent | tail -20       # Recent denials
sealert -a /var/log/audit/audit.log         # Human-readable denials
audit2allow -a -M <module>                  # Generate policy module
semodule -i <module>.pp                     # Install policy module
restorecon -Rv <path>                       # Restore default context
ls -Z <path>                                # Show SELinux context
chcon -t <type> <path>                      # Change context
semanage fcontext -a -t <type> '<path>(/.*)?'  # Persistent context rule
semanage port -l | grep <port>              # List port contexts
semanage port -a -t <type> -p <proto> <port>  # Add port context

# Booleans
getsebool -a | grep <keyword>
setsebool -P <boolean> on|off
```

### User & Permissions

```bash
id <user>
getent passwd <user>
getent group <group>
useradd -m -s /bin/bash <user>
usermod -aG wheel,docker <user>         # Fedora uses 'wheel' not 'sudo'
passwd -l <user>                         # Lock account
passwd -S <user>                         # Status
last -n 20                               # Recent logins
lastlog | grep -v "Never"
cat /etc/sudoers.d/*
visudo -c                                # Validate sudoers
```

### Storage

```bash
lsblk -o NAME,SIZE,TYPE,MOUNTPOINT,FSTYPE
blkid                                    # UUIDs and filesystems
cat /etc/fstab
mount | column -t
findmnt                                  # Mount tree
swapon --show
btrfs filesystem show                    # Btrfs (Fedora default)
btrfs filesystem df /
btrfs subvolume list /
```

### Logs

```bash
journalctl --boot -p err --no-pager | tail -30
journalctl --since "1 hour ago" --no-pager | tail -50
tail -50 /var/log/messages
tail -50 /var/log/secure                 # Auth log (Fedora/RHEL)
tail -50 /var/log/audit/audit.log        # SELinux audit
tail -50 /var/log/dnf.log
tail -50 /var/log/boot.log
```

### Security Audit

```bash
firewall-cmd --state && firewall-cmd --list-all
ss -tlnp                                 # Open ports
getenforce                               # SELinux status
# Empty password accounts
awk -F: '($2 == "" && $1 != "root"){print $1}' /etc/shadow 2>/dev/null
# UID 0 accounts
awk -F: '($3 == 0){print}' /etc/passwd
# SUID/SGID binaries
find / -perm -4000 -o -perm -2000 -ls 2>/dev/null | head -20
# SSH config
grep -v '^#' /etc/ssh/sshd_config | grep -v '^$'
lastb -n 20 2>/dev/null                  # Failed login attempts
ausearch -m avc -ts today | tail -20     # Today's SELinux denials
```

### Kernel & Hardware

```bash
uname -a
hostnamectl
cat /etc/os-release
cat /etc/fedora-release
lscpu
lspci -k | grep -A3 -i net
lsmod | grep <module>
modinfo <module>
sysctl -a | grep <param>
sysctl -w <param>=<value>
sysctl -p                                # Reload /etc/sysctl.d/*.conf
dmesg -T | tail -30
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
```

### Cron & Scheduling

```bash
crontab -l                               # User's crontab
crontab -e
cat /etc/crontab
ls /etc/cron.*/
systemctl list-timers
systemctl list-timers --all
```

### DNF System Upgrades (fedora-release)

```bash
dnf upgrade --refresh                   # Regular package upgrade
dnf system-upgrade download --releasever=<N>  # Prepare version upgrade
dnf system-upgrade reboot                # Execute version upgrade
dnf system-upgrade clean                 # Clean up after upgrade
```

### rpm-ostree (Fedora Atomic / CoreOS)

```bash
rpm-ostree status
rpm-ostree install <pkg>
rpm-ostree uninstall <pkg>
rpm-ostree upgrade
rpm-ostree rebase <ref>
rpm-ostree rollback
```

## Step 3: Interpret & Advise

When reporting results:
- Prioritize actionable issues (failed services, full disks, OOM kills, auth failures, SELinux denials)
- Show raw output first, then summarize key findings
- For errors: check journalctl for the service, then suggest fixes
- For performance: correlate CPU/memory/IO with process list
- For security: flag open ports, empty passwords, weak SSH config, and SELinux issues
- **SELinux**: don't suggest disabling it to fix problems; check audit log and create proper policies instead

## Common Workflows

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

If SELinux denial suspected: `ausearch -m avc -ts recent | grep <service>`

### "Permission denied / SELinux blocking"

```bash
ausearch -m avc -ts recent | tail -20
```

Check the denial and create a proper policy or restore context; don't disable SELinux.

### "Disk is full"

```bash
df -h && du -sh /* 2>/dev/null | sort -rh | head -15
```

Then drill into the largest directory. For btrfs: `btrfs filesystem df /`.

### "Unauthorized access suspected"

```bash
last -n 30 && lastb -n 20 2>/dev/null && tail -100 /var/log/secure
```

Check for: unknown users, logins at odd hours, repeated failures from same IP.

## Guidelines

- Run `sudo` only when needed; check `whoami` first
- Prefer `dnf` over `rpm` for package operations; use `rpm` only for queries
- Always prefer `systemctl` over direct init scripts
- Use `ss` instead of `netstat`; it's faster and always available
- Use `nmcli` for NetworkManager configuration
- Use `firewall-cmd` for firewalld; remember `--permanent` + `--reload` for persistent rules
- **Never suggest disabling SELinux**. Read `ausearch`/`sealert` output and suggest proper policy fixes
- For destructive operations (rm, fdisk, dd): confirm with user first
- Check `journalctl -xe` before suggesting complex fixes; the answer is often in logs
- Back up config files before modifying: `cp file file.bak.$(date +%s)`
