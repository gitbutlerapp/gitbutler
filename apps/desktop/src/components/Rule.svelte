<script lang="ts">
	import ReduxResult from '$components/ReduxResult.svelte';
	import {
		semanticTypeToString,
		treeStatusToShortString,
		type RuleFilter,
		type WorkspaceRule
	} from '$lib/rules/rule';
	import { RULES_SERVICE } from '$lib/rules/rulesService.svelte';
	import { getStackName } from '$lib/stacks/stack';
	import { STACK_SERVICE } from '$lib/stacks/stackService.svelte';
	import { inject } from '@gitbutler/shared/context';
	import { Button, FileStatusBadge, Icon, Modal, Tooltip } from '@gitbutler/ui';

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
</script>

{#snippet stackTarget(stackId: string)}
	{@const stack = stackService.stackById(projectId, stackId)}
	<ReduxResult {projectId} result={stack.current}>
		{#snippet children(stack)}
			{#if stack !== null}
				{@const stackName = getStackName(stack)}
				<div class="rule__pill">
					<Icon name="branch-in-square" opacity={0.6} />
					<span class="text-12 truncate" title={stackName}>{stackName}</span>
				</div>
			{:else}
				<Tooltip text="Associated stack not found" position="top">
					<div class="rule__pill error">
						<Icon name="error-small" color="error" />
						<span class="text-12 truncate">branch missing</span>
					</div>
				</Tooltip>
			{/if}
		{/snippet}
	</ReduxResult>
{/snippet}

{#snippet filterPill(filter: RuleFilter)}
	<div class="rule__pill">
		{#if filter.type === 'pathMatchesRegex'}
			<Icon name="folder" opacity={0.6} />
			<span class="text-12 trucate" title={filter.subject}>{filter.subject}</span>
		{:else if filter.type === 'contentMatchesRegex'}
			<Icon name="text-width" opacity={0.6} />
			<span class="text-12 truncate" title={filter.subject}>{filter.subject}</span>
		{:else if filter.type === 'fileChangeType'}
			<FileStatusBadge status={filter.subject} style="dot" />
			<span class="text-12 truncate">{treeStatusToShortString(filter.subject)}</span>
		{:else if filter.type === 'semanticType'}
			<Icon name="tag" opacity={0.6} />
			<span class="text-12 truncate">{semanticTypeToString(filter.subject.type)}</span>
		{/if}
	</div>
{/snippet}

{#snippet assignChip()}
	<div class="rule__action-chip">
		<Icon name="arrow-right" />
	</div>
{/snippet}

{#snippet ruleActions()}
	<div class="rule__actions">
		<div class="rule__actions-buttons">
			<Button icon="edit-text" size="tag" kind="ghost" onclick={editRule} />
			<Button
				icon="remove-from-list"
				style="error"
				size="tag"
				kind="ghost"
				onclick={() => confirmationModal?.show()}
			/>
		</div>
	</div>
{/snippet}

{#if rule.action.type === 'explicit' && rule.action.subject.type === 'assign' && rule.trigger === 'fileSytemChange'}
	{@const stackId = rule.action.subject.subject.stack_id}
	{@const filters = rule.filters}

	<div class="rule">
		{#each filters as filter (filter.type)}
			{@render filterPill(filter)}
		{:else}
			<div class="rule__pill">
				<span class="text-12 trucate">All files matched</span>
			</div>
		{/each}
		{@render assignChip()}
		{@render stackTarget(stackId)}
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

	.rule__pill {
		display: flex;
		align-items: center;
		height: var(--size-tag);
		padding: 0 6px;
		gap: 6px;
		border: 1px solid var(--clr-border-2);
		border-radius: 100px;

		&.error {
			border: 1px solid var(--clr-theme-err-element);
		}
	}

	.rule__pill span {
		max-width: 100px;
		overflow: hidden;
		text-overflow: ellipsis;
	}
</style>
