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
		width: 100%;
		display: flex;
		border-radius: var(--radius-m);
	}
</style>
