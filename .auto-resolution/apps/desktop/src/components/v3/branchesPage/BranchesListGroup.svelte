<script lang="ts">
	import type { Snippet } from 'svelte';

	type Props = {
		title: string;
		children: Snippet;
	};

	const { title, children }: Props = $props();
</script>

<div class="branches-list-wrap">
	<div class="branches-list-wrap__header"><span class="text-12 text-semibold">{title}</span></div>
	<div class="branches-list-wrap__content">
		{@render children()}
	</div>
</div>

<style class="postcss">
	.branches-list-wrap {
		display: flex;
		flex-direction: column;
		overflow: hidden;
	}

	.branches-list-wrap__header {
		display: flex;
		background-color: var(--clr-bg-2);
		border-bottom: 1px solid var(--clr-border-2);
		padding: 12px 12px 8px 12px;

		& span {
			color: var(--clr-text-2);
		}
	}

	.branches-list-wrap__content {
		display: flex;
		flex-direction: column;
	}
</style>
