<script lang="ts">
	import { page } from '$app/state';
	import { createServerQuery, createTasksQuery } from '$lib/api/queries';
	import TaskRow from '$lib/components/TaskRow.svelte';

	const id = page.params.id;
	const server = createServerQuery(id);
	const tasks = createTasksQuery(id);
</script>

{#if $server.isPending}
	<p>Loading…</p>
{:else if $server.isError}
	<p class="error">{$server.error.message}</p>
{:else}
	<h1>{$server.data.name}</h1>
	<dl>
		<dt>Host</dt><dd>{$server.data.user}@{$server.data.host}:{$server.data.port}</dd>
		<dt>Auth</dt><dd>{$server.data.auth_method}</dd>
		<dt>OS</dt><dd>{$server.data.os_family ?? 'Unknown (auto-detect on connect)'}</dd>
	</dl>

	<h2>Actions</h2>
	<div class="actions">
		<a href="/servers/{id}/install">Install app</a>
		<button>Upgrade OS</button>
		<button>Harden security</button>
	</div>

	<h2>Tasks</h2>
	{#if $tasks.isPending}
		<p>Loading tasks…</p>
	{:else}
		{#each $tasks.data?.tasks ?? [] as task}
			<TaskRow {task} />
		{/each}
	{/if}
{/if}

<style>
	dl { display: grid; grid-template-columns: max-content 1fr; gap: 0.25rem 1rem; }
	dt { color: #888; }
	.actions { display: flex; gap: 0.5rem; margin: 0.5rem 0 1rem; }
	.error { color: #f66; }
</style>
