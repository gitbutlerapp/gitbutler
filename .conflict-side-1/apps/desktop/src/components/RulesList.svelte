<script lang="ts">
	import NewRuleMenu from '$components/NewRuleMenu.svelte';
	import ReduxResult from '$components/ReduxResult.svelte';
	import Rule from '$components/Rule.svelte';
	import RuleFiltersEditor from '$components/RuleFiltersEditor.svelte';
	import { focusable } from '$lib/focus/focusable.svelte';
	import {
		type WorkspaceRuleId,
		type RuleFilterMap,
		type RuleFilterType,
		type WorkspaceRule,
		type StackTarget,
		encodeStackTarget,
		decodeStackTarget,
		compareStackTarget,
		type RuleFilter
	} from '$lib/rules/rule';
	import { RULES_SERVICE } from '$lib/rules/rulesService.svelte';
	import { getStackName } from '$lib/stacks/stack';
	import { STACK_SERVICE } from '$lib/stacks/stackService.svelte';
	import { typedKeys } from '$lib/utils/object';
	import { inject } from '@gitbutler/shared/context';
	import { Button, chipToasts, Select, SelectItem, Icon } from '@gitbutler/ui';
	import { isDefined } from '@gitbutler/ui/utils/typeguards';
	import type { Snippet } from 'svelte';

	type Props = {
		projectId: string;
		foldButton: Snippet;
	};

	const { foldButton, projectId }: Props = $props();

	const rulesService = inject(RULES_SERVICE);
	const stackService = inject(STACK_SERVICE);

	const [create, creatingRule] = rulesService.createWorkspaceRule;
	const [update, updatingRule] = rulesService.updateWorkspaceRule;

	let selectedRuleId = $state<WorkspaceRuleId>();
	let stackTargetSelected = $state<StackTarget>();
	const encodedStackTarget = $derived(
		stackTargetSelected && encodeStackTarget(stackTargetSelected)
	);

	let draftRuleFilterInitialValues = $state<Partial<RuleFilterMap>>({});
	const ruleFilterTypes = $derived(typedKeys(draftRuleFilterInitialValues));

	// Component references
	let ruleFiltersEditor = $state<RuleFiltersEditor>();

	let addRuleButton = $state<HTMLDivElement>();
	let newRuleContextMenu = $state<NewRuleMenu>();

	let addFilterButton = $state<HTMLDivElement>();
	let newFilterContextMenu = $state<NewRuleMenu>();

	// Visual state
	let mode = $state<'list' | 'edit' | 'add'>('list');
	let editingRuleId = $state<WorkspaceRuleId | null>(null);

	const validFilters = $derived(!ruleFiltersEditor || ruleFiltersEditor.imports.filtersValid);
	const canSaveRule = $derived(stackTargetSelected !== undefined && validFilters);

	function openAddRuleContextMenu(e: MouseEvent) {
		e.stopPropagation();
		newRuleContextMenu?.toggle(e);
	}

	function openAddFilterContextMenu(e: MouseEvent) {
		e.stopPropagation();
		newFilterContextMenu?.toggle(e);
	}

	function addDraftRuleFilter(type: RuleFilterType) {
		const ruleTypes = typedKeys(draftRuleFilterInitialValues);
		if (!ruleTypes.includes(type)) {
			draftRuleFilterInitialValues[type] = null;
		}
	}

	function removeDraftRuleFilter(type: RuleFilterType) {
		draftRuleFilterInitialValues[type] = undefined;
		delete draftRuleFilterInitialValues[type];
	}

	function openRuleEditor() {
		mode = 'add';
	}

	function resetEditor() {
		stackTargetSelected = undefined;
		draftRuleFilterInitialValues = {};
		selectedRuleId = undefined;
		editingRuleId = null;
	}

	function cancelRuleEdition() {
		mode = 'list';
		resetEditor();
	}

	function updateInitialValues(filter: RuleFilter, initialValues: Partial<RuleFilterMap>): true {
		switch (filter.type) {
			case 'pathMatchesRegex':
				initialValues.pathMatchesRegex = filter.subject;
				return true;
			case 'contentMatchesRegex':
				initialValues.contentMatchesRegex = filter.subject;
				return true;
			case 'fileChangeType':
				initialValues.fileChangeType = filter.subject;
				return true;
			case 'semanticType':
				initialValues.semanticType = filter.subject;
				return true;
			case 'claudeCodeSessionId':
				initialValues.claudeCodeSessionId = filter.subject;
				return true;
		}
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

		selectedRuleId = rule.id;
		editingRuleId = rule.id;
		stackTargetSelected = rule.action.subject.subject.target;
		const initialValues: Partial<RuleFilterMap> = {};

		for (const filter of rule.filters) {
			updateInitialValues(filter, initialValues);
		}

		draftRuleFilterInitialValues = initialValues;

		mode = 'edit';
	}

	async function saveRule() {
		// Logic to save the rule
		// This needs to be writtent like this.
		// If there's no rule filters editor, it means that the match is all files
		const ruleFilters = ruleFiltersEditor ? ruleFiltersEditor.getRuleFilters() : [];

		if (ruleFilters === undefined) {
			chipToasts.error('Invalid rule filters');
			return;
		}

		if (!stackTargetSelected) {
			chipToasts.error('Please select a branch to assign the rule');
			return;
		}

		if (selectedRuleId) {
			await update({
				projectId,
				request: {
					id: selectedRuleId,
					enabled: null,
					action: {
						type: 'explicit',
						subject: {
							type: 'assign',
							subject: { target: stackTargetSelected }
						}
					},
					filters: ruleFilters,
					trigger: null
				}
			});
		} else {
			await create({
				projectId,
				request: {
					action: {
						type: 'explicit',
						subject: {
							type: 'assign',
							subject: { target: stackTargetSelected }
						}
					},
					filters: ruleFilters,
					trigger: 'fileSytemChange'
				}
			});
		}

		mode = 'list';
		resetEditor();
	}
</script>

<div class="rules-list" use:focusable>
	<div class="rules-list__header">
		<div class="rules-list__title">
			{@render foldButton()}
			<h3 class="text-14 text-semibold">Rules</h3>
		</div>

		<div bind:this={addRuleButton}>
			<Button
				icon="plus-small"
				size="tag"
				kind="outline"
				tooltip="Automate actions for new code changes"
				onclick={openAddRuleContextMenu}
				disabled={mode === 'edit' || mode === 'add'}
				loading={creatingRule.current.isLoading}>Add rule</Button
			>
		</div>
	</div>

	{#if mode === 'add'}
		{@render ruleEditor()}
	{/if}
	{@render ruleListContent()}
</div>

{#snippet ruleListContent()}
	{@const rules = rulesService.workspaceRules(projectId)}
	<ReduxResult {projectId} result={rules.current}>
		{#snippet children(rules)}
			{#if rules.length > 0}
				<div class="rules-list__content">
					{#each rules as rule (rule.id)}
						{#if editingRuleId === rule.id}
							{@render ruleEditor()}
						{:else}
							<Rule {projectId} {rule} editRule={() => editExistingRule(rule)} />
						{/if}
					{/each}
				</div>
			{/if}
		{/snippet}
	</ReduxResult>
{/snippet}

{#snippet ruleEditor()}
	{@const stackEntries = stackService.stacks(projectId)}
	<div class="rules-list__editor-content">
		{#if typedKeys(draftRuleFilterInitialValues).length > 0}
			<div class="rules-list__filters">
				<RuleFiltersEditor
					bind:this={ruleFiltersEditor}
					{projectId}
					initialFilterValues={draftRuleFilterInitialValues}
					addFilter={addDraftRuleFilter}
					deleteFilter={removeDraftRuleFilter}
				/>
			</div>
		{:else}
			<div class="rules-list__matches-all">
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
					{@const stackOptions = [
						...stacks
							.map((stack) =>
								stack.id
									? {
											label: getStackName(stack),
											value: encodeStackTarget({ type: 'stackId', subject: stack.id })
										}
									: undefined
							)
							.filter(isDefined),
						{ separator: true } as const,
						{
							label: 'Leftmost lane',
							value: encodeStackTarget({ type: 'leftmost' })
						},
						{
							label: 'Rightmost lane',
							value: encodeStackTarget({ type: 'rightmost' })
						}
					]}

					<Select
						value={encodedStackTarget}
						options={stackOptions}
						placeholder="Select a branch…"
						flex="1"
						searchable
						onselect={(selectedStackTarget) => {
							stackTargetSelected = decodeStackTarget(selectedStackTarget);
						}}
						icon={stackTargetSelected
							? stackTargetSelected.type === 'leftmost'
								? 'leftmost-lane'
								: stackTargetSelected.type === 'rightmost'
									? 'rightmost-lane'
									: 'branch-remote'
							: 'branch-remote'}
					>
						{#snippet itemSnippet({ item, highlighted })}
							<SelectItem
								selected={item.value ? compareStackTarget(item.value, stackTargetSelected) : false}
								{highlighted}
							>
								{item.label || ''}
								{#snippet iconSnippet()}
									{#if item.value}
										{@const target = decodeStackTarget(item.value)}
										{#if target.type === 'leftmost'}
											<Icon name="leftmost-lane" />
										{:else if target.type === 'rightmost'}
											<Icon name="rightmost-lane" />
										{:else}
											<Icon name="branch-remote" />
										{/if}
									{/if}
								{/snippet}
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
				loading={stackEntries.current.isLoading ||
					creatingRule.current.isLoading ||
					updatingRule.current.isLoading}>Save rule</Button
			>
		</div>
	</div>

	<NewRuleMenu
		bind:this={newFilterContextMenu}
		addedFilterTypes={ruleFilterTypes}
		trigger={addFilterButton}
		addFromFilter={(type) => {
			addDraftRuleFilter(type);
		}}
	/>
{/snippet}

<NewRuleMenu
	bind:this={newRuleContextMenu}
	addedFilterTypes={ruleFilterTypes}
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

	/* HEADER */
	.rules-list__header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		height: 42px;
		padding: 0 10px 0 14px;
		gap: 8px;
		border-bottom: 1px solid var(--clr-border-2);
		background-color: var(--clr-bg-2);
	}

	.rules-list__title {
		display: flex;
		align-items: center;
		gap: 6px;
	}

	.rules-list__content {
		display: flex;
		flex-direction: column;
	}

	.rules-list__editor-content {
		display: flex;
		flex-direction: column;
		padding: 12px;
		gap: 16px;
		border-bottom: 1px solid var(--clr-border-2);
	}

	.rules-list__filters {
		display: flex;
		flex-direction: column;
	}

	.rules-list__matches-all {
		display: flex;
		align-items: center;
		justify-content: center;
		height: var(--size-cta);
		padding: 0 12px;
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
		gap: 6px;
	}
</style>
