<script lang="ts">
	import FeedActionDiff from '$components/FeedActionDiff.svelte';
	import FeedItemKind from '$components/FeedItemKind.svelte';
	import FeedStreamMessage from '$components/FeedStreamMessage.svelte';
	import {
		allCommitsUpdated,
		ButlerAction,
		getDisplayNameForWorkflowKind,
		isClaudeCodeActionSource,
		isDefinedMCPActionSource,
		isStringActionSource,
		isUndefinedMCPActionSource,
		Workflow
	} from '$lib/actions/types';
	import butbotSvg from '$lib/assets/butbot-actions.svg?raw';
	import { isFeedMessage, isInProgressAssistantMessage, type FeedEntry } from '$lib/feed/feed';
	import { Snapshot } from '$lib/history/types';
	import { USER } from '$lib/user/user';
	import { inject } from '@gitbutler/shared/context';
	import { AgentAvatar, EditorLogo, Markdown, TimeAgo, Tooltip } from '@gitbutler/ui';
	import { Icon, type IconName } from '@gitbutler/ui';
	type Props = {
		projectId: string;
		action: FeedEntry;
	};

	const { action, projectId }: Props = $props();

	const user = inject(USER);
	let failedToLoadImage = $state(false);

	function workflowTriggerTooltip(workflow: Workflow): string {
		switch (workflow.triggeredBy.type) {
			case 'manual':
				return 'Triggered by user action';
			case 'snapshot':
				return 'Triggered by the changes record';
			case 'unknown':
				return '🤷🏻‍♂️';
		}
	}

	function workflowTriggerIcon(workflow: Workflow): IconName {
		switch (workflow.triggeredBy.type) {
			case 'manual':
				return 'bowtie';
			case 'snapshot':
				return 'robot';
			case 'unknown':
				return 'question-mark';
		}
	}
</script>

<div class="action-item" id="action-{action.id}">
	{#if action instanceof ButlerAction}
		{#if isStringActionSource(action.source) || isUndefinedMCPActionSource(action.source)}
			<div>
				<div class="action-item__robot">
					<Icon name="robot" />
				</div>
			</div>
		{:else if isDefinedMCPActionSource(action.source)}
			<div class="action-item__editor-logo">
				<EditorLogo name={action.source.Mcp.name} />
				<div class="action-item__editor-source">
					{@html butbotSvg}
				</div>
			</div>
		{:else if isClaudeCodeActionSource(action.source)}
			<div class="action-item__editor-logo">
				<EditorLogo name="claude" />
				<div class="action-item__editor-source">
					{@html butbotSvg}
				</div>
			</div>
		{/if}
		<div class="action-item__content">
			<div class="action-item__content__header">
				{#if isStringActionSource(action.source)}
					<div>
						<p class="text-13 text-bold">Action</p>
						<p class="text-13 text-bold text-grey">{action.source}</p>
						<span class="text-13 text-greyer" title={new Date(action.createdAt).toLocaleString()}
							><TimeAgo date={new Date(action.createdAt)} addSuffix /></span
						>
					</div>
				{:else}
					<div>
						<p class="text-13 text-bold">Recorded changes</p>
						<span class="text-13 text-greyer" title={new Date(action.createdAt).toLocaleString()}>
							{#if isClaudeCodeActionSource(action.source)}
								Claude Hook
							{:else}
								MCP call
							{/if}

							<TimeAgo date={new Date(action.createdAt)} addSuffix /></span
						>
					</div>
				{/if}
			</div>
			<p class="action-item__prompt text-13">
				<span class="text-grey">Prompt:</span>{' ' +
					(action.externalPrompt ?? action.externalSummary)}
			</p>
			{#if !isStringActionSource(action.source) && !!action.response}
				{@const newCommits = allCommitsUpdated(action.response)}
				<FeedActionDiff {projectId} {newCommits} />
			{/if}
		</div>
	{:else if action instanceof Snapshot}
		<div class="action-item__picture">
			{#if $user?.picture && !failedToLoadImage}
				<img
					class="user-icon__image"
					src={$user.picture}
					alt=""
					referrerpolicy="no-referrer"
					onerror={() => (failedToLoadImage = true)}
				/>
			{:else}
				<Icon name="profile" />
			{/if}
		</div>
		<div class="action-item__content">
			<div class="action-item__content__header">
				<div>
					<p class="text-13 text-bold">{action.details?.operation}</p>
					<span class="text-13 text-greyer" title={new Date(action.createdAt).toLocaleString()}
						><TimeAgo date={new Date(action.createdAt)} addSuffix /></span
					>
				</div>
			</div>
			<span class="text-14 text-darkgrey">
				{#if action.details?.trailers}
					{#each action.details?.trailers as trailer}
						{trailer.key}
						{trailer.value}
					{/each}
				{/if}
				{#if action.details?.body}
					<Markdown content={action.details?.body} />
				{/if}
				{#each action.filesChanged as file}
					<span class="text-greyer">{file}</span>
				{/each}
			</span>
		</div>
	{:else if action instanceof Workflow}
		{@const iconName = workflowTriggerIcon(action)}
		{@const tooltip = workflowTriggerTooltip(action)}
		<div>
			<AgentAvatar />
		</div>
		<div class="action-item__content">
			<div class="action-item__content__header">
				<div>
					<p class="text-13 text-bold">Butler action</p>

					<Tooltip text={tooltip}>
						<div class="action-item__workflow-source">
							<Icon name={iconName} />
						</div>
					</Tooltip>
					<span class="text-13 text-greyer" title={new Date(action.createdAt).toLocaleString()}
						>But-agent <TimeAgo date={new Date(action.createdAt)} addSuffix /></span
					>
				</div>
			</div>

			<span class="text-13">
				{getDisplayNameForWorkflowKind(action.kind)}:
			</span>

			<FeedItemKind type="workflow" {projectId} kind={action.kind} />
		</div>
	{:else if isFeedMessage(action)}
		{#if action.type === 'assistant'}
			<div>
				<AgentAvatar />
			</div>
		{:else}
			<div class="action-item__picture">
				{#if $user?.picture && !failedToLoadImage}
					<img
						class="user-icon__image"
						src={$user.picture}
						alt=""
						referrerpolicy="no-referrer"
						onerror={() => (failedToLoadImage = true)}
					/>
				{:else}
					<Icon name="profile" />
				{/if}
			</div>
		{/if}
		<div class="action-item__content">
			{#if action.type === 'assistant'}
				<div class="action-item__content__header">
					<div>
						<p class="text-13 text-bold">Butler action</p>

						<Tooltip text="Triggered by chat message">
							<div class="action-item__chat-source">
								<Icon name="bowtie-small" />
							</div>
						</Tooltip>
					</div>
				</div>
				{#each action.toolCalls as toolCall}
					<FeedItemKind type="tool-call" {projectId} {toolCall} />
				{/each}
			{/if}
			<span class="text-14">
				<Markdown content={action.content} />
			</span>
		</div>
	{:else if isInProgressAssistantMessage(action)}
		<div>
			<AgentAvatar />
		</div>
		<div class="action-item__content">
			<div class="action-item__content__header">
				<div>
					<p class="text-13 text-bold">Butler action</p>

					<Tooltip text="Triggered by chat message">
						<div class="action-item__chat-source">
							<Icon name="bowtie" />
						</div>
					</Tooltip>
				</div>
			</div>
			<span class="text-14">
				<FeedStreamMessage {projectId} message={action} />
			</span>
		</div>
	{/if}
</div>

<style lang="postcss">
	.action-item__picture {
		display: flex;
		flex-shrink: 0;
		align-items: center;
		justify-content: center;
		width: 28px;
		height: 28px;
		overflow: hidden;
		border: 1px solid var(--clr-border-2);
		border-radius: 100%;
		background-color: var(--clr-bg-2);
	}

	.user-icon__image {
		width: 100%;
		height: 100%;
		object-fit: cover;
	}

	.action-item__robot {
		padding: 4px 6px;
		border: 1px solid var(--clr-border-2);

		border-radius: var(--radius-m);
		background-color: var(--clr-bg-2);
	}

	.action-item {
		box-sizing: border-box;
		display: flex;
		min-width: 0;
		padding: 16px 12px;
		gap: 14px;
		border-bottom: 1px solid var(--clr-border-3);
	}

	.action-item__content__header {
		display: flex;
		align-items: flex-start;

		> div:first-of-type {
			flex-grow: 1;
		}

		> div {
			display: flex;
			flex-wrap: wrap;

			align-items: center;
			gap: 8px;
		}
	}

	.action-item__content {
		display: flex;

		flex-direction: column;
		width: 100%;
		min-width: 0;
		gap: 8px;
	}

	.action-item__editor-logo {
		position: relative;
		height: fit-content;
	}

	.action-item__editor-source {
		position: absolute;
		right: -5px;
		bottom: -5px;

		width: 20px;
		height: 20px;
	}
	.action-item__workflow-source {
		display: flex;
		align-items: center;
		justify-content: center;
		color: var(--clr-text-2);
	}

	.action-item__chat-source {
		display: flex;
		align-items: center;
		justify-content: center;
		border-radius: 2px;
		background-color: var(--clr-bg-2);
		color: var(--clr-text-2);
	}

	.action-item__prompt {
		line-height: 160%;
	}

	.text-grey {
		color: var(--clr-text-2);
	}

	.text-darkgrey {
		color: var(--clr-core-ntrl-20);
		text-decoration-color: var(--clr-core-ntrl-20);
	}

	.text-greyer {
		color: var(--clr-text-3);
	}
</style>
