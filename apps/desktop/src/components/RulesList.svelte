<script lang="ts">
	import NewRuleMenu from '$components/NewRuleMenu.svelte';
	import ReduxResult from '$components/ReduxResult.svelte';
	import Rule from '$components/Rule.svelte';
	import RuleFiltersEditor from '$components/RuleFiltersEditor.svelte';
	import { RULES_SERVICE } from '$lib/rules/rulesService.svelte';
	import { getStackName } from '$lib/stacks/stack';
	import { STACK_SERVICE } from '$lib/stacks/stackService.svelte';
	import { inject } from '@gitbutler/shared/context';
	import { Button, chipToasts, Select, SelectItem } from '@gitbutler/ui';
	import type { RuleFilterType, WorkspaceRule } from '$lib/rules/rule';
	import type { Snippet } from 'svelte';

	type Props = {
		projectId: string;
		foldButton: Snippet;
	};

	const { foldButton, projectId }: Props = $props();

	const rulesService = inject(RULES_SERVICE);
	const stackService = inject(STACK_SERVICE);

	const [create, creatingRule] = rulesService.createWorkspaceRule;

	let stackIdSelected = $state<string>();
	let draftRuleFilters = $state<RuleFilterType[]>([]);

	// Component references
	let ruleFiltersEditor = $state<RuleFiltersEditor>();

	let addRuleButton = $state<HTMLDivElement>();
	let newRuleContextMenu = $state<NewRuleMenu>();

	let addFilterButton = $state<HTMLDivElement>();
	let newFilterContextMenu = $state<NewRuleMenu>();

	// Visual state
	let mode = $state<'list' | 'edit'>('list');

	const validFilters = $derived(!ruleFiltersEditor || ruleFiltersEditor.imports.filtersValid);
	const canSaveRule = $derived(stackIdSelected !== undefined && validFilters);

	function openAddRuleContextMenut(e: MouseEvent) {
		e.stopPropagation();
		newRuleContextMenu?.open(e);
	}

	function openAddFilterContextMenu(e: MouseEvent) {
		e.stopPropagation();
		newFilterContextMenu?.open(e);
	}

	function addDraftRuleFilter(type: RuleFilterType) {
		if (!draftRuleFilters.includes(type)) {
			draftRuleFilters.push(type);
		}
	}

	function removeDraftRuleFilter(type: RuleFilterType) {
		draftRuleFilters = draftRuleFilters.filter((filterType) => filterType !== type);
	}

	function openRuleEditor() {
		mode = 'edit';
	}

	function resetEditor() {
		stackIdSelected = undefined;
		draftRuleFilters = [];
	}

	function cancelRuleEdition() {
		mode = 'list';
		resetEditor();
	}

	async function editExistingRule(rule: WorkspaceRule) {
		if (rule.action.type === 'implicit') {
			chipToasts.error('Cannot edit implicit rules');
			return;
		}

		if (rule.action.subject.type !== 'assign') {
			chipToasts.error('Cannot edit rules that are not branch assignments');
			return;
		}

		stackIdSelected = rule.action.subject.subject.stack_id;
		draftRuleFilters = rule.filters.map((r) => r.type);
		mode = 'edit';
	}

	async function saveRule() {
		// Logic to save the rule
		const ruleFilters = ruleFiltersEditor?.getRuleFilters();

		if (!ruleFilters) {
			chipToasts.error('Invalid rule filters');
			return;
		}

		if (!stackIdSelected) {
			chipToasts.error('Please select a branch to assign the rule');
			return;
		}

		await create({
			projectId,
			request: {
				action: {
					type: 'explicit',
					subject: {
						type: 'assign',
						subject: {
							stack_id: stackIdSelected
						}
					}
				},
				filters: ruleFilters,
				trigger: 'fileSytemChange'
			}
		});

		mode = 'list';
		resetEditor();
	}
</script>

<div class="rules-list">
	<div class="rules-list__header">
		<div class="rules-list__title">
			{@render foldButton()}
			<h3 class="text-14 text-semibold truncate">Rules</h3>
		</div>

		<div bind:this={addRuleButton}>
			<Button
				icon="plus-small"
				kind="outline"
				onclick={openAddRuleContextMenut}
				disabled={mode === 'edit'}
				loading={creatingRule.current.isLoading}>Add rule</Button
			>
		</div>
	</div>

	{#if mode === 'list'}
		{@render ruleListContent()}
	{:else if mode === 'edit'}
		{@render ruleEditor()}
	{/if}
</div>

{#snippet ruleListContent()}
	{@const rules = rulesService.listWorkspaceRules(projectId)}
	<ReduxResult {projectId} result={rules.current}>
		{#snippet children(rules)}
			{#if rules.length > 0}
				<div class="rules-list__content">
					{#each rules as rule (rule.id)}
						<Rule {projectId} {rule} editRule={() => editExistingRule(rule)} />
					{/each}
				</div>
			{/if}
		{/snippet}
	</ReduxResult>
{/snippet}

{#snippet ruleEditor()}
	{@const stackEntries = stackService.stacks(projectId)}
	<div class="rules-list__editor-content">
		{#if draftRuleFilters.length > 0}
			<RuleFiltersEditor
				bind:this={ruleFiltersEditor}
				ruleFilterTypes={draftRuleFilters}
				addFilter={addDraftRuleFilter}
				deleteFilter={removeDraftRuleFilter}
			/>
		{:else}
			<div class="rules-list__content-message">
				<p class="text-12">Matches all changes</p>
				<div bind:this={addFilterButton} class="rules-list__add-filter-button text-12">
					<button type="button" onclick={openAddFilterContextMenu}>
						<span class="underline-dotted"> Add filter +</span>
					</button>
				</div>
			</div>
		{/if}

		<div class="rules-list__action">
			<h3 class="text-13 text-semibold">Assign to branch</h3>
			<ReduxResult {projectId} result={stackEntries.current}>
				{#snippet children(stacks)}
					<Select
						value={stackIdSelected}
						options={stacks.map((stack) => ({ label: getStackName(stack), value: stack.id }))}
						placeholder="Select a branch"
						flex="1"
						searchable
						onselect={(selectedId) => {
							stackIdSelected = selectedId;
						}}
					>
						{#snippet itemSnippet({ item, highlighted })}
							<SelectItem selected={item.value === stackIdSelected} {highlighted}>
								{item.label}
							</SelectItem>
						{/snippet}
					</Select>
				{/snippet}
			</ReduxResult>
		</div>

		<div class="rules-list__editor-buttons">
			<Button onclick={cancelRuleEdition} kind="outline">Cancel</Button>
			<Button
				onclick={saveRule}
				kind="solid"
				style="neutral"
				disabled={!canSaveRule}
				loading={stackEntries.current.isLoading || creatingRule.current.isLoading}>Save</Button
			>
		</div>
	</div>

	<NewRuleMenu
		bind:this={newFilterContextMenu}
		addedFilterTypes={draftRuleFilters}
		trigger={addFilterButton}
		addFromFilter={(type) => {
			addDraftRuleFilter(type);
		}}
	/>
{/snippet}

<NewRuleMenu
	bind:this={newRuleContextMenu}
	addedFilterTypes={draftRuleFilters}
	trigger={addRuleButton}
	addFromFilter={(type) => {
		addDraftRuleFilter(type);
		openRuleEditor();
	}}
	addEmpty={() => {
		openRuleEditor();
	}}
/>

<style lang="postcss">
	.rules-list {
		display: flex;
		flex-direction: column;
	}

	.rules-list__header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding: 10px 10px 10px 14px;
		gap: 8px;
		border-bottom: 1px solid var(--clr-border-2);
	}

	.rules-list__title {
		display: flex;
		align-items: center;
		gap: 6px;
	}

	.rules-list__content {
		display: flex;
		flex-direction: column;
		border-bottom: 1px solid var(--clr-border-2);
	}

	.rules-list__editor-content {
		display: flex;
		flex-direction: column;
		padding: 12px;
		gap: 16px;
		border-bottom: 1px solid var(--clr-border-2);
	}

	.rules-list__content-message {
		display: flex;
		padding: 16px;
		gap: 8px;
		border-radius: var(--radius-m);
		background-color: var(--clr-bg-2);

		& p {
			color: var(--clr-text-2);
			text-align: center;
		}
	}

	.rules-list__action {
		display: flex;
		flex-direction: column;
		gap: 10px;
	}

	.rules-list__add-filter-button {
		color: var(--clr-text-2);
	}

	.rules-list__editor-buttons {
		display: flex;
		justify-content: flex-end;
		gap: 8px;
	}
</style>
