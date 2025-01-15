<script lang="ts">
	import type { Snippet } from 'svelte';

	type Props = {
		title: string;
		children: Snippet;
	};

	const { title, children }: Props = $props();
</script>

<div class="container">
	<p class="text-12 text-regular title">{title}</p>
	<div>
		{@render children()}
	</div>
</div>

<style lang="postcss">
	.container {
		display: flex;
		flex-direction: column;
		gap: 8px;
	}

	.title {
		color: var(--clr-text-2);
	}
</style>
