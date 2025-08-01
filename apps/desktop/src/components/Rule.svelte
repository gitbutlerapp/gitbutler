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
	import { Button, Icon, Modal, Tooltip } from '@gitbutler/ui';

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
			<Icon name="file-changes" opacity={0.6} />
			<span class="text-12 truncate">{treeStatusToShortString(filter.subject)}</span>
		{:else if filter.type === 'semanticType'}
			<Icon name="tag" opacity={0.6} />
			<span class="text-12 truncate">{semanticTypeToString(filter.subject.type)}</span>
		{/if}
	</div>
{/snippet}

{#snippet assignChip()}
	<div class="rule__chip">
		<Icon name="arrow-right" />
	</div>
{/snippet}

{#snippet ruleActions()}
	<div class="rule__actions">
		<div class="rule__actions-buttons">
			<Button icon="edit-text" size="tag" kind="ghost" onclick={editRule} />
			<Button icon="bin" size="tag" kind="ghost" onclick={() => confirmationModal?.show()} />
		</div>
	</div>
{/snippet}

{#if rule.action.type === 'explicit' && rule.action.subject.type === 'assign' && rule.trigger === 'fileSytemChange'}
	{@const stackId = rule.action.subject.subject.stack_id}
	{@const filters = rule.filters}

	<div class="rule">
		{#each filters as filter (filter.type)}
			{@render filterPill(filter)}
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
	<p class="text-12">Are you sure you want to delete this rule? This action cannot be undone.</p>

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
		padding: 12px;
		overflow: hidden;
		gap: 4px;

		&:not(:last-child) {
			border-bottom: 1px solid var(--clr-border-2);
		}

		&:hover .rule__actions {
			opacity: 1;
			pointer-events: auto;
		}
	}

	.rule__actions {
		display: flex;
		position: absolute;
		top: 9px;
		right: 9px;
		align-items: center;
		background-color: var(--clr-bg-1);
		box-shadow: 0 0 34px 30px var(--clr-bg-1);

		/* Initial state is hidden */
		opacity: 0;
		pointer-events: none;
		transition: opacity var(--transition-slow);
	}

	.rule__actions-buttons {
		display: flex;
		padding: 2px 4px;
		gap: 4px;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-ml);
	}

	.rule__chip {
		display: flex;
		align-items: center;
		justify-content: center;
		width: 24px;
		height: 24px;
		border-radius: 46px;
		background: var(--clr-bg-2);
	}

	.rule__pill {
		display: flex;
		align-items: center;
		padding: 4px 6px;
		gap: 4px;
		border: 1px solid var(--clr-btn-ntrl-outline);
		border-radius: 46px;

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
