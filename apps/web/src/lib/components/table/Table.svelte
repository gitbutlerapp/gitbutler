<script lang="ts">
	import TableHead from './TableHead.svelte';
	import { type ColumnTypes } from './types';
	import { type Snippet } from 'svelte';

	type Props = {
		headColumns?: Array<{ key: keyof ColumnTypes; value?: string; tooltip?: string }>;
		body: Snippet;
	};

	const { headColumns, body }: Props = $props();
</script>

<table class="dyntbl">
	{#if headColumns}
		<TableHead columns={headColumns} />
	{/if}
	<tbody>
		{@render body()}
	</tbody>
</table>

<style lang="postcss">
	.dyntbl {
		--cell-padding: 14px;

		width: 100%;
		border-collapse: separate;
		/* border-collapse: collapse; */
		border-spacing: 0;
	}
</style>
