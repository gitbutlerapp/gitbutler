<script lang="ts">
	import Drawer from '$components/Drawer.svelte';
	import NewRuleMenu from '$components/NewRuleMenu.svelte';
	import ReduxResult from '$components/ReduxResult.svelte';
	import Rule from '$components/Rule.svelte';
	import RuleFiltersEditor from '$components/RuleFiltersEditor.svelte';
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
	import { RULES_SERVICE, workspaceRulesSelectors } from '$lib/rules/rulesService.svelte';
	import { getStackName } from '$lib/stacks/stack';
	import { STACK_SERVICE } from '$lib/stacks/stackService.svelte';
	import { typedKeys } from '$lib/utils/object';
	import { inject } from '@gitbutler/core/context';
	import {
		Button,
		chipToasts,
		Select,
		SelectItem,
		Icon,
		Badge,
		Spacer,
		SkeletonBone
	} from '@gitbutler/ui';
	import { focusable } from '@gitbutler/ui/focus/focusable';
	import { isDefined } from '@gitbutler/ui/utils/typeguards';
	import { slide } from 'svelte/transition';

	type Props = {
		projectId: string;
	};

	const { projectId }: Props = $props();

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

	let addFilterButton = $state<HTMLDivElement>();
	let newFilterContextMenu = $state<NewRuleMenu>();

	// Visual state
	let mode = $state<'list' | 'edit' | 'add'>('list');
	let editingRuleId = $state<WorkspaceRuleId | null>(null);

	// Read initial collapsed state from persisted storage, default to false (open) if not found
	const drawerPersistId = `rules-drawer-${projectId}`;
	let drawer = $state<Drawer>();

	const validFilters = $derived(!ruleFiltersEditor || ruleFiltersEditor.imports.filtersValid);
	const canSaveRule = $derived(stackTargetSelected !== undefined && validFilters);

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
		if (editingRuleId !== null) {
			chipToasts.error('Please finish editing the current rule first');
			return;
		}
		// Open drawer if it's collapsed
		if (drawer?.getIsCollapsed()) {
			drawer.open();
		}

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
		if (mode === 'add' || (editingRuleId !== null && editingRuleId !== rule.id)) {
			chipToasts.error('Please finish editing the current rule first');
			return;
		}

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

	const rules = $derived(rulesService.workspaceRules(projectId));

	function separateRules(allRules: WorkspaceRule[]) {
		const regularRules: WorkspaceRule[] = [];
		const aiRules: WorkspaceRule[] = [];

		for (const rule of allRules) {
			const hasAiFilter = rule.filters.some(
				(filter: RuleFilter) => filter.type === 'claudeCodeSessionId'
			);
			if (hasAiFilter) {
				aiRules.push(rule);
			} else {
				regularRules.push(rule);
			}
		}

		return { regularRules, aiRules };
	}
</script>

<Drawer bind:this={drawer} bottomBorder={false} persistId={drawerPersistId} maxHeight="60%">
	{#snippet header()}
		<h4 class="text-14 text-semibold truncate">Rules</h4>
		{#if rules.result.isSuccess}
			<Badge>{rules.result.data.ids.length}</Badge>
		{:else}
			<Badge skeleton />
		{/if}
	{/snippet}
	{#snippet actions()}
		<Button onclick={openRuleEditor} icon="plus-small" size="tag" kind="ghost" />
	{/snippet}
	<div class="rules-list" use:focusable>
		{@render ruleListContent()}
	</div>
</Drawer>

{#snippet skeletonLoading()}
	<div class="rules-list__content">
		<div class="rule-skeleton">
			<SkeletonBone width="50%" height="var(--size-tag)" radius="var(--size-tag)" opacity={0.2} />
			<SkeletonBone width="40%" height="var(--size-tag)" radius="var(--size-tag)" opacity={0.2} />
		</div>
		<div class="rule-skeleton">
			<SkeletonBone width="40%" height="var(--size-tag)" radius="var(--size-tag)" opacity={0.2} />
			<SkeletonBone width="35%" height="var(--size-tag)" radius="var(--size-tag)" opacity={0.2} />
		</div>
		<div class="rule-skeleton">
			<SkeletonBone width="55%" height="var(--size-tag)" radius="var(--size-tag)" opacity={0.2} />
			<SkeletonBone width="30%" height="var(--size-tag)" radius="var(--size-tag)" opacity={0.2} />
		</div>
	</div>
{/snippet}

{#snippet ruleListContent()}
	<ReduxResult {projectId} result={rules.result}>
		{#snippet loading()}
			{@render skeletonLoading()}
		{/snippet}

		{#snippet children(rulesEntityState)}
			{@const rulesArray = workspaceRulesSelectors.selectAll(rulesEntityState)}
			{@const { regularRules, aiRules } = separateRules(rulesArray)}
			{@const totalRules = regularRules.length + aiRules.length}

			{#if totalRules > 0}
				{#if mode === 'add'}
					<div in:slide={{ duration: 150 }}>
						{@render ruleEditor()}
					</div>
				{/if}

				<div class="rules-list__content">
					{#if regularRules.length > 0}
						{#each regularRules.slice().reverse() as rule (rule.id)}
							{#if editingRuleId === rule.id}
								<div in:slide={{ duration: 150 }}>
									{@render ruleEditor()}
								</div>
							{:else}
								<Rule {projectId} {rule} editRule={() => editExistingRule(rule)} />
							{/if}
						{/each}
					{/if}

					{#if aiRules.length > 0}
						<div class="rules-section">
							{#each aiRules.slice().reverse() as rule (rule.id)}
								{#if editingRuleId === rule.id}
									<div in:slide={{ duration: 150 }}>
										{@render ruleEditor()}
									</div>
								{:else}
									<Rule {projectId} {rule} editRule={() => editExistingRule(rule)} />
								{/if}
							{/each}
						</div>
					{/if}
				</div>
			{:else if mode === 'add'}
				<div transition:slide={{ duration: 170 }}>
					{@render ruleEditor()}
				</div>
			{:else}
				<div class="rules-placeholder">
					<p class="text-13 text-body rules-placeholder-text">
						Set up rules to automatically route changes to the right branch.
					</p>
				</div>
			{/if}
		{/snippet}
	</ReduxResult>
{/snippet}

{#snippet ruleEditor()}
	{@const stackEntries = stackService.stacks(projectId)}
	<div class="rules-list__editor-content">
		<div class="rules-list__action">
			<h3 class="text-13 text-semibold">Assign to branch</h3>
			<ReduxResult {projectId} result={stackEntries.result}>
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
						minHeight={200}
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
				<p class="text-13">Matches all changes</p>
				<div bind:this={addFilterButton} class="rules-list__add-filter-button text-12">
					<button type="button" onclick={openAddFilterContextMenu}>
						<span class="clr-text-1 underline-dotted"> Add filter +</span>
					</button>
				</div>
			</div>
		{/if}

		<Spacer margin={2} dotted />

		<div class="rules-list__editor-buttons">
			<Button onclick={cancelRuleEdition} kind="outline">Cancel</Button>
			<Button
				onclick={saveRule}
				kind="solid"
				wide
				style="neutral"
				disabled={!canSaveRule}
				loading={stackEntries.result.isLoading ||
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

<style lang="postcss">
	.rules-list {
		display: flex;
		flex-direction: column;
	}

	.rules-placeholder {
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		padding: 24px 20px 32px;
		gap: 14px;
		background-color: var(--clr-bg-2);
	}

	.rules-placeholder-text {
		color: var(--clr-text-3);
		text-align: center;
		text-wrap: balance;
	}

	.rules-list__content {
		display: flex;
		flex-direction: column;
	}

	.rules-section {
		display: flex;
		flex-direction: column;
	}

	.rules-list__editor-content {
		display: flex;
		flex-direction: column;
		padding: 12px;
		padding-bottom: 16px;
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
		padding: 12px;
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

	.rule {
		display: flex;
		position: relative;
		align-items: center;
		padding: 10px;
		overflow: hidden;
		gap: 4px;
		border-bottom: 1px solid var(--clr-border-3);

		&:last-child {
			border-bottom: none;
		}
	}

	.rule-skeleton {
		display: flex;
		padding: 8px;
		gap: 4px;
		border-bottom: 1px solid var(--clr-border-3);

		&:last-child {
			border-bottom: none;
		}
	}
</style>
