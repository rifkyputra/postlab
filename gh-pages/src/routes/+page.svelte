<script>
	const screens = [
		{ key: '1', name: 'Dashboard', desc: 'Live CPU, memory, disk gauges. Process table. Per-core sparklines.' },
		{ key: '2', name: 'Packages', desc: 'Install, remove, search. Batch queue. Security-update flags.' },
		{ key: '3', name: 'Security', desc: 'Auto-scan findings with one-click fixes. Firewall, SSH, Fail2Ban.' },
		{ key: '4', name: 'Networking', desc: 'Caddy reverse proxy. Cloudflare Tunnels. Tailscale VPN.' },
		{ key: '5', name: 'Docker', desc: 'Containers, images, compose. Managed one-click dev services.' },
		{ key: '6', name: 'wasmCloud', desc: 'Hosts, components, apps. Lattice inspector.' },
		{ key: '7', name: 'Agent', desc: 'Interactive pi agent chat. Scheduled tasks. Skill library.' },
		{ key: '8', name: 'System', desc: 'Services, users, swap, storage, ghost process hunter.' },
		{ key: '9', name: 'Projects', desc: 'Clone repos, scaffold apps. Git-backed project browser.' }
	];
</script>

<svelte:head>
	<title>postlab — bare-metal server manager</title>
	<meta name="description" content="Interactive TUI server manager. Single binary. Runs anywhere." />
</svelte:head>

<section class="hero">
	<h1>postlab</h1>
	<p class="tagline">Interactive bare-metal server manager</p>
	<p class="sub">Single binary. Cross-platform. Runs as root. <code>~8 MB</code>.</p>
	<div class="hero-buttons">
		<a href="#install" class="btn">Install</a>
		<a href="https://github.com/rifkyputra/postlab" class="btn outline">GitHub</a>
	</div>
</section>

<section id="install">
	<h2>Install</h2>
	<pre><code>curl -fsSL https://raw.githubusercontent.com/rifkyputra/postlab/main/install.sh | bash</code></pre>
	<p>Or download a <a href="https://github.com/rifkyputra/postlab/releases">release binary</a>.</p>
	<p>Launch: <code>sudo postlab</code></p>
</section>

<section id="cli">
	<h2>CLI</h2>
	<p>Run one-shot commands without the TUI:</p>
	<pre><code>sudo postlab info                  # system summary
sudo postlab packages search nginx
sudo postlab processes list --sort mem
sudo postlab services list
sudo postlab containers list</code></pre>
</section>

<section id="screens">
	<h2>Screens</h2>
	<p>Press <kbd>1</kbd>–<kbd>9</kbd> to switch. <kbd>?</kbd> for help.</p>
	<div class="grid">
		{#each screens as s}
			<div class="card">
				<h3><span class="key">{s.key}</span> {s.name}</h3>
				<p>{s.desc}</p>
			</div>
		{/each}
	</div>
</section>

<section id="features">
	<h2>Highlights</h2>
	<div class="grid">
		<div class="card">
			<h3>&#x1f6e1;&#xfe0f; Security first</h3>
			<p>Auto-scan on first visit. One-click fixes. Timestamped <code>.bak</code> backups before every change.</p>
		</div>
		<div class="card">
			<h3>&#x26a1; Non-blocking</h3>
			<p>Every operation runs in the background. The TUI stays responsive while installing packages or pulling images.</p>
		</div>
		<div class="card">
			<h3>&#x1f310; Cross-platform</h3>
			<p>Linux (apt, dnf, pacman) and macOS (brew). Auto-detects package manager, firewall, init system.</p>
		</div>
		<div class="card">
			<h3>&#x1f9e0; AI agent built in</h3>
			<p>Chat with pi agent from the TUI. Schedule background tasks. Install skills from the library.</p>
		</div>
		<div class="card">
			<h3>&#x1f4e6; One-click services</h3>
			<p>PostgreSQL, Redis, MySQL, MongoDB, MinIO — deploy with a single keypress.</p>
		</div>
		<div class="card">
			<h3>&#x1f3d7;&#xfe0f; Project scaffolding</h3>
			<p>Clone repos by <code>user/repo</code> shorthand. Scaffold full-stack apps with create-better-t-stack.</p>
		</div>
	</div>
</section>

<section id="cli-table">
	<h2>All CLI commands</h2>
	<table>
		<thead>
			<tr><th>Command</th><th>Description</th></tr>
		</thead>
		<tbody>
			<tr><td><code>sudo postlab</code></td><td>Launch TUI</td></tr>
			<tr><td><code>postlab info</code></td><td>OS, kernel, CPU, memory, uptime</td></tr>
			<tr><td><code>postlab list</code></td><td>Installed packages</td></tr>
			<tr><td><code>postlab system info|cpu|mem|disks|net|swap</code></td><td>System diagnostics</td></tr>
			<tr><td><code>postlab packages search|install|remove|upgrade</code></td><td>Package management</td></tr>
			<tr><td><code>postlab processes list|kill</code></td><td>Process management</td></tr>
			<tr><td><code>postlab services list|start|stop|restart</code></td><td>Service management</td></tr>
			<tr><td><code>postlab storage list</code></td><td>Filesystems + disk health</td></tr>
			<tr><td><code>postlab completions bash|zsh|fish</code></td><td>Shell completions</td></tr>
		</tbody>
	</table>
</section>
