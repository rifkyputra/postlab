<script lang="ts">
	import type { Task } from '$lib/api/queries';

	interface Props { task: Task; }
	let { task }: Props = $props();

	const statusColor: Record<string, string> = {
		pending: '#888',
		running: '#4af',
		success: '#4c4',
		failed: '#f44',
	};
</script>

<a href="/tasks/{task.id}" class="row">
	<span class="dot" style="background:{statusColor[task.status] ?? '#888'}"></span>
	<span class="kind">{task.kind}</span>
	<span class="status">{task.status}</span>
	<span class="time">{new Date(task.created_at).toLocaleString()}</span>
</a>

<style>
	.row {
		display: flex;
		align-items: center;
		gap: 0.75rem;
		padding: 0.5rem 0;
		border-bottom: 1px solid #222;
		text-decoration: none;
		color: inherit;
	}
	.row:hover { background: #111; }
	.dot { width: 8px; height: 8px; border-radius: 50%; flex-shrink: 0; }
	.kind { flex: 1; font-family: monospace; }
	.status { color: #888; font-size: 0.85rem; }
	.time { color: #555; font-size: 0.8rem; }
</style>
