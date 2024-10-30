<script lang="ts">
	// This is always displayed in the context of not having a cooresponding vbranch or remote
	import { Project } from '$lib/backend/projects';
	import { BaseBranchService } from '$lib/baseBranch/baseBranchService';
	import Markdown from '$lib/components/Markdown.svelte';
	import { RemotesService } from '$lib/remotes/service';
	import Link from '$lib/shared/Link.svelte';
	import * as toasts from '$lib/utils/toasts';
	import { remoteUrlIsHttp } from '$lib/utils/url';
	import { BranchController } from '$lib/vbranches/branchController';
	import { VirtualBranchService } from '$lib/vbranches/virtualBranch';
	import { getContext } from '@gitbutler/shared/context';
	import Button from '@gitbutler/ui/Button.svelte';
	import Modal from '@gitbutler/ui/Modal.svelte';
	import Textbox from '@gitbutler/ui/Textbox.svelte';
	import { get } from 'svelte/store';
	import type { PullRequest } from '$lib/gitHost/interface/types';
	import { goto } from '$app/navigation';

	export let pullrequest: PullRequest;

	const branchController = getContext(BranchController);
	const project = getContext(Project);
	const remotesService = getContext(RemotesService);
	const baseBranchService = getContext(BaseBranchService);
	const virtualBranchService = getContext(VirtualBranchService);

	let remoteName = structuredClone(pullrequest.repoName) || '';
	let createRemoteModal: Modal | undefined;

	let loading = false;

	function closeModal(close: () => void) {
		remoteName = structuredClone(pullrequest.repoName) || '';
		close();
	}

	function getRemoteUrl() {
		const baseRemoteUrl = get(baseBranchService.base)?.remoteUrl;

		if (!baseRemoteUrl) return;

		if (remoteUrlIsHttp(baseRemoteUrl)) {
			return pullrequest.repositoryHttpsUrl;
		} else {
			return pullrequest.repositorySshUrl;
		}
	}

	async function createRemoteAndBranch() {
		const remoteUrl = getRemoteUrl();

		if (!remoteUrl) {
			toasts.error('Failed to get the remote URL');
			return;
		}

		const remotes = await remotesService.remotes(project.id);
		if (remotes.includes(remoteName)) {
			toasts.error('Remote already exists');
			return;
		}

		loading = true;

		try {
			await remotesService.addRemote(project.id, remoteName, remoteUrl);
			await baseBranchService.fetchFromRemotes();
			await branchController.createvBranchFromBranch(
				`refs/remotes/${remoteName}/${pullrequest.sourceBranch}`,
				undefined,
				{ type: 'GitHub', subject: { prNumber: pullrequest.number } }
			);
			await virtualBranchService.refresh();

			// This is a little absurd, but it makes it soundly typed
			goto(`/${project.id}/board`);

			createRemoteModal?.close();
		} finally {
			loading = false;
		}
	}
</script>

<Modal width="small" bind:this={createRemoteModal} onSubmit={createRemoteAndBranch}>
	<p class="text-15 fork-notice">
		In order to apply a branch from a fork, GitButler must first add a remote.
	</p>
	<Textbox label="Choose a remote name" bind:value={remoteName}></Textbox>
	{#snippet controls(close)}
		<Button style="ghost" outline onclick={() => closeModal(close)}>Cancel</Button>
		<Button style="pop" kind="solid" type="submit" grow {loading}>Confirm</Button>
	{/snippet}
</Modal>

<div class="wrapper">
	<div class="card">
		<div class="card__header text-14 text-body text-semibold">
			<h2 class="text-14 text-semibold">
				{pullrequest.title}
				<span class="card__title-pr">
					<Link target="_blank" rel="noreferrer" href={pullrequest.htmlUrl}>
						#{pullrequest.number}
					</Link>
				</span>
			</h2>
			{#if pullrequest.draft}
				<Button size="tag" clickable={false} style="neutral" icon="draft-pr-small">Draft</Button>
			{:else}
				<Button size="tag" clickable={false} style="success" kind="solid" icon="pr-small">
					Open
				</Button>
			{/if}
		</div>

		<div class="card__content">
			<div class="text-13">
				<span class="text-bold">
					{pullrequest.author?.name}
				</span>
				wants to merge into
				<span class="code-string">
					{pullrequest.targetBranch}
				</span>
				from
				<span class="code-string">
					{pullrequest.sourceBranch}
				</span>
			</div>
			{#if pullrequest.body}
				<Markdown content={pullrequest.body} />
			{/if}
		</div>
		<div class="card__footer">
			<Button
				style="pop"
				kind="solid"
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
