<script lang="ts">
	import ReduxResult from '$components/ReduxResult.svelte';
	import {
		semanticTypeToString,
		treeStatusToShortString,
		type RuleFilter,
		type WorkspaceRule
	} from '$lib/rules/rule';
	import { getStackName } from '$lib/stacks/stack';
	import { STACK_SERVICE } from '$lib/stacks/stackService.svelte';
	import { inject } from '@gitbutler/shared/context';
	import { Icon } from '@gitbutler/ui';

	type Props = {
		projectId: string;
		rule: WorkspaceRule;
	};

	const { rule, projectId }: Props = $props();

	const stackService = inject(STACK_SERVICE);
</script>

{#snippet stackTarget(stackId: string)}
	{@const stack = stackService.stackById(projectId, stackId)}
	<ReduxResult {projectId} result={stack.current}>
		{#snippet children(stack)}
			{@const stackName = getStackName(stack)}
			<div class="rule__pill">
				<Icon name="branch-in-square" opacity={0.6} />
				<span class="text-12 truncate" title={stackName}>{stackName}</span>
			</div>
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

{#if rule.action.type === 'explicit' && rule.action.subject.type === 'assign' && rule.trigger === 'fileSytemChange'}
	{@const stackId = rule.action.subject.subject.stack_id}
	{@const filters = rule.filters}

	<div class="rule">
		{#each filters as filter (filter.type)}
			{@render filterPill(filter)}
		{/each}
		{@render assignChip()}
		{@render stackTarget(stackId)}
	</div>
{:else}
	<!-- No support for this yet -->
	<span> '¯\_(ツ)_/¯' </span>
{/if}

<style lang="postcss">
	.rule {
		display: flex;
		padding: 12px;
		gap: 4px;

		&:not(:last-child) {
			border-bottom: 1px solid var(--clr-border-2);
		}
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
		gap: 6px;
		border: 1px solid var(--clr-btn-ntrl-outline);
		border-radius: 46px;
	}

	.rule__pill span {
		max-width: 100px;
		overflow: hidden;
		text-overflow: ellipsis;
	}
</style>
