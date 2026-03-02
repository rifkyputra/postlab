<script lang="ts">
	import { createTasksQuery } from '$lib/api/queries';
	import TaskRow from '$lib/components/TaskRow.svelte';

	const tasks = createTasksQuery();

	const statuses = ['all', 'pending', 'running', 'success', 'failed'];
	let filter = $state('all');

	const filtered = $derived(
		filter === 'all'
			? ($tasks.data?.tasks ?? [])
			: ($tasks.data?.tasks ?? []).filter((t) => t.status === filter),
	);
</script>

<h1>Tasks</h1>

<div class="filters">
	{#each statuses as s}
		<button class:active={filter === s} onclick={() => (filter = s)}>{s}</button>
	{/each}
</div>

{#if $tasks.isPending}
	<p>Loading…</p>
{:else if $tasks.isError}
	<p class="error">{$tasks.error.message}</p>
{:else}
	{#each filtered as task}
		<TaskRow {task} />
	{/each}
	{#if filtered.length === 0}
		<p class="dim">No tasks match the filter.</p>
	{/if}
{/if}

<style>
	.filters { display: flex; gap: 0.5rem; margin-bottom: 1rem; }
	button.active { background: #444; }
	.error { color: #f66; }
	.dim { color: #666; }
</style>
