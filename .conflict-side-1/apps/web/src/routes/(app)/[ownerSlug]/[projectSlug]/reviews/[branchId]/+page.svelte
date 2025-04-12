<script lang="ts">
	import BranchCommitsTable from '$lib/components/changes/BranchCommitsTable.svelte';
	import PrivateProjectError from '$lib/components/errors/PrivateProjectError.svelte';
	import Factoid from '$lib/components/infoFlexRow/Factoid.svelte';
	import InfoFlexRow from '$lib/components/infoFlexRow/InfoFlexRow.svelte';
	import { updateFavIcon } from '$lib/utils/faviconUtils';
	import { UserService } from '$lib/user/userService';
	import BranchStatusBadge from '@gitbutler/shared/branches/BranchStatusBadge.svelte';
	import Minimap from '@gitbutler/shared/branches/Minimap.svelte';
	import { BranchService } from '@gitbutler/shared/branches/branchService';
	import { getBranchReview } from '@gitbutler/shared/branches/branchesPreview.svelte';
	import { lookupLatestBranchUuid } from '@gitbutler/shared/branches/latestBranchLookup.svelte';
	import { LatestBranchLookupService } from '@gitbutler/shared/branches/latestBranchLookupService';
	import { BranchStatus, type Branch } from '@gitbutler/shared/branches/types';
	import { copyToClipboard } from '@gitbutler/shared/clipboard';
	import { getContext } from '@gitbutler/shared/context';
	import { getContributorsWithAvatars } from '@gitbutler/shared/contributors';
	import Loading from '@gitbutler/shared/network/Loading.svelte';
	import { isFound, and, isError, map } from '@gitbutler/shared/network/loadable';
	import { AppState } from '@gitbutler/shared/redux/store.svelte';
	import {
		WebRoutesService,
		type ProjectReviewParameters
	} from '@gitbutler/shared/routing/webRoutes.svelte';
	import { UploadsService } from '@gitbutler/shared/uploads/uploadsService';
	import AsyncButton from '@gitbutler/ui/AsyncButton.svelte';
	import Button from '@gitbutler/ui/Button.svelte';
	import RichTextEditor from '@gitbutler/ui/RichTextEditor.svelte';
	import Textarea from '@gitbutler/ui/Textarea.svelte';
	import AvatarGroup from '@gitbutler/ui/avatar/AvatarGroup.svelte';
	import Link from '@gitbutler/ui/link/Link.svelte';
	import Markdown from '@gitbutler/ui/markdown/Markdown.svelte';
	import FileUploadPlugin, {
		type DropFileResult
	} from '@gitbutler/ui/richText/plugins/FileUpload.svelte';
	import toasts from '@gitbutler/ui/toasts';
	import dayjs from 'dayjs';
	import relativeTime from 'dayjs/plugin/relativeTime';
	import { goto } from '$app/navigation';

	const ACCEPTED_FILE_TYPES = ['image/*', 'application/*', 'text/*', 'audio/*', 'video/*'];

	dayjs.extend(relativeTime);

	interface Props {
		data: ProjectReviewParameters;
	}

	let { data }: Props = $props();

	const latestBranchLookupService = getContext(LatestBranchLookupService);
	const branchService = getContext(BranchService);
	const appState = getContext(AppState);
	const routes = getContext(WebRoutesService);
	const userService = getContext(UserService);
	const uploadsService = getContext(UploadsService);
	const user = $derived(userService.user);

	const branchUuid = $derived(
		lookupLatestBranchUuid(
			appState,
			latestBranchLookupService,
			data.ownerSlug,
			data.projectSlug,
			data.branchId
		)
	);

	const branch = $derived(
		map(branchUuid?.current, (branchUuid) => {
			return getBranchReview(branchUuid);
		})
	);

	const isBranchAuthor = $derived(
		map(branch?.current, (branch) => {
			return branch.contributors.some(
				(contributor) => contributor.user?.id !== undefined && contributor.user?.id === $user?.id
			);
		})
	);

	const contributors = $derived(
		isFound(branch?.current)
			? getContributorsWithAvatars(branch.current.value)
			: Promise.resolve([])
	);

	// Check if there's a 403 error in either branchUuid or branch
	function isForbiddenError(data: any) {
		if (!isError(data)) return false;

		const errorMessage = data.error.message || '';
		return (
			(data.error.name === 'ApiError' && errorMessage.includes('403')) ||
			errorMessage.includes('Forbidden') ||
			errorMessage.includes('Access denied') ||
			(typeof errorMessage === 'string' && errorMessage.includes('403'))
		);
	}

	// Check if there's a 403 error
	const hasForbiddenError = $derived(
		isForbiddenError(branchUuid?.current) || isForbiddenError(branch?.current)
	);

	// Check for any error in the combined loadable
	const combinedLoadable = $derived(and([branchUuid?.current, branch?.current]));
	const hasAnyError = $derived(isError(combinedLoadable));

	function visitFirstCommit(branch: Branch) {
		if ((branch.patchCommitIds?.length || 0) === 0) return;

		goto(
			routes.projectReviewBranchCommitPath({ ...data, changeId: branch.patchCommitIds.at(-1)! })
		);
	}

	let editingSummary = $state(false);
	let summary = $state('');
	let title = $state('');

	function editSummary() {
		if (!isFound(branch?.current)) return;
		// Make sure we're not dealing with a reference to the origional
		summary = structuredClone(branch.current.value.description || '');
		title = structuredClone(branch.current.value.title || '');
		editingSummary = true;
	}

	function abortEditingSummary() {
		if (!confirm('Canceling will lose any changes made')) {
			return;
		}

		editingSummary = false;
	}

	async function saveSummary() {
		if (!isFound(branch?.current)) return;

		try {
			await branchService.updateBranch(branch.current.value.uuid, {
				title: title,
				description: summary
			});
			toasts.success('Updated review status');
		} finally {
			editingSummary = false;
		}
	}

	async function updateStatus(status: BranchStatus.Active | BranchStatus.Closed) {
		if (!isFound(branch?.current)) return;

		await branchService.updateBranch(branch.current.value.uuid, {
			status
		});
		toasts.success('Saved review summary');
	}

	function copyLocation() {
		copyToClipboard(location.href);
	}

	function isAcceptedFileType(file: File): boolean {
		const type = file.type.split('/')[0];
		return ACCEPTED_FILE_TYPES.some((acceptedType) => acceptedType.startsWith(type));
	}

	async function handleDropFiles(files: FileList | undefined): Promise<DropFileResult[]> {
		if (files === undefined) return [];
		const uploads = Array.from(files)
			.filter(isAcceptedFileType)
			.map(async (file) => {
				const upload = await uploadsService.uploadFile(file);
				return { name: file.name, url: upload.url, isImage: upload.isImage };
			});
		const settled = await Promise.allSettled(uploads);
		const successful = settled.filter((result) => result.status === 'fulfilled');
		return successful.map((result) => result.value);
	}

	$effect(() => {
		if (isFound(branch?.current)) {
			updateFavIcon(branch.current.value?.reviewStatus);
		}
	});
</script>

{#snippet startReview(branch: Branch)}
	{#if (branch.stackSize || 0) > 0 && isBranchAuthor === false}
		<Button style="pop" icon="play" onclick={() => visitFirstCommit(branch)}>Start review</Button>
	{/if}
{/snippet}

<svelte:head>
	{#if isFound(branch?.current)}
		<title>{branch.current.value?.title}</title>
		<meta property="og:title" content="GitButler Review: {branch.current.value?.title}" />
		<meta property="og:description" content="GitButler code review" />
	{:else}
		<title>{data.ownerSlug}/{data.projectSlug}</title>
		<meta property="og:title" content="GitButler Review: {data.ownerSlug}/{data.projectSlug}" />
		<meta property="og:description" content="GitButler code review" />
	{/if}
</svelte:head>

{#if hasForbiddenError}
	<PrivateProjectError />
{:else if hasAnyError && combinedLoadable}
	{#if isForbiddenError(combinedLoadable)}
		<PrivateProjectError />
	{:else if isError(combinedLoadable)}
		<div class="error-container">
			<h2 class="text-15 text-body text-bold">Error loading project data</h2>
			<p class="text-13 text-body">{combinedLoadable.error.message}</p>
		</div>
	{/if}
{:else}
	<Loading loadable={combinedLoadable}>
		{#snippet children(branch)}
			<div class="layout">
				<div class="information">
					<div class="heading">
						{#if editingSummary}
							<Textarea bind:value={title}></Textarea>
						{:else}
							<p class="text-15 text-bold">{branch.title}</p>
						{/if}
						<div class="actions">
							<Button icon="copy-small" kind="outline" onclick={copyLocation}>Share link</Button>
							{@render startReview(branch)}
							{#if branch.status === BranchStatus.Closed}
								<AsyncButton action={async () => updateStatus(BranchStatus.Active)} kind="outline"
									>Re-open review</AsyncButton
								>
							{:else}
								<AsyncButton
									style="error"
									kind="outline"
									action={async () => updateStatus(BranchStatus.Closed)}>Close review</AsyncButton
								>
							{/if}
						</div>
					</div>
					<InfoFlexRow>
						<Factoid label="Status"><BranchStatusBadge {branch} /></Factoid>
						<Factoid label="Commits">
							{#if $user}
								<Minimap
									branchUuid={branch.uuid}
									ownerSlug={data.ownerSlug}
									projectSlug={data.projectSlug}
									horizontal
									user={$user}
								/>
							{/if}
						</Factoid>
						{#if branch.forgeUrl}
							<Factoid label="PR"
								><Link href={branch.forgeUrl}>{branch.forgeDescription || '#unknown'}</Link
								></Factoid
							>
						{/if}
						<Factoid label="Authors">
							{#await contributors then contributors}
								<AvatarGroup avatars={contributors}></AvatarGroup>
							{/await}
						</Factoid>
						<Factoid label="Updated">
							{dayjs(branch.updatedAt).fromNow()}
						</Factoid>
						<Factoid label="Version">
							{branch.version}
						</Factoid>
					</InfoFlexRow>
					<div class="summary">
						{#if editingSummary}
							<div class="summary-wrapper">
								<RichTextEditor
									namespace="review-description"
									markdown={false}
									onError={console.error}
									styleContext="chat-input"
									initialText={branch.description}
									onChange={(text) => (summary = text)}
								>
									{#snippet plugins()}
										<FileUploadPlugin onDrop={handleDropFiles} />
									{/snippet}
								</RichTextEditor>
							</div>

							<div class="summary-actions">
								<Button kind="outline" onclick={abortEditingSummary}>Cancel</Button>
								<AsyncButton style="pop" action={saveSummary}>Save</AsyncButton>
							</div>
						{:else if branch.description}
							<div class="text-13 summary-text">
								<Markdown content={branch.description} />
							</div>
							{#if branch.permissions.canWrite}
								<div>
									<Button kind="outline" onclick={editSummary}>Change details</Button>
								</div>
							{/if}
						{:else}
							<div class="summary-placeholder">
								<p class="text-13 text-clr2">No summary provided.</p>
								{#if branch.permissions.canWrite}
									<p class="text-12 text-body text-clr2">
										<em>
											Summaries provide context on the branch's purpose and helps team members
											understand it's changes.
										</em>
									</p>
									<Button icon="plus-small" kind="outline" onclick={editSummary}>Add summary</Button
									>
								{/if}
							</div>
						{/if}
					</div>
				</div>

				<BranchCommitsTable {branch} {data} />
			</div>
		{/snippet}
	</Loading>
{/if}

<style lang="postcss">
	.layout {
		display: grid;
		grid-template-columns: 6fr 10fr;
		gap: var(--layout-col-gap);

		@media (--desktop-small-viewport) {
			grid-template-columns: unset;
			display: flex;
			flex-direction: column;
		}
	}

	.information {
		display: flex;
		gap: 24px;
		flex-direction: column;
		padding-right: 20px;

		@media (--tablet-viewport) {
			padding-right: 0;
		}
	}

	.heading {
		display: flex;
		gap: 16px;
		flex-direction: column;
	}

	.summary {
		display: flex;
		flex-direction: column;
		gap: 16px;
	}

	.summary-text {
		line-height: 160%;
		border: 1px solid #ddd;
		border-radius: 9px;
		padding: 12px;
		background-color: #fff;
	}

	.summary-placeholder {
		display: flex;
		flex-direction: column;
		align-items: flex-start;
		gap: 12px;
	}

	.summary-wrapper {
		flex-shrink: 0;
		background-color: var(--clr-bg-1);
		padding: 6px;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-m);
	}
</style>
