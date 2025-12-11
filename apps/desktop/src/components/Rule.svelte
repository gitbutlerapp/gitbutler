<script lang="ts">
	import ClaudeSessionDescriptor from '$components/ClaudeSessionDescriptor.svelte';
	import ReduxResult from '$components/ReduxResult.svelte';
	import {
		semanticTypeToString,
		treeStatusToShortString,
		type RuleFilter,
		type StackTarget,
		type WorkspaceRule
	} from '$lib/rules/rule';
	import { RULES_SERVICE } from '$lib/rules/rulesService.svelte';
	import { getStackName } from '$lib/stacks/stack';
	import { STACK_SERVICE } from '$lib/stacks/stackService.svelte';
	import { inject } from '@gitbutler/core/context';
	import {
		Button,
		FileStatusBadge,
		Icon,
		KebabButton,
		ContextMenuSection,
		ContextMenuItem,
		Modal,
		Tooltip
	} from '@gitbutler/ui';
	import type iconsJson from '@gitbutler/ui/data/icons.json';

	type Props = {
		projectId: string;
		rule: WorkspaceRule;
		editRule: () => void;
	};

	const { rule, projectId, editRule }: Props = $props();

	const stackService = inject(STACK_SERVICE);
	const rulesService = inject(RULES_SERVICE);

	const [deleteRule, deletingRule] = rulesService.deleteWorkspaceRule;

	let confirmationModal = $state<Modal>();

	async function handleDeleteRule() {
		await deleteRule({
			projectId,
			id: rule.id
		});
	}

	function getFilterConfig(filter: RuleFilter) {
		switch (filter.type) {
			case 'pathMatchesRegex':
				return {
					icon: 'folder' as keyof typeof iconsJson,
					label: filter.subject,
					tooltip: `Path: ${filter.subject}`
				};
			case 'contentMatchesRegex':
				return {
					icon: 'text-width' as keyof typeof iconsJson,
					label: filter.subject,
					tooltip: `Containing text: ${filter.subject}`
				};
			case 'fileChangeType':
				return {
					icon: null,
					label: treeStatusToShortString(filter.subject),
					tooltip: `File change type: ${treeStatusToShortString(filter.subject)}`
				};
			case 'semanticType':
				return {
					icon: 'tag' as keyof typeof iconsJson,
					label: semanticTypeToString(filter.subject.type),
					tooltip: `Semantic type: ${semanticTypeToString(filter.subject.type)}`
				};
			case 'claudeCodeSessionId':
				return {
					icon: 'ai-outline' as keyof typeof iconsJson,
					label: filter.subject,
					tooltip: `Claude session: ${filter.subject}`
				};
		}
	}
</script>

{#snippet stackPill(
	icon: keyof typeof iconsJson,
	label: string,
	tooltip: string,
	hasError?: boolean
)}
	<Tooltip text={tooltip}>
		<div class="target-pill" class:error={hasError}>
			<Icon name={icon} color={hasError ? 'error' : 'var(--clr-text-2)'} />
			<span class="text-12 truncate">{label}</span>
		</div>
	</Tooltip>
{/snippet}

{#snippet stackTarget(target: StackTarget)}
	{#if target.type === 'stackId'}
		{@const stackId = target.subject}
		{@const stack = stackService.stackById(projectId, stackId)}
		<ReduxResult {projectId} result={stack.result}>
			{#snippet children(stack)}
				{#if stack !== null}
					{@const stackName = getStackName(stack)}
					{@render stackPill('branch-remote', stackName, stackName)}
				{:else}
					{@render stackPill('error-small', 'branch missing', 'Associated stack not found', true)}
				{/if}
			{/snippet}
		</ReduxResult>
	{:else if target.type === 'leftmost'}
		{@render stackPill('leftmost-lane', 'Leftmost', 'Leftmost lane')}
	{:else if target.type === 'rightmost'}
		{@render stackPill('rightmost-lane', 'Rightmost', 'Rightmost stack')}
	{/if}
{/snippet}

{#snippet renderBasicPill(config: ReturnType<typeof getFilterConfig>)}
	<div class="filter-pill">
		<Tooltip text={config.tooltip}>
			<div class="flex items-center gap-6 overflow-hidden">
				{#if config.icon}
					<Icon name={config.icon} color="var(--clr-text-2)" />
				{/if}
				<span class="text-12 truncate">{config.label}</span>
			</div>
		</Tooltip>
	</div>
{/snippet}

{#snippet renderFileChangePill(config: ReturnType<typeof getFilterConfig>, fileStatus: any)}
	<Tooltip text={config.tooltip}>
		<div class="filter-pill">
			<FileStatusBadge status={fileStatus} style="dot" />
			<span class="text-12 truncate">{config.label}</span>
		</div>
	</Tooltip>
{/snippet}

{#snippet renderSessionPill(tooltip: string, icon: keyof typeof iconsJson, title: string)}
	<Tooltip text={tooltip}>
		<div class="ai-pill">
			<Icon name={icon} color="var(--clr-theme-purp-element)" />
			<span class="text-12 text-semibold truncate">{title}</span>
		</div>
	</Tooltip>
{/snippet}

{#snippet aiSessionPill(filter: RuleFilter)}
	{#if filter.type === 'claudeCodeSessionId'}
		{@const config = getFilterConfig(filter)}
		<ClaudeSessionDescriptor {projectId} sessionId={filter.subject}>
			{#snippet fallback()}
				{@render renderBasicPill(config)}
			{/snippet}
			{#snippet children(descriptor)}
				{@render renderSessionPill(`Claude session: ${descriptor}`, config.icon!, descriptor)}
			{/snippet}
		</ClaudeSessionDescriptor>
	{/if}
{/snippet}

{#snippet filterPill(filter: RuleFilter)}
	{@const config = getFilterConfig(filter)}

	{#if filter.type === 'claudeCodeSessionId'}
		{@render aiSessionPill(filter)}
	{:else if filter.type === 'fileChangeType'}
		{@render renderFileChangePill(config, filter.subject)}
	{:else}
		{@render renderBasicPill(config)}
	{/if}
{/snippet}

{#snippet ruleActions()}
	<div class="rule__actions">
		<KebabButton minimal buttonClassname="extra-padding">
			{#snippet contextMenu({ close })}
				<ContextMenuSection>
					<ContextMenuItem
						label="Edit rule"
						icon="edit"
						onclick={() => {
							close();
							editRule();
						}}
					/>
					<ContextMenuItem
						label="Delete rule"
						icon="bin"
						onclick={async () => {
							close();
							confirmationModal?.show();
						}}
					/>
				</ContextMenuSection>
			{/snippet}
		</KebabButton>
	</div>
{/snippet}

{#if rule.action.type === 'explicit' && rule.action.subject.type === 'assign' && (rule.trigger === 'fileSytemChange' || rule.trigger === 'claudeCodeHook')}
	{@const target = rule.action.subject.subject.target}
	{@const filters = rule.filters}

	<div class="rule" role="presentation" ondblclick={editRule}>
		<div class="flex items-center gap-4 flex-1 overflow-hidden m-r-4">
			{#if filters.length > 0}
				{#each filters as filter (filter.type)}
					{@render filterPill(filter)}
				{/each}
			{:else}
				<div class="filter-pill">
					<span class="text-12 truncate">*. All changes</span>
				</div>
			{/if}
			<Tooltip text="Assign to branch">
				<Icon name="arrow-right" color="var(--clr-text-3)" />
			</Tooltip>
			{@render stackTarget(target)}
		</div>

		{@render ruleActions()}
	</div>
{/if}

<Modal
	bind:this={confirmationModal}
	width="small"
	type="warning"
	title="Delete rule"
	onSubmit={async (close) => {
		await handleDeleteRule();
		close();
	}}
>
	Are you sure you want to delete this rule? This action cannot be undone.

	{#snippet controls(close)}
		<Button kind="outline" onclick={close}>Cancel</Button>
		<Button loading={deletingRule.current.isLoading} style="error" type="submit">Delete rule</Button
		>
	{/snippet}
</Modal>

<style lang="postcss">
	.rule {
		display: flex;
		position: relative;
		align-items: center;
		padding: 10px 8px 10px 10px;
		overflow: hidden;
		gap: 4px;
		border-bottom: 1px solid var(--clr-border-3);

		&:last-child {
			border-bottom: none;
		}
	}

	.rule__actions {
		display: flex;
		align-items: center;
	}

	:global(.rule__actions .extra-padding) {
		padding: 4px;
	}

	.filter-pill,
	.target-pill,
	.ai-pill {
		display: flex;
		align-items: center;
		height: 24px;
		padding: 0 6px;
		overflow: hidden;
		gap: 6px;
		border-radius: 100px;
	}

	.filter-pill {
		background-color: var(--clr-bg-2);
	}

	.target-pill {
		background-color: var(--clr-bg-2);
	}

	.ai-pill {
		background: var(--clr-theme-purp-soft);
		color: var(--clr-theme-purp-on-soft);
	}
</style>
