<script lang="ts">
	import { type ColumnTypes } from './types';

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
		text-align: left;
		color: var(--clr-text-2);
		padding: var(--cell-padding);
	}

	/* MODIFIERS */
	.dynthead_changes,
	.dynthead_number {
		text-align: right;
	}
</style>
