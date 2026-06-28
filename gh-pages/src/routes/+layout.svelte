<script>
	let { children } = $props();
	import '../app.css';
</script>

<main class="container">
	{@render children?.()}
</main>

<footer>
	<p><a href="https://github.com/rifkyputra/postlab">GitHub</a></p>
</footer>
