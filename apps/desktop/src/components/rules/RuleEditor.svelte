<script lang="ts">
	import NewRuleMenu from "$components/rules/NewRuleMenu.svelte";
	import RuleFiltersEditor from "$components/rules/RuleFiltersEditor.svelte";
	import ReduxResult from "$components/shared/ReduxResult.svelte";
	import {
		type WorkspaceRuleId,
		type RuleFilterMap,
		type RuleFilterType,
		type StackTarget,
		encodeStackTarget,
		decodeStackTarget,
		compareStackTarget,
	} from "$lib/rules/rule";
	import { RULES_SERVICE } from "$lib/rules/rulesService.svelte";
	import { getStackName } from "$lib/stacks/stack";
	import { STACK_SERVICE } from "$lib/stacks/stackService.svelte";
	import { typedKeys } from "$lib/utils/object";
	import { inject } from "@gitbutler/core/context";
	import { Button, chipToasts, Icon, Select, SelectItem, Spacer } from "@gitbutler/ui";
	import { isDefined } from "@gitbutler/ui/utils/typeguards";

	type Props = {
		projectId: string;
		ruleId?: WorkspaceRuleId;
		initialStackTarget?: StackTarget;
		initialFilterValues?: Partial<RuleFilterMap>;
		onsave: () => void;
		oncancel: () => void;
	};

	const {
		projectId,
		ruleId,
		initialStackTarget,
		initialFilterValues = {},
		onsave,
		oncancel,
	}: Props = $props();

	const rulesService = inject(RULES_SERVICE);
	const stackService = inject(STACK_SERVICE);

	const [create, creatingRule] = rulesService.createWorkspaceRule;
	const [update, updatingRule] = rulesService.updateWorkspaceRule;

	let stackTargetSelected = $state<StackTarget | undefined>(initialStackTarget);
	let draftRuleFilterInitialValues = $state<Partial<RuleFilterMap>>(initialFilterValues);
	const ruleFilterTypes = $derived(typedKeys(draftRuleFilterInitialValues));
	const encodedStackTarget = $derived(
		stackTargetSelected && encodeStackTarget(stackTargetSelected),
	);

	let ruleFiltersEditor = $state<RuleFiltersEditor>();
	let addFilterButton = $state<HTMLDivElement>();
	let newFilterContextMenu = $state<NewRuleMenu>();

	const validFilters = $derived(!ruleFiltersEditor || ruleFiltersEditor.imports.filtersValid);
	const canSaveRule = $derived(stackTargetSelected !== undefined && validFilters);

	const stackEntries = $derived(stackService.stacks(projectId));

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

	async function saveRule() {
		const ruleFilters = ruleFiltersEditor ? ruleFiltersEditor.getRuleFilters() : [];

		if (ruleFilters === undefined) {
			chipToasts.error("Invalid rule filters");
			return;
		}

		if (!stackTargetSelected) {
			chipToasts.error("Please select a branch to assign the rule");
			return;
		}

		if (ruleId) {
			await update({
				projectId,
				request: {
					id: ruleId,
					enabled: null,
					action: {
						type: "explicit",
						subject: {
							type: "assign",
							subject: { target: stackTargetSelected },
						},
					},
					filters: ruleFilters,
					trigger: null,
				},
			});
		} else {
			await create({
				projectId,
				request: {
					action: {
						type: "explicit",
						subject: {
							type: "assign",
							subject: { target: stackTargetSelected },
						},
					},
					filters: ruleFilters,
					trigger: "fileSytemChange",
				},
			});
		}

		onsave();
	}
</script>

<div class="rule-editor">
	<div class="rule-editor__action">
		<h3 class="text-13 text-semibold">Assign to branch</h3>
		<ReduxResult {projectId} result={stackEntries.result}>
			{#snippet children(stacks)}
				{@const stackOptions = [
					...stacks
						.map((stack) =>
							stack.id
								? {
										label: getStackName(stack),
										value: encodeStackTarget({ type: "stackId", subject: stack.id }),
									}
								: undefined,
						)
						.filter(isDefined),
					{ separator: true } as const,
					{
						label: "Leftmost lane",
						value: encodeStackTarget({ type: "leftmost" }),
					},
					{
						label: "Rightmost lane",
						value: encodeStackTarget({ type: "rightmost" }),
					},
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
						? stackTargetSelected.type === "leftmost"
							? "leftmost-lane"
							: stackTargetSelected.type === "rightmost"
								? "rightmost-lane"
								: "branch"
						: "branch"}
				>
					{#snippet itemSnippet({ item, highlighted })}
						<SelectItem
							selected={item.value ? compareStackTarget(item.value, stackTargetSelected) : false}
							{highlighted}
						>
							{item.label || ""}
							{#snippet iconSnippet()}
								{#if item.value}
									{@const target = decodeStackTarget(item.value)}
									{#if target.type === "leftmost"}
										<Icon name="leftmost-lane" />
									{:else if target.type === "rightmost"}
										<Icon name="rightmost-lane" />
									{:else}
										<Icon name="branch" />
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
		<div class="rule-editor__filters">
			<RuleFiltersEditor
				bind:this={ruleFiltersEditor}
				{projectId}
				initialFilterValues={draftRuleFilterInitialValues}
				addFilter={addDraftRuleFilter}
				deleteFilter={removeDraftRuleFilter}
			/>
		</div>
	{:else}
		<div class="rule-editor__matches-all">
			<p class="text-13">Matches all changes</p>
			<div bind:this={addFilterButton} class="rule-editor__add-filter-button text-12">
				<button type="button" onclick={openAddFilterContextMenu}>
					<span class="clr-text-1 underline-dotted"> Add filter +</span>
				</button>
			</div>
		</div>
	{/if}

	<Spacer margin={2} dotted />

	<div class="rule-editor__buttons">
		<Button onclick={oncancel} kind="outline">Cancel</Button>
		<Button
			onclick={saveRule}
			kind="solid"
			wide
			style="gray"
			disabled={!canSaveRule}
			loading={stackEntries.result.isLoading ||
				creatingRule.current.isLoading ||
				updatingRule.current.isLoading}
		>
			Save rule
		</Button>
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

<style lang="postcss">
	.rule-editor {
		display: flex;
		flex-direction: column;
		padding: 12px;
		padding-bottom: 16px;
		gap: 16px;
		border-bottom: 1px solid var(--clr-border-2);
	}

	.rule-editor__action {
		display: flex;
		flex-direction: column;
		gap: 10px;
	}

	.rule-editor__filters {
		display: flex;
		flex-direction: column;
	}

	.rule-editor__matches-all {
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

	.rule-editor__add-filter-button {
		color: var(--clr-text-2);
	}

	.rule-editor__buttons {
		display: flex;
		justify-content: flex-end;
		gap: 6px;
	}
</style>
