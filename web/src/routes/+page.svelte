<script lang="ts">
	import { createServersQuery, createTasksQuery } from '$lib/api/queries';
	import ServerCard from '$lib/components/ServerCard.svelte';
	import TaskRow from '$lib/components/TaskRow.svelte';

	const servers = createServersQuery();
	const tasks = createTasksQuery();
</script>

<h1>Dashboard</h1>

<section>
	<h2>Servers</h2>
	{#if $servers.isPending}
		<p>Loading servers…</p>
	{:else if $servers.isError}
		<p class="error">Error: {$servers.error.message}</p>
	{:else}
		<div class="grid">
			{#each $servers.data.servers as server}
				<ServerCard {server} />
			{/each}
			{#if $servers.data.servers.length === 0}
				<p class="dim">No servers yet. <a href="/servers">Add one.</a></p>
			{/if}
		</div>
	{/if}
</section>

<section>
	<h2>Recent Tasks</h2>
	{#if $tasks.isPending}
		<p>Loading tasks…</p>
	{:else if $tasks.isError}
		<p class="error">Error: {$tasks.error.message}</p>
	{:else}
		{#each $tasks.data.tasks.slice(0, 10) as task}
			<TaskRow {task} />
		{/each}
		{#if $tasks.data.tasks.length === 0}
			<p class="dim">No tasks yet.</p>
		{/if}
	{/if}
</section>

<style>
	.grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(260px, 1fr)); gap: 1rem; }
	.error { color: #f66; }
	.dim { color: #666; }
	h2 { margin-top: 1.5rem; }
</style>
