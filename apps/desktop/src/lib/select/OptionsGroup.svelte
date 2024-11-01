<script lang="ts">
	import { type Snippet } from 'svelte';

	interface Props {
		children: Snippet;
	}

	const { children }: Props = $props();
</script>

<div class="options__group">
	{@render children()}
</div>

<style lang="postcss">
	.options__group {
		display: flex;
		flex-direction: column;
		padding: 6px;
		gap: 2px;

		&:not(:global(&:last-child)) {
			border-bottom: 1px solid var(--clr-border-2);
		}
	}
</style>
