<script lang="ts">
	import type { Snippet } from 'svelte';

	interface Props {
		children: Snippet;
	}

	const { children }: Props = $props();
</script>

<div class="modal__footer">
	{@render children()}
</div>

<style lang="postcss">
	.modal__footer {
		display: flex;
		justify-content: flex-end;
		width: 100%;
		padding: 16px;
		gap: 8px;
		border-top: 1px solid var(--clr-border-2);
		border-radius: 0 0 var(--radius-l) var(--radius-l);
		background-color: var(--clr-bg-1);
	}
</style>
