<script lang="ts">
	import Rule from "$components/rules/Rule.svelte";
	import RuleEditor from "$components/rules/RuleEditor.svelte";
	import Drawer from "$components/shared/Drawer.svelte";
	import ReduxResult from "$components/shared/ReduxResult.svelte";
	import {
		type WorkspaceRuleId,
		type RuleFilterMap,
		type StackTarget,
		type WorkspaceRule,
		type RuleFilter,
	} from "$lib/rules/rule";
	import { RULES_SERVICE, workspaceRulesSelectors } from "$lib/rules/rulesService.svelte";
	import { inject } from "@gitbutler/core/context";
	import { Badge, Button, chipToasts, Link, SkeletonBone } from "@gitbutler/ui";
	import { focusable } from "@gitbutler/ui/focus/focusable";
	import { slide } from "svelte/transition";

	type Props = {
		projectId: string;
	};

	const { projectId }: Props = $props();

	const rulesService = inject(RULES_SERVICE);

	// Visual state
	let mode = $state<"list" | "edit" | "add">("list");
	let editingRuleId = $state<WorkspaceRuleId | null>(null);

	// Initial values passed to RuleEditor when opening edit mode
	let editorInitialStackTarget = $state<StackTarget | undefined>(undefined);
	let editorInitialFilterValues = $state<Partial<RuleFilterMap>>({});

	// Read initial collapsed state from persisted storage, default to false (open) if not found
	const drawerPersistId = `rules-drawer-${projectId}`;
	let drawer = $state<Drawer>();

	const rules = $derived(rulesService.workspaceRules(projectId));

	function openRuleEditor() {
		if (editingRuleId !== null) {
			chipToasts.error("Please finish editing the current rule first");
			return;
		}
		// Open drawer if it's collapsed
		if (drawer?.getIsCollapsed()) {
			drawer.open();
		}

		mode = "add";
	}

	function resetEditor() {
		editorInitialStackTarget = undefined;
		editorInitialFilterValues = {};
		editingRuleId = null;
	}

	function closeEditor() {
		mode = "list";
		resetEditor();
	}

	function updateInitialValues(filter: RuleFilter, initialValues: Partial<RuleFilterMap>): true {
		switch (filter.type) {
			case "pathMatchesRegex":
				initialValues.pathMatchesRegex = filter.subject;
				return true;
			case "contentMatchesRegex":
				initialValues.contentMatchesRegex = filter.subject;
				return true;
			case "fileChangeType":
				initialValues.fileChangeType = filter.subject;
				return true;
			case "semanticType":
				initialValues.semanticType = filter.subject;
				return true;
			case "claudeCodeSessionId":
				initialValues.claudeCodeSessionId = filter.subject;
				return true;
		}
	}

	async function editExistingRule(rule: WorkspaceRule) {
		if (mode === "add" || (editingRuleId !== null && editingRuleId !== rule.id)) {
			chipToasts.error("Please finish editing the current rule first");
			return;
		}

		if (rule.action.type === "implicit") {
			chipToasts.error("Cannot edit implicit rules");
			return;
		}

		if (rule.action.subject.type !== "assign") {
			chipToasts.error("Cannot edit rules that are not branch assignments");
			return;
		}

		editingRuleId = rule.id;
		editorInitialStackTarget = rule.action.subject.subject.target;

		const initialValues: Partial<RuleFilterMap> = {};
		for (const filter of rule.filters) {
			updateInitialValues(filter, initialValues);
		}
		editorInitialFilterValues = initialValues;

		mode = "edit";
	}

	function separateRules(allRules: WorkspaceRule[]) {
		const regularRules: WorkspaceRule[] = [];
		const aiRules: WorkspaceRule[] = [];

		for (const rule of allRules) {
			const hasAiFilter = rule.filters.some(
				(filter: RuleFilter) => filter.type === "claudeCodeSessionId",
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

<Drawer
	bind:this={drawer}
	bottomBorder={false}
	persistId={drawerPersistId}
	maxHeight="60%"
	defaultCollapsed={true}
>
	{#snippet header()}
		<h4 class="text-14 text-semibold truncate">Rules</h4>
		{#if rules.result.isSuccess}
			<Badge>{rules.result.data.ids.length}</Badge>
		{:else}
			<Badge skeleton />
		{/if}
	{/snippet}
	{#snippet actions()}
		<Button onclick={openRuleEditor} icon="plus" size="tag" kind="ghost" />
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
				{#if mode === "add"}
					<div in:slide={{ duration: 150 }}>
						<RuleEditor {projectId} onsave={closeEditor} oncancel={closeEditor} />
					</div>
				{/if}

				<div class="rules-list__content">
					{#if regularRules.length > 0}
						{#each regularRules.slice().reverse() as rule (rule.id)}
							{#if editingRuleId === rule.id}
								<div in:slide={{ duration: 150 }}>
									<RuleEditor
										{projectId}
										ruleId={rule.id}
										initialStackTarget={editorInitialStackTarget}
										initialFilterValues={editorInitialFilterValues}
										onsave={closeEditor}
										oncancel={closeEditor}
									/>
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
										<RuleEditor
											{projectId}
											ruleId={rule.id}
											initialStackTarget={editorInitialStackTarget}
											initialFilterValues={editorInitialFilterValues}
											onsave={closeEditor}
											oncancel={closeEditor}
										/>
									</div>
								{:else}
									<Rule {projectId} {rule} editRule={() => editExistingRule(rule)} />
								{/if}
							{/each}
						</div>
					{/if}
				</div>
			{:else if mode === "add"}
				<div in:slide={{ duration: 170 }}>
					<RuleEditor {projectId} onsave={closeEditor} oncancel={closeEditor} />
				</div>
			{:else}
				<div class="rules-placeholder">
					<p class="text-13 text-body rules-placeholder-text">
						Let rules automatically sort your changes.
						<Link
							href="https://docs.gitbutler.com/features/branch-management/rules"
							class="underline-dotted clr-text-2"
						>
							Read the docs
						</Link> or set up your
						<button type="button" class="underline-dotted clr-text-2" onclick={openRuleEditor}>
							first rule
						</button> +
					</p>
				</div>
			{/if}
		{/snippet}
	</ReduxResult>
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
