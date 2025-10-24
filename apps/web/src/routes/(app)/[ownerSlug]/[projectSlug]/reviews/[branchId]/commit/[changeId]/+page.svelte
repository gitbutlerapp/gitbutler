<script lang="ts">
	import { goto } from '$app/navigation';
	import { ChatMinimize } from '$lib/chat/minimize.svelte';
	import ChatComponent from '$lib/components/ChatComponent.svelte';
	import Navigation from '$lib/components/Navigation.svelte';
	import PrivateProjectError from '$lib/components/errors/PrivateProjectError.svelte';
	import ChangeActionButton from '$lib/components/review/ChangeActionButton.svelte';
	import ChangeNavigator from '$lib/components/review/ChangeNavigator.svelte';
	import ReviewInfo from '$lib/components/review/ReviewInfo.svelte';
	import ReviewSections from '$lib/components/review/ReviewSections.svelte';
	import DiffLineSelection from '$lib/diff/lineSelection.svelte';
	import { USER_SERVICE } from '$lib/user/userService';
	import { updateFavIcon } from '$lib/utils/faviconUtils';
	import { inject } from '@gitbutler/core/context';
	import Minimap from '@gitbutler/shared/branches/Minimap.svelte';
	import { getBranchReview } from '@gitbutler/shared/branches/branchesPreview.svelte';
	import { lookupLatestBranchUuid } from '@gitbutler/shared/branches/latestBranchLookup.svelte';
	import { LATEST_BRANCH_LOOKUP_SERVICE } from '@gitbutler/shared/branches/latestBranchLookupService';
	import Loading from '@gitbutler/shared/network/Loading.svelte';
	import { combine, isFound, map, isError } from '@gitbutler/shared/network/loadable';
	import { lookupProject } from '@gitbutler/shared/organizations/repositoryIdLookupPreview.svelte';
	import { getPatch } from '@gitbutler/shared/patches/patchCommitsPreview.svelte';
	import { APP_STATE } from '@gitbutler/shared/redux/store.svelte';
	import {
		WEB_ROUTES_SERVICE,
		type ProjectReviewCommitParameters
	} from '@gitbutler/shared/routing/webRoutes.svelte';
	import { Button, Markdown } from '@gitbutler/ui';

	const DESCRIPTION_PLACE_HOLDER = 'No commit message description provided';

	interface Props {
		data: ProjectReviewCommitParameters;
	}

	let { data }: Props = $props();

	const latestBranchLookupService = inject(LATEST_BRANCH_LOOKUP_SERVICE);
	const appState = inject(APP_STATE);
	const routes = inject(WEB_ROUTES_SERVICE);
	const userService = inject(USER_SERVICE);
	const user = $derived(userService.user);
	const chatMinimizer = new ChatMinimize();
	const diffLineSelection = new DiffLineSelection(chatMinimizer);

	const chatTabletModeBreakpoint = 1024;
	let isChatTabletMode = $state(window.innerWidth < chatTabletModeBreakpoint);
	let isTabletModeEntered = $state(false);
	let chatComponent = $state<ReturnType<typeof ChatComponent>>();

	const repositoryId = $derived(lookupProject(data.ownerSlug, data.projectSlug));

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

	const patchCommitIds = $derived(map(branch?.current, (b) => b.patchCommitIds));

	const patchCommit = $derived(
		map(branchUuid?.current, (branchUuid) => {
			return getPatch(branchUuid, data.changeId);
		})
	);

	const isPatchAuthor = $derived(
		map(patchCommit?.current, (patch) => {
			return patch.contributors.some(
				(contributor) => contributor.user?.id !== undefined && contributor.user?.id === $user?.id
			);
		})
	);

	let headerEl = $state<HTMLDivElement>();
	let headerHeight = $state(0);

	let headerIsStuck = $state(false);
	const HEADER_STUCK_THRESHOLD = 4;

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

	// Check for forbidden error in either the branchUuid lookup or the branch data
	const hasForbiddenError = $derived(
		isForbiddenError(patchCommit?.current) || isForbiddenError(branch?.current)
	);
	// Check for any error in the combined loadable
	const combinedLoadable = $derived(
		combine([patchCommit?.current, repositoryId.current, branchUuid?.current, branch?.current])
	);
	const hasAnyError = $derived(isError(combinedLoadable));
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

<svelte:window onscroll={handleScroll} onresize={handleResize} onkeydown={handleKeyDown} />

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
	<div class="review-page" class:column={chatMinimizer.value}>
		<Loading loadable={combinedLoadable}>
			{#snippet children([patchCommit, repositoryId, branchUuid, branch])}
				<div class="review-page__minimap">
					{#if $user}
						<Minimap
							{branchUuid}
							ownerSlug={data.ownerSlug}
							projectSlug={data.projectSlug}
							user={$user}
						/>
					{/if}
				</div>

				<div class="review-main" class:expand={chatMinimizer.value}>
					<Navigation />

					<div
						class="review-main__header"
						bind:this={headerEl}
						bind:clientHeight={headerHeight}
						class:bottom-line={headerIsStuck}
					>
						<div class="review-main__title">
							{#if headerIsStuck}
								<div class="scroll-to-top">
									<Button kind="outline" icon="arrow-up" onclick={scrollToTop} />
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

					<div class="review-main__meta">
						<ReviewInfo projectId={repositoryId} {patchCommit} />
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
						{branchUuid}
						{patchCommit}
						changeId={data.changeId}
						commitPageHeaderHeight={headerHeight}
						toggleDiffLine={(f, s, p) => diffLineSelection.toggle(f, s, p)}
						selectedSha={diffLineSelection.selectedSha}
						selectedLines={diffLineSelection.selectedLines}
						onCopySelection={(sections) => diffLineSelection.copy(sections)}
						onQuoteSelection={() => {
							diffLineSelection.quote();
							chatComponent?.focus();
						}}
						clearLineSelection={(fileName) => diffLineSelection.clear(fileName)}
					/>
				</div>

				{#if branchUuid !== undefined}
					<div
						id="chat-panel"
						class="review-chat"
						class:minimized={chatMinimizer.value}
						class:tablet-mode={isChatTabletMode}
					>
						<ChatComponent
							bind:this={chatComponent}
							{isPatchAuthor}
							isUserLoggedIn={!!$user}
							{branchUuid}
							{patchCommit}
							isTabletMode={isChatTabletMode}
							messageUuid={data.messageUuid}
							projectId={repositoryId}
							projectSlug={data.projectSlug}
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
{/if}

<style lang="postcss">
	.review-page {
		display: grid;
		grid-template-columns: 9fr 7fr;
		flex-grow: 1;
		width: 100%;
		gap: var(--layout-col-gap);
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

	.review-page__minimap {
		display: contents;

		@media (--mobile-viewport) {
			display: none;
		}
	}

	.error-container {
		display: flex;
		flex-direction: column;
		align-items: center;
		width: 100%;
		padding: 32px;
		gap: 12px;
	}

	.review-main {
		container-type: inline-size;
		display: flex;
		flex-shrink: 0;
		flex-direction: column;

		&.expand {
			flex-grow: 1;
			max-width: 100%;
		}

		@media (--tablet-viewport) {
			max-width: 100%;
		}
	}

	.review-main__header {
		display: flex;
		z-index: var(--z-ground);
		position: sticky;
		top: 0;
		flex-direction: column;
		margin-top: -14px;
		padding: 16px 0;
		gap: 12px;
		border-bottom: 1px solid transparent;

		background-color: var(--clr-bg-2);

		transition:
			border-bottom var(--transition-medium),
			padding var(--transition-medium);

		&.bottom-line {
			border-bottom: 1px solid var(--clr-border-2);
		}
	}

	.scroll-to-top {
		display: flex;
	}

	.review-main__title {
		display: flex;
		align-items: flex-end;
		gap: 16px;
	}

	.review-main__title-wrapper {
		display: flex;
		flex-direction: column;
		overflow: hidden;
		gap: 6px;
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
		padding-bottom: 24px;
		gap: 16px 20px;
	}

	.review-main__meta {
		display: flex;
		flex-direction: column;
		margin-bottom: 10px;
		gap: 24px;
	}

	.review-main-description {
		display: flex;
		flex-direction: column;
		padding: 16px;
		gap: 8px;
		border: 1px solid var(--clr-border-2);
		border-radius: 10px;
		background: var(--clr-bg-1);
		color: var(--text-1);
		font-style: normal;
		font-size: 13px;
		line-height: 180%;
		font-family: var(--font-default);
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
		height: calc(100dvh - var(--bottom-margin));

		&.minimized {
			z-index: var(--z-floating);
			position: sticky;
			top: unset;
			bottom: var(--top-nav-offset);
			align-items: center;

			justify-content: flex-end;
			max-width: unset;
			height: fit-content;
		}

		&.tablet-mode {
			display: flex;
			z-index: var(--z-floating);
			top: 0;
			bottom: var(--top-nav-offset);
			left: 0;
			align-items: end;
			justify-content: flex-end;
			width: 100%;
			max-width: unset;
			height: 100dvh;
			pointer-events: none;
		}
	}
</style>
