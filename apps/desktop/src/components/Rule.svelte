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
	import { Button, FileStatusBadge, Icon, Modal, Tooltip } from '@gitbutler/ui';
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
					icon: 'ai-small' as keyof typeof iconsJson,
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
		<div class="rule__pill" class:error={hasError}>
			<Icon name={icon} color={hasError ? 'error' : 'var(--clr-text-2)'} />
			<span class="text-12 truncate">{label}</span>
		</div>
	</Tooltip>
{/snippet}

{#snippet stackTarget(target: StackTarget)}
	{#if target.type === 'stackId'}
		{@const stackId = target.subject}
		{@const stack = stackService.stackById(projectId, stackId)}
		<ReduxResult {projectId} result={stack.current}>
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
	<Tooltip text={config.tooltip}>
		<div class="rule__pill">
			{#if config.icon}
				<Icon name={config.icon} color="var(--clr-text-2)" />
			{/if}
			<span class="text-12 truncate">{config.label}</span>
		</div>
	</Tooltip>
{/snippet}

{#snippet renderFileChangePill(config: ReturnType<typeof getFilterConfig>, fileStatus: any)}
	<Tooltip text={config.tooltip}>
		<div class="rule__pill">
			<FileStatusBadge status={fileStatus} style="dot" />
			<span class="text-12 truncate">{config.label}</span>
		</div>
	</Tooltip>
{/snippet}

{#snippet renderSessionPill(tooltip: string, icon: keyof typeof iconsJson, title: string)}
	<Tooltip text={tooltip}>
		<div class="rule__ai-pill">
			<Icon name={icon} color="var(--clr-theme-purp-element)" />
			<span class="text-12 truncate">{title}</span>
		</div>
	</Tooltip>
{/snippet}

{#snippet filterPill(filter: RuleFilter)}
	{@const config = getFilterConfig(filter)}

	{#if filter.type === 'claudeCodeSessionId'}
		<ClaudeSessionDescriptor {projectId} sessionId={filter.subject}>
			{#snippet fallback()}
				{@render renderBasicPill(config)}
			{/snippet}
			{#snippet children(descriptor)}
				{@render renderSessionPill(`Claude session: ${descriptor}`, config.icon!, descriptor)}
			{/snippet}
		</ClaudeSessionDescriptor>
	{:else if filter.type === 'fileChangeType'}
		{@render renderFileChangePill(config, filter.subject)}
	{:else}
		{@render renderBasicPill(config)}
	{/if}
{/snippet}

{#snippet assignChip()}
	<Tooltip text="Assign to branch">
		<div class="rule__action-chip">
			<Icon name="arrow-right" />
		</div>
	</Tooltip>
{/snippet}

{#snippet ruleActions()}
	<div class="rule__actions">
		<div class="rule__actions-buttons">
			<Button icon="edit" size="tag" kind="ghost" onclick={editRule} tooltip="Edit rule" />
			<Button
				icon="bin"
				style="error"
				size="tag"
				kind="ghost"
				onclick={() => confirmationModal?.show()}
				tooltip="Delete rule"
			/>
		</div>
	</div>
{/snippet}

{#if rule.action.type === 'explicit' && rule.action.subject.type === 'assign' && (rule.trigger === 'fileSytemChange' || rule.trigger === 'claudeCodeHook')}
	{@const target = rule.action.subject.subject.target}
	{@const filters = rule.filters}

	<div class="rule">
		{#each filters as filter (filter.type)}
			{@render filterPill(filter)}
		{:else}
			<div class="rule__pill">
				<span class="text-12 truncate">*. All changes</span>
			</div>
		{/each}
		{@render assignChip()}
		{@render stackTarget(target)}
		{@render ruleActions()}
	</div>
{:else}
	<!-- No support for this yet -->
	<span> '¯\_(ツ)_/¯' </span>
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
		flex-wrap: wrap;
		padding: 12px 10px;
		overflow: hidden;
		gap: 4px;
		border-bottom: 1px solid var(--clr-border-2);

		&:hover .rule__actions {
			transform: scale(1);
			opacity: 1;
			pointer-events: auto;
		}
	}

	.rule__actions {
		display: flex;
		position: absolute;
		top: 8px;
		right: 8px;
		align-items: center;
		transform: scale(0.9);
		transform-origin: top right;
		background-color: var(--clr-bg-1);
		box-shadow: 0 0 30px 30px var(--clr-bg-1);
		opacity: 0;
		pointer-events: none;
		transition:
			opacity var(--transition-fast),
			transform var(--transition-medium);
	}

	.rule__actions-buttons {
		display: flex;
		padding: 3px;
		gap: 2px;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-ml);
	}

	.rule__action-chip {
		display: flex;
		align-items: center;
		justify-content: center;
		height: var(--size-tag);
		padding: 0 4px;
		border-radius: 100px;
		background: var(--clr-bg-2);
		color: var(--clr-text-2);
	}

	.rule__pill,
	.rule__ai-pill {
		display: flex;
		align-items: center;
		max-width: 130px;
		height: var(--size-tag);
		padding: 0 6px;
		overflow: hidden;
		gap: 5px;
		border-radius: 100px;
	}

	.rule__pill {
		border: 1px solid var(--clr-border-2);
		&.error {
			border: 1px solid var(--clr-theme-err-element);
		}
	}

	.rule__ai-pill {
		background: var(--clr-theme-purp-soft);
	}
</style>
