<script lang="ts">
	import { type ColumnTypes } from '$lib/components/table/types';

	type Props = {
		columns: Array<{
			key: keyof ColumnTypes;
			value?: string;
			tooltip?: string;
		}>;
	};

	const { columns }: Props = $props();
</script>

<thead class="dynthead">
	<tr>
		{#each columns as { key, value, tooltip }}
			<th>
				<div class="text-12 dynthead-title dynthead_{key}" title={tooltip}>
					{value}
				</div>
			</th>
		{/each}
	</tr>
</thead>

<style lang="postcss">
	.dynthead {
		@media (--mobile-viewport) {
			display: none;
		}
	}

	.dynthead th {
		border: 1px solid var(--clr-border-2);
		background-color: var(--clr-bg-2);
		vertical-align: top;

		&:not(:last-child) {
			border-right: none;
		}

		&:not(:first-child) {
			border-left: none;
		}

		&:first-child {
			border-top-left-radius: var(--radius-ml);
		}

		&:last-child {
			border-top-right-radius: var(--radius-ml);
		}
	}

	.dynthead-title {
		padding: var(--cell-padding);
		color: var(--clr-text-2);
		text-align: left;
	}

	/* MODIFIERS */
	.dynthead_changes {
		text-align: right;
	}
</style>
