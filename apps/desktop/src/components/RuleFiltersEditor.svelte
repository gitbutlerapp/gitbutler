<script lang="ts">
	import NewRuleMenu from '$components/NewRuleMenu.svelte';
	import {
		type SemanticType,
		type TreeStatus,
		type RuleFilterType,
		RULE_FILTER_TREE_STATUS,
		SEMANTIC_TYPES,
		type RuleFilter,
		treeStatusToString,
		semanticTypeToString,
		canAddMoreFilters
	} from '$lib/rules/rule';
	import { Button, Select, SelectItem, Textbox } from '@gitbutler/ui';

	type Props = {
		ruleFilterTypes: RuleFilterType[];
		addFilter: (type: RuleFilterType) => void;
		deleteFilter: (type: RuleFilterType) => void;
	};

	const { ruleFilterTypes, addFilter, deleteFilter }: Props = $props();

	let addFilterButton = $state<HTMLDivElement>();
	let newFilterContextMenu = $state<NewRuleMenu>();

	let pathRegex = $state<string>();
	let contentRegex = $state<string>();
	let treeChangeType = $state<TreeStatus>();
	let semanticType = $state<SemanticType>();

	const orderMap: Record<RuleFilterType, number> = {
		pathMatchesRegex: 0,
		contentMatchesRegex: 1,
		fileChangeType: 2,
		semanticType: 3
	};

	function isLastFilterType(type: RuleFilterType): boolean {
		const orderValue = orderMap[type];
		const allFilterOrderValue = ruleFilterTypes.map((t) => orderMap[t]);
		const maxOrderValue = Math.max(...allFilterOrderValue);
		return orderValue === maxOrderValue;
	}

	function areDraftRuleFiltersReady(types: RuleFilterType[]): boolean {
		for (const type of types) {
			if (type === 'pathMatchesRegex' && !pathRegex) return false;
			if (type === 'contentMatchesRegex' && !contentRegex) return false;
			if (type === 'fileChangeType' && !treeChangeType) return false;
			if (type === 'semanticType' && !semanticType) return false;
		}
		return true;
	}

	const filtersAreReady = $derived(areDraftRuleFiltersReady(ruleFilterTypes));
	const canAddMore = $derived(canAddMoreFilters(ruleFilterTypes));

	function handleAddFilter(e: MouseEvent) {
		e.stopPropagation();
		newFilterContextMenu?.open(e);
	}

	export function getRuleFilters(): RuleFilter[] | undefined {
		if (!filtersAreReady) return undefined;

		const filters: RuleFilter[] = [];

		if (ruleFilterTypes.includes('pathMatchesRegex') && pathRegex) {
			filters.push({ type: 'pathMatchesRegex', subject: pathRegex });
		}

		if (ruleFilterTypes.includes('contentMatchesRegex') && contentRegex) {
			filters.push({ type: 'contentMatchesRegex', subject: contentRegex });
		}

		if (ruleFilterTypes.includes('fileChangeType') && treeChangeType) {
			filters.push({ type: 'fileChangeType', subject: treeChangeType });
		}

		if (
			ruleFilterTypes.includes('semanticType') &&
			semanticType &&
			semanticType !== 'userDefined'
		) {
			filters.push({ type: 'semanticType', subject: { type: semanticType } });
		}

		return filters;
	}

	export const imports = {
		get filtersValid() {
			return filtersAreReady;
		}
	};
</script>

<!-- Path filter -->
{#snippet pathMatchesRegex()}
	<Textbox
		icon="folder"
		wide
		value={pathRegex}
		onchange={(v) => (pathRegex = v)}
		placeholder="Path: e.g. src/components"
	/>
{/snippet}

<!-- Content filter -->
{#snippet contentMatchesRegex()}
	<Textbox
		icon="text-width"
		wide
		value={contentRegex}
		onchange={(v) => (contentRegex = v)}
		placeholder="Contains: e.g. TODO"
	/>
{/snippet}

<!-- File change type filter -->
{#snippet fileChangeType()}
	<Select
		value={treeChangeType}
		options={RULE_FILTER_TREE_STATUS.map((type) => ({
			label: treeStatusToString(type),
			value: type
		}))}
		placeholder="Change type..."
		flex="1"
		onselect={(selected) => {
			treeChangeType = selected as TreeStatus;
		}}
	>
		{#snippet itemSnippet({ item, highlighted })}
			<SelectItem selected={item.value === treeChangeType} {highlighted}>
				{item.label}
			</SelectItem>
		{/snippet}
	</Select>
{/snippet}

<!-- Semantic type -->
{#snippet semanticTypeFilter()}
	<Select
		value={semanticType}
		options={SEMANTIC_TYPES.map((type) => ({ label: semanticTypeToString(type), value: type }))}
		placeholder="Work category..."
		flex="1"
		searchable
		onselect={(selected) => {
			semanticType = selected as SemanticType;
		}}
	>
		{#snippet itemSnippet({ item, highlighted })}
			<SelectItem selected={item.value === semanticType} {highlighted}>
				{item.label}
			</SelectItem>
		{/snippet}
	</Select>
{/snippet}

<!-- This is the parent component,
        wraps around the rule input -->
{#snippet ruleFilterRow(type: RuleFilterType)}
	<div class="rule-filter-row">
		{#if type === 'pathMatchesRegex'}
			{@render pathMatchesRegex()}
		{:else if type === 'contentMatchesRegex'}
			{@render contentMatchesRegex()}
		{:else if type === 'fileChangeType'}
			{@render fileChangeType()}
		{:else if type === 'semanticType'}
			{@render semanticTypeFilter()}
		{/if}

		<div class="rule-filter-row__actions">
			<Button
				icon="bin"
				size="icon"
				kind="ghost"
				onclick={() => {
					deleteFilter(type);
				}}
			/>
			{#if isLastFilterType(type) && canAddMore}
				<div bind:this={addFilterButton}>
					<Button icon="plus" size="icon" kind="ghost" onclick={handleAddFilter} />
				</div>
			{/if}
		</div>
	</div>
{/snippet}

{#if ruleFilterTypes.includes('pathMatchesRegex')}
	{@render ruleFilterRow('pathMatchesRegex')}
{/if}

{#if ruleFilterTypes.includes('contentMatchesRegex')}
	{@render ruleFilterRow('contentMatchesRegex')}
{/if}

{#if ruleFilterTypes.includes('fileChangeType')}
	{@render ruleFilterRow('fileChangeType')}
{/if}

{#if ruleFilterTypes.includes('semanticType')}
	{@render ruleFilterRow('semanticType')}
{/if}

<NewRuleMenu
	bind:this={newFilterContextMenu}
	addedFilterTypes={ruleFilterTypes}
	trigger={addFilterButton}
	addFromFilter={(type) => {
		addFilter(type);
	}}
/>

<style lang="postcss">
	.rule-filter-row {
		display: flex;
		align-items: center;
		gap: 8px;
	}

	.rule-filter-row__actions {
		display: flex;
		align-items: center;
		gap: 4px;
	}
</style>
