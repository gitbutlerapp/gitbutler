<script lang="ts">
	import { type Snippet } from 'svelte';

	interface Props {
		children: Snippet;
	}

	const { children }: Props = $props();
</script>

<ul class="tab-list">
	{@render children()}
</ul>

<style>
	.tab-list {
		display: flex;
		gap: 0.5rem;
	}
</style>
