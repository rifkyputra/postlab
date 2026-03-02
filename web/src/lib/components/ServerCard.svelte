<script lang="ts">
	import type { Server } from '$lib/api/queries';

	interface Props { server: Server; }
	let { server }: Props = $props();
</script>

<a href="/servers/{server.id}" class="card">
	<h3>{server.name}</h3>
	<p class="host">{server.user}@{server.host}:{server.port}</p>
	{#if server.os_family}
		<span class="badge">{server.os_family}</span>
	{/if}
</a>

<style>
	.card {
		display: block;
		padding: 1rem;
		border: 1px solid #333;
		border-radius: 6px;
		text-decoration: none;
		color: inherit;
		transition: border-color 0.15s;
	}
	.card:hover { border-color: #777; }
	h3 { margin: 0 0 0.25rem; }
	.host { color: #888; font-size: 0.85rem; margin: 0; }
	.badge {
		display: inline-block;
		margin-top: 0.5rem;
		padding: 0.1rem 0.4rem;
		background: #222;
		border-radius: 4px;
		font-size: 0.75rem;
		color: #aaa;
	}
</style>
