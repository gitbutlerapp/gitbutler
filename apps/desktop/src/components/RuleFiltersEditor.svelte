<script lang="ts">
	import NewRuleMenu from '$components/NewRuleMenu.svelte';
	import {
		type SemanticType,
		type RuleFilterType,
		SEMANTIC_TYPES,
		type RuleFilter,
		treeStatusToString,
		semanticTypeToString,
		canAddMoreFilters,
		type RuleFilterMap
	} from '$lib/rules/rule';
	import { typedKeys } from '$lib/utils/object';
	import { Button, Select, SelectItem, Textbox, FileStatusBadge } from '@gitbutler/ui';
	import type { FileStatus } from '@gitbutler/ui/components/file/types';

	const FILE_STATUS_OPTIONS: FileStatus[] = ['addition', 'modification', 'deletion', 'rename'];

	type Props = {
		initialFilterValues: Partial<RuleFilterMap>;
		addFilter: (type: RuleFilterType) => void;
		deleteFilter: (type: RuleFilterType) => void;
	};

	const { initialFilterValues, addFilter, deleteFilter }: Props = $props();

	let addFilterButton = $state<HTMLDivElement>();
	let newFilterContextMenu = $state<NewRuleMenu>();

	let pathRegex = $state<string | undefined>(initialFilterValues.pathMatchesRegex ?? undefined);
	let contentRegex = $state<string | undefined>(
		initialFilterValues.contentMatchesRegex ?? undefined
	);
	let treeChangeType = $state<FileStatus | undefined>(
		initialFilterValues.fileChangeType ?? undefined
	);
	let semanticType = $state<SemanticType | undefined>(initialFilterValues.semanticType?.type);

	const ruleFilterTypes = $derived(typedKeys(initialFilterValues));

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
		newFilterContextMenu?.toggle(e);
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
		iconLeft="folder"
		wide
		value={pathRegex}
		oninput={(v) => (pathRegex = v)}
		placeholder="Path e.g. src/components or a regex"
		autofocus
	/>
{/snippet}

<!-- Content filter -->
{#snippet contentMatchesRegex()}
	<Textbox
		iconLeft="text-width"
		wide
		value={contentRegex}
		oninput={(v) => (contentRegex = v)}
		placeholder="String e.g. TODO or a regex"
		autofocus
	/>
{/snippet}

<!-- File change type filter -->
{#snippet fileChangeType()}
	<Select
		value={treeChangeType}
		options={FILE_STATUS_OPTIONS.map((type) => ({
			label: treeStatusToString(type),
			value: type
		}))}
		placeholder="Change type..."
		flex="1"
		icon="file-changes"
		autofocus
		onselect={(selected) => {
			treeChangeType = selected as FileStatus;
		}}
	>
		{#snippet itemSnippet({ item, highlighted })}
			<SelectItem selected={item.value === treeChangeType} {highlighted}>
				{item.label}
				{#snippet iconSnippet()}
					<FileStatusBadge style="dot" status={item.value as FileStatus} />
				{/snippet}
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
		icon="tag"
		autofocus
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
				size="cta"
				class="rule-filter-row__button"
				kind="ghost"
				width="auto"
				onclick={() => {
					deleteFilter(type);
				}}
			/>
			{#if isLastFilterType(type) && canAddMore}
				<Button
					bind:el={addFilterButton}
					class="rule-filter-row__button"
					width="auto"
					icon="plus"
					size="cta"
					kind="ghost"
					onclick={handleAddFilter}
				/>
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
		margin-bottom: 8px;
		gap: 8px;

		&:last-child {
			margin-bottom: 0;
		}
	}

	.rule-filter-row__actions {
		display: flex;
		align-items: center;
	}

	:global(.rule-filter-row .rule-filter-row__button) {
		padding: 0 6px;
	}
</style>
