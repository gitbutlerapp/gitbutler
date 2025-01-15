<script lang="ts">
	// This is always displayed in the context of not having a cooresponding vbranch or remote
	import Link from '$components/Link.svelte';
	import Markdown from '$components/Markdown.svelte';
	import { Project } from '$lib/backend/projects';
	import { BaseBranchService } from '$lib/baseBranch/baseBranchService';
	import { showError } from '$lib/notifications/toasts';
	import { RemotesService } from '$lib/remotes/remotesService';
	import { BranchController } from '$lib/vbranches/branchController';
	import { VirtualBranchService } from '$lib/vbranches/virtualBranch';
	import { getContext } from '@gitbutler/shared/context';
	import Badge from '@gitbutler/ui/Badge.svelte';
	import Button from '@gitbutler/ui/Button.svelte';
	import Modal from '@gitbutler/ui/Modal.svelte';
	import Textbox from '@gitbutler/ui/Textbox.svelte';
	import type { PullRequest } from '$lib/forge/interface/types';
	import { goto } from '$app/navigation';

	const { pr }: { pr: PullRequest } = $props();

	const branchController = getContext(BranchController);
	const project = getContext(Project);
	const remotesService = getContext(RemotesService);
	const baseBranchService = getContext(BaseBranchService);
	const virtualBranchService = getContext(VirtualBranchService);
	const baseRepo = $derived(baseBranchService.repo);

	let inputRemoteName = $state<string>(pr.repoOwner || '');

	let createRemoteModal: Modal | undefined;

	let loading = $state(false);

	function closeModal(close: () => void) {
		close();
	}

	function getRemoteUrl() {
		const repo = $baseRepo;
		if (!repo) return;

		if ($baseRepo?.protocol?.startsWith('http')) {
			return pr.repositoryHttpsUrl;
		} else {
			return pr.repositorySshUrl;
		}
	}

	async function handleConfirmRemote() {
		const remoteUrl = getRemoteUrl();

		if (!remoteUrl) {
			throw new Error(`Remote url not available for pr #${pr.number}.`);
		}

		loading = true;

		try {
			const remoteRef = 'refs/remotes/' + inputRemoteName + '/' + pr.sourceBranch;
			await remotesService.addRemote(project.id, inputRemoteName, remoteUrl);
			await baseBranchService.fetchFromRemotes();
			await branchController.createvBranchFromBranch(remoteRef, remoteRef, pr.number);
			await virtualBranchService.refresh();

			// This is a little absurd, but it makes it soundly typed
			goto(`/${project.id}/board`);

			createRemoteModal?.close();
		} catch (err: unknown) {
			showError('Failed to apply forked branch', err);
		} finally {
			loading = false;
		}
	}
</script>

<Modal width="small" bind:this={createRemoteModal} onSubmit={handleConfirmRemote}>
	<p class="text-15 fork-notice">
		In order to apply a branch from a fork, GitButler must first add a remote.
	</p>
	<Textbox label="Choose a remote name" bind:value={inputRemoteName} required />
	{#snippet controls(close)}
		<Button kind="outline" onclick={() => closeModal(close)}>Cancel</Button>
		<Button style="pop" type="submit" grow {loading}>Confirm</Button>
	{/snippet}
</Modal>

<div class="wrapper">
	<div class="card">
		<div class="card__header text-14 text-body text-semibold">
			<h2 class="text-14 text-semibold">
				{pr.title}
				<span class="card__title-pr">
					<Link target="_blank" rel="noreferrer" href={pr.htmlUrl}>
						#{pr.number}
					</Link>
				</span>
			</h2>
			{#if pr.draft}
				<Badge size="tag" style="neutral" icon="draft-pr-small">Draft</Badge>
			{:else}
				<Badge size="tag" style="success" icon="pr-small">Open</Badge>
			{/if}
		</div>

		<div class="card__content">
			<div class="text-13">
				<span class="text-bold">
					{pr.author?.name}
				</span>
				wants to merge into
				<span class="code-string">
					{pr.targetBranch}
				</span>
				from
				<span class="code-string">
					{pr.sourceBranch}
				</span>
			</div>
			{#if pr.body}
				<Markdown content={pr.body} />
			{/if}
		</div>
		<div class="card__footer">
			<Button
				style="pop"
				tooltip="Does not create a commit. Can be toggled."
				onclick={async () => createRemoteModal?.show()}>Apply from fork</Button
			>
		</div>
	</div>
</div>

<style lang="postcss">
	.wrapper {
		display: flex;
		flex-direction: column;
		gap: 16px;
		max-width: 896px;
	}
	.card__content {
		gap: 12px;
	}
	.card__title-pr {
		opacity: 0.4;
		margin-left: 4px;
	}

	.fork-notice {
		margin-bottom: 8px;
	}
</style>
