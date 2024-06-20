<script lang="ts">
	import type { CommitNode } from '$lib/commitLines/types';

	interface Props {
		commitNode: CommitNode;
	}

	// eslint-disable-next-line @typescript-eslint/no-unused-vars
	const { commitNode }: Props = $props();
</script>

<div class="node"></div>

<style lang="postcss">
	.node {
		height: 10px;
		width: 10px;

		margin-top: -5px;
		margin-bottom: -5px;
		margin-left: -6px;
		margin-right: -4px;

		z-index: var(--z-lifted);

		background-color: var(--clr-commit-shadow);
		border-radius: 8px;
	}
</style>
