<script lang="ts">
	import { goto } from '$app/navigation';
	import ReduxResult from '$components/ReduxResult.svelte';
	import BranchCard from '$components/v3/BranchCard.svelte';
	import BaseBranchService from '$lib/baseBranch/baseBranchService.svelte';
	import { DefaultForgeFactory } from '$lib/forge/forgeFactory.svelte';
	import { showError } from '$lib/notifications/toasts';
	import { RemotesService } from '$lib/remotes/remotesService';
	import { workspacePath } from '$lib/routes/routes.svelte';
	import { pushStatusToColor } from '$lib/stacks/stack';
	import { StackService } from '$lib/stacks/stackService.svelte';
	import { UiState } from '$lib/state/uiState.svelte';
	import { TestId } from '$lib/testing/testIds';
	import { getContext } from '@gitbutler/shared/context';
	import Button from '@gitbutler/ui/Button.svelte';
	import Modal from '@gitbutler/ui/Modal.svelte';
	import Textbox from '@gitbutler/ui/Textbox.svelte';
	import { getColorFromBranchType } from '@gitbutler/ui/utils/getColorFromBranchType';
	import type { DetailedPullRequest } from '$lib/forge/interface/types';

	type Props = {
		projectId: string;
		prNumber: number;
		onerror?: (error: unknown) => void;
	};

	const { projectId, prNumber, onerror }: Props = $props();

	const forge = getContext(DefaultForgeFactory);
	const prService = $derived(forge.current.prService);
	const prResult = $derived(prService?.get(prNumber, { forceRefetch: true }));

	const uiState = getContext(UiState);
	const projectState = $derived(uiState.project(projectId));
	const branchesState = $derived(projectState.branchesSelection);

	const selected = $derived(branchesState.current.prNumber === prNumber);

	const baseBranchService = getContext(BaseBranchService);
	const baseRepoResponse = $derived(baseBranchService.repo(projectId));
	const baseRepo = $derived(baseRepoResponse.current.data);

	const remotesService = getContext(RemotesService);
	const stackService = getContext(StackService);

	let createRemoteModal = $state<Modal>();
	let inputRemoteName = $state<string>();
	let loading = $state(false);

	function getRemoteUrl(pr: DetailedPullRequest) {
		if (!baseRepo) return;

		if (baseRepo.protocol?.startsWith('http')) {
			return pr.repositoryHttpsUrl;
		} else {
			return pr.repositorySshUrl;
		}
	}

	async function handleConfirmRemote(pr: DetailedPullRequest) {
		const remoteUrl = getRemoteUrl(pr);

		if (!remoteUrl) {
			throw new Error(`Remote url not available for pr #${prNumber}.`);
		}

		if (!inputRemoteName) {
			showError('Cannot create a remote', 'Please provide a remote name.');
			return;
		}

		loading = true;

		try {
			const remoteRef = 'refs/remotes/' + inputRemoteName + '/' + pr.sourceBranch;
			await remotesService.addRemote(projectId, inputRemoteName, remoteUrl);
			await baseBranchService.fetchFromRemotes(projectId);
			await stackService.createVirtualBranchFromBranch({
				projectId,
				branch: remoteRef,
				remote: remoteRef,
				prNumber
			});

			goto(workspacePath(projectId));
		} catch (err: unknown) {
			showError('Failed to apply forked branch', err);
		} finally {
			loading = false;
		}
	}

	export function applyPr() {
		createRemoteModal?.show();
	}
</script>

<ReduxResult result={prResult?.current} {projectId} {onerror}>
	{#snippet children(pr, { projectId })}
		<BranchCard
			type="pr-branch"
			{selected}
			branchName={pr.sourceBranch}
			{projectId}
			readonly
			active
			trackingBranch={pr.sourceBranch}
			lastUpdatedAt={new Date(pr.updatedAt).getTime()}
			lineColor={getColorFromBranchType(pushStatusToColor('nothingToPush'))}
		/>

		<Modal
			testId={TestId.BranchesView_CreateRemoteModal}
			title="Apply Pull Request"
			width="small"
			bind:this={createRemoteModal}
			onSubmit={async () => await handleConfirmRemote(pr)}
		>
			<p class="text-15 fork-notice">
				In order to apply a branch from a fork, GitButler must first add a remote.
			</p>
			<Textbox label="Choose a remote name" bind:value={inputRemoteName} required />
			{#snippet controls(close)}
				<Button kind="outline" onclick={close}>Cancel</Button>
				<Button
					testId={TestId.BranchesView_CreateRemoteModalActionButton}
					style="pop"
					type="submit"
					{loading}
					disabled={!inputRemoteName}>Confirm</Button
				>
			{/snippet}
		</Modal>
	{/snippet}
</ReduxResult>

<style lang="postcss">
	.fork-notice {
		margin-bottom: 8px;
	}
</style>
