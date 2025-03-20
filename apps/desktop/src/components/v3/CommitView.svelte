<script lang="ts">
	import ConfigurableScrollableContainer from '$components/ConfigurableScrollableContainer.svelte';
	import ReduxResult from '$components/ReduxResult.svelte';
	import ChangedFiles from '$components/v3/ChangedFiles.svelte';
	import CommitDetails from '$components/v3/CommitDetails.svelte';
	import CommitHeader from '$components/v3/CommitHeader.svelte';
	import CommitMessageInput from '$components/v3/CommitMessageInput.svelte';
	import Drawer from '$components/v3/Drawer.svelte';
	import { StackService } from '$lib/stacks/stackService.svelte';
	import { inject } from '@gitbutler/shared/context';
	import type { CommitKey } from '$lib/commits/commit';

	type Props = {
		projectId: string;
		stackId: string;
		commitKey: CommitKey;
		onclick?: () => void;
	};

	const { projectId, stackId, commitKey, onclick }: Props = $props();

	const [stackService] = inject(StackService);
	const commitResult = $derived(
		commitKey.upstream
			? stackService.upstreamCommitById(projectId, commitKey)
			: stackService.commitById(projectId, commitKey)
	);
	const [updateCommitMessage, messageUpdateResult] = stackService.updateCommitMessage();

	type Mode = 'view' | 'edit';

	let mode = $state<Mode>('view');
	let commitMessageInput = $state<ReturnType<typeof CommitMessageInput>>();

	function setMode(newMode: Mode) {
		mode = newMode;
	}

	async function editCommitMessage() {
		if (!commitMessageInput) return;
		const title = commitMessageInput.getTitle();
		const message = await commitMessageInput.getPlaintext();
		if (!message && !title) return;

		const commitMessage = [title, message].filter((a) => a).join('\n\n');

		updateCommitMessage({
			projectId,
			stackId,
			commitId: commitKey.commitId,
			message: commitMessage
		});
		setMode('view');
	}

	function getCommitTitile(message: string): string | undefined {
		// Return undefined if there is no title
		return message.split('\n').slice(0, 1).join('\n') || undefined;
	}

	function getCommitDescription(message: string): string | undefined {
		// Return undefined if there is no description
		return message.split('\n').slice(1).join('\n') || undefined;
	}
</script>

<ReduxResult result={commitResult.current}>
	{#snippet children(commit)}
		{#if mode === 'edit'}
			<Drawer {projectId} {stackId}>
				{#snippet header()}
					<p class="text-14 text-semibold">Edit commit message</p>
				{/snippet}

				<CommitMessageInput
					bind:this={commitMessageInput}
					{projectId}
					action={editCommitMessage}
					actionLabel="Save"
					onCancel={() => setMode('view')}
					initialTitle={getCommitTitile(commit.message)}
					initialMessage={getCommitDescription(commit.message)}
					loading={messageUpdateResult.current.isLoading}
				/>
			</Drawer>
		{:else}
			<Drawer {projectId} {stackId}>
				{#snippet header()}
					<CommitHeader {commit} />
				{/snippet}
				<ConfigurableScrollableContainer>
					<div class="commit-view">
						<CommitDetails
							{projectId}
							{commit}
							{stackId}
							{onclick}
							onEditCommitMessage={() => setMode('edit')}
						/>
						<ChangedFiles type="commit" {projectId} commitId={commitKey.commitId} />
					</div>
				</ConfigurableScrollableContainer>
			</Drawer>
		{/if}
	{/snippet}
</ReduxResult>

<style>
	.commit-view {
		position: relative;
		min-height: 100%;
		flex: 1;
		display: flex;
		flex-direction: column;
		gap: 14px;
	}
</style>
