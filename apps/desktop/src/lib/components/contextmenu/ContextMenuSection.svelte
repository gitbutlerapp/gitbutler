<script lang="ts">
	import { type Snippet } from 'svelte';

	interface Props {
		children?: Snippet;
	}

	let { children }: Props = $props();
</script>

<div class="context-menu-section">
	{@render children?.()}
</div>

<style lang="postcss">
	.context-menu-section {
		position: relative;
		display: flex;
		flex-direction: column;
		padding: 4px;
		gap: 2px;

		&:not(:first-child) {
			border-top: 1px solid var(--clr-border-2);
		}
	}
</style>
