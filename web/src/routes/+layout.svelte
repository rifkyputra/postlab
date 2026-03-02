<script lang="ts">
	import favicon from '$lib/assets/favicon.svg';
	import { QueryClient, QueryClientProvider } from '@tanstack/svelte-query';

	const queryClient = new QueryClient({
		defaultOptions: { queries: { staleTime: 30_000 } },
	});

	let { children } = $props();
</script>

<svelte:head>
	<link rel="icon" href={favicon} />
</svelte:head>

<QueryClientProvider client={queryClient}>
	<nav>
		<a href="/">Dashboard</a>
		<a href="/servers">Servers</a>
		<a href="/tasks">Tasks</a>
	</nav>
	<main>
		{@render children()}
	</main>
</QueryClientProvider>

<style>
	nav {
		display: flex;
		gap: 1rem;
		padding: 1rem;
		background: #1a1a1a;
	}
	nav a {
		color: #aaa;
		text-decoration: none;
	}
	nav a:hover {
		color: #fff;
	}
	main {
		padding: 1.5rem;
	}
</style>
