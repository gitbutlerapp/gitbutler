<script lang="ts">
	import { ChatMinimize } from '$lib/chat/minimize.svelte';
	import ChatComponent from '$lib/components/ChatComponent.svelte';
	import Navigation from '$lib/components/Navigation.svelte';
	import ChangeActionButton from '$lib/components/review/ChangeActionButton.svelte';
	import ChangeNavigator from '$lib/components/review/ChangeNavigator.svelte';
	import ReviewInfo from '$lib/components/review/ReviewInfo.svelte';
	import ReviewSections from '$lib/components/review/ReviewSections.svelte';
	import DiffLineSelection from '$lib/diff/lineSelection.svelte';
	import { updateFavIcon } from '$lib/utils/faviconUtils';
	import { UserService } from '$lib/user/userService';
	import { BranchService } from '@gitbutler/shared/branches/branchService';
	import { getBranchReview } from '@gitbutler/shared/branches/branchesPreview.svelte';
	import { lookupLatestBranchUuid } from '@gitbutler/shared/branches/latestBranchLookup.svelte';
	import { LatestBranchLookupService } from '@gitbutler/shared/branches/latestBranchLookupService';
	import { getContext } from '@gitbutler/shared/context';
	import Loading from '@gitbutler/shared/network/Loading.svelte';
	import { combine, isFound, map } from '@gitbutler/shared/network/loadable';
	import { lookupProject } from '@gitbutler/shared/organizations/repositoryIdLookupPreview.svelte';
	import { RepositoryIdLookupService } from '@gitbutler/shared/organizations/repositoryIdLookupService';
	import { PatchCommitService } from '@gitbutler/shared/patches/patchCommitService';
	import { getPatch, getPatchSections } from '@gitbutler/shared/patches/patchCommitsPreview.svelte';
	import { AppState } from '@gitbutler/shared/redux/store.svelte';
	import {
		WebRoutesService,
		type ProjectReviewCommitParameters
	} from '@gitbutler/shared/routing/webRoutes.svelte';
	import Button from '@gitbutler/ui/Button.svelte';
	import Markdown from '@gitbutler/ui/markdown/Markdown.svelte';
	import { goto } from '$app/navigation';

	const DESCRIPTION_PLACE_HOLDER = 'No commit message description provided';

	interface Props {
		data: ProjectReviewCommitParameters;
	}

	let { data }: Props = $props();

	const repositoryIdLookupService = getContext(RepositoryIdLookupService);
	const latestBranchLookupService = getContext(LatestBranchLookupService);
	const branchService = getContext(BranchService);
	const patchCommitService = getContext(PatchCommitService);
	const appState = getContext(AppState);
	const routes = getContext(WebRoutesService);
	const userService = getContext(UserService);
	const user = $derived(userService.user);
	const chatMinimizer = new ChatMinimize();
	const diffLineSelection = new DiffLineSelection(chatMinimizer);

	const chatTabletModeBreakpoint = 1024;
	let isChatTabletMode = $state(window.innerWidth < chatTabletModeBreakpoint);
	let isTabletModeEntered = $state(false);

	const repositoryId = $derived(
		lookupProject(appState, repositoryIdLookupService, data.ownerSlug, data.projectSlug)
	);

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
			return getBranchReview(appState, branchService, branchUuid);
		})
	);

	const patchCommitIds = $derived(map(branch?.current, (b) => b.patchCommitIds));

	const patchCommit = $derived(
		map(branchUuid?.current, (branchUuid) => {
			return getPatch(appState, patchCommitService, branchUuid, data.changeId);
		})
	);

	const isPatchAuthor = $derived(
		map(patchCommit?.current, (patch) => {
			return patch.contributors.some(
				(contributor) => contributor.user?.id !== undefined && contributor.user?.id === $user?.id
			);
		})
	);

	const patchSections = $derived(
		map(branchUuid?.current, (branchUuid) => {
			return getPatchSections(appState, patchCommitService, branchUuid, data.changeId);
		})
	);

	let headerEl = $state<HTMLDivElement>();
	let headerHeight = $state(0);
	let headerIsStuck = $state(false);
	let metaSectionHidden = $state(false);
	const HEADER_STUCK_THRESHOLD = 4;

	let metaSectionEl = $state<HTMLDivElement>();

	function handleScroll() {
		if (headerEl) {
			const top = headerEl.getBoundingClientRect().top;
			if (!headerIsStuck && top <= HEADER_STUCK_THRESHOLD) {
				headerIsStuck = true;
			}

			if (headerIsStuck && top > HEADER_STUCK_THRESHOLD) {
				headerIsStuck = false;
			}
		}

		if (metaSectionEl && headerEl) {
			metaSectionHidden =
				metaSectionEl.getBoundingClientRect().top -
					headerEl.clientHeight +
					metaSectionEl.clientHeight <
				0;
		}
	}

	function scrollToTop() {
		window.scrollTo({ top: 0, behavior: 'smooth' });
	}

	function goToPatch(changeId: string) {
		const url = routes.projectReviewBranchCommitPath({
			ownerSlug: data.ownerSlug,
			projectSlug: data.projectSlug,
			branchId: data.branchId,
			changeId
		});

		goto(url);
	}

	function handleKeyDown(event: KeyboardEvent) {
		if (chatMinimizer.isKeyboardShortcut(event)) {
			chatMinimizer.toggle();
			event.stopPropagation();
			event.preventDefault();
			return;
		}
	}

	function handleResize() {
		isChatTabletMode = window.innerWidth < chatTabletModeBreakpoint;
	}

	$effect(() => {
		if (isChatTabletMode && !chatMinimizer.value) {
			document.body.style.overflow = 'hidden';
		} else {
			document.body.style.overflow = '';
		}
	});

	$effect(() => {
		if (isChatTabletMode && !isTabletModeEntered) {
			isTabletModeEntered = true;
			chatMinimizer.minimize();
		} else if (!isChatTabletMode && isTabletModeEntered) {
			isTabletModeEntered = false;
			chatMinimizer.maximize();
		}
	});

	$effect(() => {
		if (isFound(patchCommit?.current)) {
			updateFavIcon(patchCommit.current.value?.reviewStatus);
		}
	});
</script>

<svelte:head>
	{#if isFound(patchCommit?.current)}
		<title>🔬{patchCommit.current.value?.title}</title>
		<meta property="og:title" content="Review: {patchCommit.current.value?.title}" />
		<meta property="og:description" content={patchCommit.current.value?.description} />
	{:else}
		<title>GitButler Review</title>
		<meta property="og:title" content="Butler Review: {data.ownerSlug}/{data.projectSlug}" />
		<meta property="og:description" content="GitButler code review" />
	{/if}
</svelte:head>

<svelte:window onkeydown={handleKeyDown} onscroll={handleScroll} onresize={handleResize} />

<div class="review-page" class:column={chatMinimizer.value}>
	<Loading
		loadable={combine([
			patchCommit?.current,
			repositoryId.current,
			branchUuid?.current,
			branch?.current
		])}
	>
		{#snippet children([patchCommit, repositoryId, branchUuid, branch])}
			<div class="review-main" class:expand={chatMinimizer.value}>
				<Navigation />

				<div
					class="review-main__header"
					bind:this={headerEl}
					bind:clientHeight={headerHeight}
					class:stucked={headerIsStuck}
					class:bottom-line={headerIsStuck && !metaSectionHidden}
				>
					<div class="review-main__title">
						{#if headerIsStuck}
							<div class="scroll-to-top">
								<Button kind="outline" icon="arrow-top" onclick={scrollToTop} />
							</div>
						{/if}
						<div class="review-main__title-wrapper">
							<p class="text-12 review-main__title-wrapper__branch">
								<span class="">Branch:</span>
								<a
									class="truncate"
									href={routes.projectReviewBranchPath({
										ownerSlug: data.ownerSlug,
										projectSlug: data.projectSlug,
										branchId: data.branchId
									})}>{branch.title}</a
								>
							</p>
							<h3 class="text-18 text-bold review-main-title">{patchCommit.title}</h3>
						</div>
					</div>
				</div>

				<div class="review-main__patch-navigator">
					{#if patchCommitIds !== undefined}
						<ChangeNavigator
							{goToPatch}
							currentPatchId={patchCommit.changeId}
							patchIds={patchCommitIds}
						/>
					{/if}

					{#if branchUuid !== undefined && isPatchAuthor === false}
						<ChangeActionButton {branchUuid} patch={patchCommit} isUserLoggedIn={!!$user} />
					{/if}
				</div>

				<div class="review-main__meta" bind:this={metaSectionEl}>
					<ReviewInfo projectId={repositoryId} patch={patchCommit} />
					<div class="review-main-description">
						<span class="text-12 review-main-description__caption">Commit message:</span>
						<p class="review-main-description__markdown">
							{#if patchCommit.description?.trim()}
								<Markdown content={patchCommit.description} />
							{:else}
								<span class="review-main-description__placeholder">
									{DESCRIPTION_PLACE_HOLDER}</span
								>
							{/if}
						</p>
					</div>
				</div>

				<ReviewSections
					patch={patchCommit}
					headerShift={headerHeight}
					patchSections={patchSections?.current}
					toggleDiffLine={(f, s, p) => diffLineSelection.toggle(f, s, p)}
					selectedSha={diffLineSelection.selectedSha}
					selectedLines={diffLineSelection.selectedLines}
					onCopySelection={(sections) => diffLineSelection.copy(sections)}
					onQuoteSelection={() => diffLineSelection.quote()}
					clearLineSelection={(fileName) => diffLineSelection.clear(fileName)}
				/>
			</div>

			{#if branchUuid !== undefined}
				<div
					class="review-chat"
					class:minimized={chatMinimizer.value}
					class:tablet-mode={isChatTabletMode}
				>
					<ChatComponent
						{isPatchAuthor}
						isUserLoggedIn={!!$user}
						{branchUuid}
						isTabletMode={isChatTabletMode}
						messageUuid={data.messageUuid}
						projectId={repositoryId}
						branchId={data.branchId}
						changeId={data.changeId}
						minimized={chatMinimizer.value}
						onMinimizeToggle={() => chatMinimizer.toggle()}
						diffSelection={diffLineSelection.diffSelection}
						clearDiffSelection={() => diffLineSelection.clear()}
					/>
				</div>
			{/if}
		{/snippet}
	</Loading>
</div>

<style lang="postcss">
	.review-page {
		display: grid;
		grid-template-columns: 9fr 7fr;
		gap: var(--layout-col-gap);
		width: 100%;
		flex-grow: 1;
		gap: 20px;

		&.column {
			display: flex;
			flex-direction: column;
		}

		@media (--tablet-viewport) {
			display: flex;
			flex-direction: column;
		}
	}

	.review-main {
		display: flex;
		flex-direction: column;
		flex-shrink: 0;
		container-type: inline-size;

		&.expand {
			max-width: 100%;
			flex-grow: 1;
		}

		@media (--tablet-viewport) {
			max-width: 100%;
		}
	}

	.review-main__header {
		z-index: var(--z-ground);
		position: sticky;
		top: 0;

		display: flex;
		flex-direction: column;
		gap: 12px;

		background-color: var(--clr-bg-2);
		margin-top: -24px;
		padding: 24px 0 12px;
		border-bottom: 1px solid transparent;

		transition:
			border-bottom var(--transition-medium),
			padding var(--transition-medium);

		&.bottom-line {
			border-bottom: 1px solid var(--clr-border-2);
		}

		&.stucked {
			padding: 16px 0;
		}
	}

	.scroll-to-top {
		display: flex;
		animation: fadeInScrollButton var(--transition-medium) forwards;
	}

	@keyframes fadeInScrollButton {
		from {
			opacity: 0;
			width: 0;
		}
		to {
			opacity: 1;
			min-width: var(--size-button);
		}
	}

	.review-main__title {
		display: flex;
		align-items: flex-end;
		gap: 16px;
	}

	.review-main__title-wrapper {
		display: flex;
		flex-direction: column;
		gap: 6px;
		overflow: hidden;
	}

	.review-main__title-wrapper__branch {
		display: flex;
		gap: 6px;

		& span {
			color: var(--clr-text-2);
			opacity: 0.8;
		}

		& a:hover {
			text-decoration: underline;
		}
	}

	.review-main-title {
		color: var(--clr-text-1);
	}

	.review-main__patch-navigator {
		display: flex;
		flex-wrap: wrap;
		gap: 16px 20px;
		padding-bottom: 24px;
	}

	.review-main__meta {
		display: flex;
		flex-direction: column;
		gap: 24px;
		margin-bottom: 10px;
	}

	.review-main-description {
		display: flex;
		flex-direction: column;
		gap: 8px;
		color: var(--text-1);
		font-size: 13px;
		font-style: normal;
		line-height: 180%;
		padding: 16px;
		background: var(--clr-bg-1);
		font-family: var(--fontfamily-default);
		border-radius: 10px;
		border: 1px solid var(--clr-border-2);
	}

	.review-main-description__placeholder {
		color: var(--clr-text-3);
		font-style: italic;
	}

	.review-main-description__caption {
		color: var(--clr-text-2);
	}

	.review-chat {
		--top-nav-offset: 0;
		--bottom-margin: 44px;

		display: flex;
		position: sticky;
		top: 24px;
		height: calc(100vh - var(--bottom-margin));

		&.minimized {
			height: fit-content;
			max-width: unset;
			position: sticky;
			top: unset;
			bottom: var(--top-nav-offset);
			z-index: var(--z-floating);

			justify-content: flex-end;
			align-items: center;
			box-shadow: var(--fx-shadow-s);
		}

		&.tablet-mode {
			z-index: var(--z-floating);
			position: fixed;
			max-width: unset;
			height: 100dvh;
			top: unset;
			left: 0;
			bottom: 0;
			pointer-events: none;
		}
	}
</style>
