<script lang="ts">
	import ReduxResult from '$components/ReduxResult.svelte';
	import { Badge, Select, SelectItem, Button } from '@gitbutler/ui';
	import type { ForgePrService } from '$lib/forge/interface/forgePrService';

	interface Props {
		projectId: string;
		prService: ForgePrService | undefined;
		selectedLabels: string[];
		disabled?: boolean;
	}

	let { projectId, prService, selectedLabels = $bindable(), disabled }: Props = $props();

	let labelsQuery = $state<ReturnType<NonNullable<typeof prService>['labels']>>();

	function fetchLabels() {
		if (prService) {
			if (!labelsQuery) {
				labelsQuery = prService.labels();
			} else {
				labelsQuery.result.refetch();
			}
		}
	}
</script>

<div class="label-section">
	<div class="label-section__header">
		<span class="label-section__title text-13 text-semibold">Labels</span>
		<Select
			options={(labelsQuery?.result?.data || [])
				.filter((l) => !selectedLabels.includes(l.name))
				.map((l) => ({ label: l.name, value: l.name }))}
			loading={labelsQuery?.result?.status === 'pending'}
			searchable
			autoWidth={true}
			popupAlign="left"
			closeOnSelect={false}
			{disabled}
			onselect={(value) => {
				if (!selectedLabels.includes(value)) {
					selectedLabels = [...selectedLabels, value];
				}
			}}
			ontoggle={(isOpen) => {
				if (isOpen) fetchLabels();
			}}
		>
			{#snippet customSelectButton()}
				<Button
					kind="ghost"
					class="text-13"
					{disabled}
					loading={labelsQuery?.result?.status === 'pending'}
				>
					Edit
				</Button>
			{/snippet}
			{#snippet itemSnippet({ item, highlighted })}
				<SelectItem selected={selectedLabels.includes(item.value)} {highlighted}>
					{item.label}
				</SelectItem>
			{/snippet}
		</Select>
	</div>

	<div class="label-section__content">
		{#if selectedLabels.length > 0}
			<div class="labels-list">
				{#each selectedLabels as label}
					<Badge
						color="gray"
						kind="soft"
						size="tag"
						icon="cross-small"
						reversedDirection
						onclick={() => (selectedLabels = selectedLabels.filter((l) => l !== label))}
					>
						{label}
					</Badge>
				{/each}
			</div>
		{:else}
			<span class="text-13 text-secondary">None</span>
		{/if}
	</div>

	{#if labelsQuery?.result?.error}
		<ReduxResult {projectId} result={labelsQuery.result} />
	{/if}
</div>

<style lang="postcss">
	.label-section {
		display: flex;
		flex-direction: column;
		gap: 6px;
	}

	.label-section__header {
		display: flex;
		justify-content: space-between;
		align-items: center;

		& :global(.btn-label) {
			font-size: var(--size-13);
		}
	}

	.label-section__title {
		color: var(--clr-text-1);
	}

	.label-section__content {
		display: flex;
		flex-wrap: wrap;
		min-height: 20px;
	}

	.labels-list {
		display: flex;
		flex-wrap: wrap;
		gap: 4px;
	}

	.text-secondary {
		color: var(--clr-text-2);
	}
</style>
