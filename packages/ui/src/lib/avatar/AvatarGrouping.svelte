<script lang="ts">
	import type { Snippet } from 'svelte';

	interface Props {
		children: Snippet;
	}

	const { children }: Props = $props();
</script>

<div class="avatar-grouping">
	{@render children()}
</div>

<style lang="postcss">
	.avatar-grouping {
		display: flex;

		:global(& > *) {
			margin-right: -4px;

			&:last-child {
				margin-right: 0;
			}
		}
	}
</style>
