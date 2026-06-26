---
name: ubuntu-sysadmin
description: Ubuntu Linux system administration. Use when the user asks to manage services, install packages, configure networking, inspect logs, manage users, tune kernel parameters, secure a server, or diagnose system issues on Ubuntu. Covers systemd, apt, ufw, netplan, journalctl, SSH hardening, performance tuning, and common diagnostic workflows.
---

# Ubuntu System Administration

## Execution Model

Pi executes tool calls sequentially, even when you emit multiple calls in one turn. Batch independent calls in a single turn to save round-trips:

| Pattern | Use for |
|---------|---------|
| Multiple bash calls in one turn | Independent diagnostics (df & free & uptime) |
| `read /path & read /other` via bash | Read multiple files concurrently |

## Step 1: Classify the Request

| Type | Trigger | Primary Approach |
|------|---------|-----------------|
| **Package Ops** | Install, remove, update, search packages | apt/dpkg/snap commands |
| **Service Ops** | Start, stop, restart, enable, check status | systemctl + journalctl |
| **Diagnostic** | "Why is X slow?", "What's using Y?", "Check Z" | top/free/df/ss + targeted log inspection |
| **Network** | "Configure IP", "Fix DNS", "Check port" | ip/ss/netplan/resolvectl |
| **Security** | "Harden", "Audit", "Check firewall", "SSH setup" | ufw/fail2ban/SSH config/auditd |
| **User/Access** | "Add user", "Groups", "Permissions" | useradd/usermod/chown/chmod |
| **Storage** | "Add disk", "Mount", "Check space", "LVM" | lsblk/df/mount/fdisk/LVM tools |
| **Kernel/Tuning** | "Tune sysctl", "Check modules", "Boot issues" | sysctl/modprobe/dmesg/grub |

## Step 2: Run Commands

Always use `sudo` for privileged operations. Check the current user first with `whoami`.

### Package Management

```bash
apt update && apt list --upgradable
apt search <keyword>
apt show <pkg>
apt install -y <pkg>
apt remove --purge <pkg>
apt autoremove -y
apt-mark hold <pkg>        # Pin version
apt-mark showhold
dpkg -l | grep <pkg>       # Installed packages
dpkg -L <pkg>               # Files owned by package
snap list
snap install <pkg>
apt-cache policy <pkg>      # Available versions
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
dmesg | tail -30
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
cat /etc/netplan/*.yaml     # Netplan config
netplan apply
```

### Firewall (UFW)

```bash
ufw status verbose
ufw allow|deny <port>/<proto>
ufw allow from <ip> to any port <port>
ufw delete <rule-number>
ufw enable|disable
ufw reload
ufw show added
```

### User & Permissions

```bash
id <user>
getent passwd <user>
getent group <group>
useradd -m -s /bin/bash <user>
usermod -aG sudo,docker <user>
passwd -l <user>            # Lock account
passwd -S <user>            # Status
last -n 20                  # Recent logins
lastlog | grep -v "Never"
cat /etc/sudoers.d/*
visudo -c                   # Validate sudoers
```

### Storage

```bash
lsblk -o NAME,SIZE,TYPE,MOUNTPOINT,FSTYPE
blkid                       # UUIDs and filesystems
cat /etc/fstab
mount | column -t
findmnt                     # Mount tree
swapon --show
```

### Logs

```bash
journalctl --boot -p err --no-pager | tail -30
journalctl --since "1 hour ago" --no-pager | tail -50
tail -50 /var/log/syslog
tail -50 /var/log/auth.log
tail -50 /var/log/kern.log
cat /var/log/dpkg.log | tail -20
```

### Security Audit

```bash
ufw status
ss -tlnp                    # Open ports
passwd -S -a                # Password status for all accounts (requires passwd -S -a support)
awk -F: '($2 == ""){print}' /etc/shadow   # Empty password accounts
awk -F: '($3 == 0){print}' /etc/passwd    # UID 0 accounts
find / -perm -4000 -o -perm -2000 -ls 2>/dev/null | head -20  # SUID/SGID
cat /etc/ssh/sshd_config | grep -v '^#' | grep -v '^$'
lastb -n 20 2>/dev/null     # Failed login attempts
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
sysctl -a | grep <param>
sysctl -w <param>=<value>
sysctl -p                   # Reload /etc/sysctl.conf
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
crontab -l                  # User's crontab
crontab -e
cat /etc/crontab
ls /etc/cron.*/
systemctl list-timers
systemctl list-timers --all
```

## Step 3: Interpret & Advise

When reporting results:
- Prioritize actionable issues (failed services, full disks, OOM kills, auth failures)
- Show raw output first, then summarize key findings
- For errors: check journalctl for the service, then suggest fixes
- For performance: correlate CPU/memory/IO with process list
- For security: flag open ports, empty passwords, and weak SSH config

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

### "Disk is full"

```bash
df -h && du -sh /* 2>/dev/null | sort -rh | head -15
```

Then drill into the largest directory.

### "Unauthorized access suspected"

```bash
last -n 30 && lastb -n 20 2>/dev/null && tail -100 /var/log/auth.log
```

Check for: unknown users, logins at odd hours, repeated failures from same IP.

## Guidelines

- Run `sudo` only when needed; check `whoami` first
- Prefer `apt` over `dpkg` for package operations; use `dpkg` only for queries
- Always prefer `systemctl` over direct init scripts
- Use `ss` instead of `netstat`; it's faster and always available
- For netplan changes: read the file, explain the edit, then apply with `netplan apply`
- For destructive operations (rm, fdisk, dd): confirm with user first
- Check `journalctl -xe` before suggesting complex fixes; the answer is often in logs
- Back up config files before modifying: `cp file file.bak.$(date +%s)`
