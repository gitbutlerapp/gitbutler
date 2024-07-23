<script lang="ts">
	import { type Snippet } from 'svelte';

	interface Props {
		lines: Snippet;
		action: Snippet;
	}

	const { lines, action }: Props = $props();
</script>

<div class="row">
	<div>
		{@render lines()}
	</div>
	<div class="action">
		{@render action()}
	</div>
</div>

<style lang="postcss">
	.row {
		display: flex;
		border-bottom: 1px solid var(--clr-border-2);
	}

	.action {
		display: flex;
		flex-direction: column;
		width: 100%;
		padding-top: 10px;
		padding-right: 14px;
		padding-bottom: 10px;
	}
</style>
