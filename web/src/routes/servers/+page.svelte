<script lang="ts">
	import { createServersQuery } from '$lib/api/queries';
	import { createDeleteServerMutation } from '$lib/api/mutations';

	const servers = createServersQuery();
	const deleteServer = createDeleteServerMutation();

	function handleDelete(id: string, name: string) {
		if (confirm(`Remove server "${name}"?`)) {
			$deleteServer.mutate(id);
		}
	}
</script>

<h1>Servers</h1>
<a href="/servers/new">+ Add server</a>

{#if $servers.isPending}
	<p>Loading…</p>
{:else if $servers.isError}
	<p class="error">{$servers.error.message}</p>
{:else}
	<table>
		<thead>
			<tr><th>Name</th><th>Host</th><th>OS</th><th>Auth</th><th></th></tr>
		</thead>
		<tbody>
			{#each $servers.data.servers as s}
				<tr>
					<td><a href="/servers/{s.id}">{s.name}</a></td>
					<td>{s.user}@{s.host}:{s.port}</td>
					<td>{s.os_family ?? '—'}</td>
					<td>{s.auth_method}</td>
					<td>
						<button onclick={() => handleDelete(s.id, s.name)}>Remove</button>
					</td>
				</tr>
			{/each}
		</tbody>
	</table>
{/if}

<style>
	table { width: 100%; border-collapse: collapse; margin-top: 1rem; }
	th, td { padding: 0.5rem; border-bottom: 1px solid #222; text-align: left; }
	.error { color: #f66; }
</style>
