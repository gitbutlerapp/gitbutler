<script lang="ts">
	// This is always displayed in the context of not having a cooresponding vbranch or remote
	import { Project } from '$lib/backend/projects';
	import Button from '$lib/components/Button.svelte';
	import Link from '$lib/components/Link.svelte';
	import Modal from '$lib/components/Modal.svelte';
	import TextBox from '$lib/components/TextBox.svelte';
	import { RemotesService } from '$lib/remotes/service';
	import { getContext } from '$lib/utils/context';
	import * as toasts from '$lib/utils/toasts';
	import { BaseBranchService } from '$lib/vbranches/baseBranch';
	import { BranchController } from '$lib/vbranches/branchController';
	import { VirtualBranchService } from '$lib/vbranches/virtualBranch';
	import { marked } from 'marked';
	import type { PullRequest } from '$lib/github/types';
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

	function closeModal() {
		remoteName = structuredClone(pullrequest.repoName) || '';
		createRemoteModal?.close();
	}

	async function createRemoteAndBranch() {
		if (!pullrequest.sshUrl) return;

		const remotes = await remotesService.remotes(project.id);
		if (remotes.includes(remoteName)) {
			toasts.error('Remote already exists');
			return;
		}

		loading = true;

		try {
			await remotesService.addRemote(project.id, remoteName, pullrequest.sshUrl);
			await baseBranchService.fetchFromRemotes();
			await branchController.createvBranchFromBranch(
				`refs/remotes/${remoteName}/${pullrequest.targetBranch}`
			);
			await virtualBranchService.reload();
			const vbranch = await virtualBranchService.getByUpstreamSha(pullrequest.sha);

			// This is a little absurd, but it makes it soundly typed
			if (!vbranch) {
				goto(`/${project.id}/board`);
				return;
			}

			// Active seems to be a more reliable metric to determine whether to go to the branch page
			if (vbranch.active) {
				goto(`/${project.id}/board`);
			} else {
				goto(`/${project.id}/stashed/${vbranch.id}`);
			}

			createRemoteModal?.close();
		} finally {
			loading = false;
		}
	}
</script>

<Modal width="small" bind:this={createRemoteModal}>
	<p class="text-base-15 fork-notice">
		In order to apply a branch from a fork, GitButler must first add a remote.
	</p>
	<TextBox label="Choose a remote name" bind:value={remoteName}></TextBox>
	<svelte:fragment slot="controls">
		<Button style="ghost" kind="solid" on:click={closeModal}>Cancel</Button>
		<Button style="pop" kind="solid" grow on:click={createRemoteAndBranch} {loading}>Confirm</Button
		>
	</svelte:fragment>
</Modal>

<div class="wrapper">
	<div class="card">
		<div class="card__header text-base-body-14 text-semibold">
			<h2 class="text-base-14 text-semibold">
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
				<Button size="tag" clickable={false} style="success" kind="solid" icon="pr-small"
					>Open</Button
				>
			{/if}
		</div>

		<div class="card__content">
			<div class="text-base-13">
				<span class="text-bold">
					{pullrequest.author?.name}
				</span>
				wants to merge into
				<span class="code-string">
					{pullrequest.sourceBranch}
				</span>
				from
				<span class="code-string">
					{pullrequest.targetBranch}
				</span>
			</div>
			{#if pullrequest.body}
				<div class="markdown">
					{@html marked.parse(pullrequest.body)}
				</div>
			{/if}
		</div>
		<div class="card__footer">
			{#if !pullrequest.repoName && !pullrequest.sshUrl}
				<p>Cannot apply pull request due to insufficient information</p>
			{:else}
				<Button
					style="pop"
					kind="solid"
					help="Does not create a commit. Can be toggled."
					on:click={async () => createRemoteModal?.show()}>Apply from fork</Button
				>
			{/if}
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
