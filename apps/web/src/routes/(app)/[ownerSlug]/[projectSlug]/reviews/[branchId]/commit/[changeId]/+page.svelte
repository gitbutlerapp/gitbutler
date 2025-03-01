<script lang="ts">
	import { ChatMinimize } from '$lib/chat/minimize.svelte';
	import ChatComponent from '$lib/components/ChatComponent.svelte';
	import Navigation from '$lib/components/Navigation.svelte';
	import ChangeActionButton from '$lib/components/review/ChangeActionButton.svelte';
	import ChangeNavigator from '$lib/components/review/ChangeNavigator.svelte';
	import ReviewInfo from '$lib/components/review/ReviewInfo.svelte';
	import ReviewSections from '$lib/components/review/ReviewSections.svelte';
	import DiffLineSelection from '$lib/diff/lineSelection.svelte';
	import { UserService } from '$lib/user/userService';
	import { BranchService } from '@gitbutler/shared/branches/branchService';
	import { getBranchReview } from '@gitbutler/shared/branches/branchesPreview.svelte';
	import { lookupLatestBranchUuid } from '@gitbutler/shared/branches/latestBranchLookup.svelte';
	import { LatestBranchLookupService } from '@gitbutler/shared/branches/latestBranchLookupService';
	import { PatchService } from '@gitbutler/shared/branches/patchService';
	import { getPatch, getPatchSections } from '@gitbutler/shared/branches/patchesPreview.svelte';
	import { getContext } from '@gitbutler/shared/context';
	import Loading from '@gitbutler/shared/network/Loading.svelte';
	import { combine, map } from '@gitbutler/shared/network/loadable';
	import { lookupProject } from '@gitbutler/shared/organizations/repositoryIdLookupPreview.svelte';
	import { RepositoryIdLookupService } from '@gitbutler/shared/organizations/repositoryIdLookupService';
	import { AppState } from '@gitbutler/shared/redux/store.svelte';
	import {
		WebRoutesService,
		type ProjectReviewCommitParameters
	} from '@gitbutler/shared/routing/webRoutes.svelte';
	import Button from '@gitbutler/ui/Button.svelte';
	import Markdown from '@gitbutler/ui/markdown/Markdown.svelte';
	import { goto } from '$app/navigation';

	const DESCRIPTION_PLACE_HOLDER = 'No description provided';

	interface Props {
		data: ProjectReviewCommitParameters;
	}

	let { data }: Props = $props();

	const repositoryIdLookupService = getContext(RepositoryIdLookupService);
	const latestBranchLookupService = getContext(LatestBranchLookupService);
	const branchService = getContext(BranchService);
	const patchService = getContext(PatchService);
	const appState = getContext(AppState);
	const routes = getContext(WebRoutesService);
	const userService = getContext(UserService);
	const user = $derived(userService.user);
	const chatMinimizer = new ChatMinimize();
	const diffLineSelection = new DiffLineSelection(chatMinimizer);

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

	const patchIds = $derived(map(branch?.current, (b) => b.patchIds));

	const patch = $derived(
		map(branchUuid?.current, (branchUuid) => {
			return getPatch(appState, patchService, branchUuid, data.changeId);
		})
	);

	const isPatchAuthor = $derived(
		map(patch?.current, (patch) => {
			return patch.contributors.some(
				(contributor) => contributor.user?.id !== undefined && contributor.user?.id === $user?.id
			);
		})
	);

	const patchSections = $derived(
		map(branchUuid?.current, (branchUuid) => {
			return getPatchSections(appState, patchService, branchUuid, data.changeId);
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
</script>

<svelte:window onkeydown={handleKeyDown} onscroll={handleScroll} />

<div class="review-page" class:column={chatMinimizer.value}>
	<Loading loadable={combine([patch?.current, repositoryId.current, branchUuid?.current])}>
		{#snippet children([patch, repositoryId, branchUuid])}
			<div class="review-main" class:expand={chatMinimizer.value}>
				<Navigation />

				<div
					class="review-main__header"
					bind:this={headerEl}
					bind:clientHeight={headerHeight}
					class:stucked={headerIsStuck}
					class:bottom-line={headerIsStuck && !metaSectionHidden}
				>
					<div class="review-main__title-wrapper">
						{#if headerIsStuck}
							<div class="scroll-to-top">
								<Button kind="outline" icon="arrow-top" onclick={scrollToTop} />
							</div>
						{/if}
						<h3 class="text-18 text-bold review-main-title">{patch.title}</h3>
					</div>

					<!-- {#if !headerIsStuck}
						<div class="review-main__patch-navigator">
							{#if patchIds !== undefined}
								<ChangeNavigator {goToPatch} currentPatchId={patch.changeId} {patchIds} />
							{/if}

							{#if branchUuid !== undefined && isPatchAuthor === false}
								<ChangeActionButton {branchUuid} {patch} isUserLoggedIn={!!$user} />
							{/if}
						</div>
					{/if} -->
				</div>

				<div class="review-main__patch-navigator">
					{#if patchIds !== undefined}
						<ChangeNavigator {goToPatch} currentPatchId={patch.changeId} {patchIds} />
					{/if}

					{#if branchUuid !== undefined && isPatchAuthor === false}
						<ChangeActionButton {branchUuid} {patch} isUserLoggedIn={!!$user} />
					{/if}
				</div>

				<div class="review-main__meta" bind:this={metaSectionEl}>
					<p class="review-main-description">
						<Markdown content={patch.description?.trim() || DESCRIPTION_PLACE_HOLDER} />
					</p>
					<ReviewInfo projectId={repositoryId} {patch} />
				</div>

				<ReviewSections
					{patch}
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
				<div class="review-chat" class:minimized={chatMinimizer.value}>
					<ChatComponent
						{isPatchAuthor}
						isUserLoggedIn={!!$user}
						{branchUuid}
						messageUuid={data.messageUuid}
						projectId={repositoryId}
						branchId={data.branchId}
						changeId={data.changeId}
						minimized={chatMinimizer.value}
						toggleMinimized={() => chatMinimizer.toggle()}
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
		display: flex;
		width: 100%;
		flex-grow: 1;
		gap: 20px;

		&.column {
			flex-direction: column;
		}

		@media (--tablet-viewport) {
			flex-direction: column;
		}
	}

	.review-main {
		display: flex;
		flex-direction: column;
		width: 100%;
		max-width: 50%;

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

	.review-main__title-wrapper {
		display: flex;
		align-items: center;
		gap: 16px;
	}

	.review-main-title {
		color: var(--clr-text-1);
	}

	.review-main__patch-navigator {
		display: flex;
		gap: 6px;
		padding-bottom: 24px;

		@media (--tablet-viewport) {
			flex-wrap: wrap;
			gap: 12px;
		}
	}

	.review-main__meta {
		display: flex;
		flex-direction: column;
		gap: 24px;
		margin-bottom: 40px;
	}

	.review-main-description {
		color: var(--text-1);
		font-family: var(--fontfamily-mono);
		font-size: 12px;
		font-style: normal;
		line-height: 160%;
	}

	.review-chat {
		--top-nav-offset: 0;
		--bottom-margin: 44px;

		display: flex;
		position: sticky;
		top: 24px;
		width: 100%;
		height: calc(100vh - var(--bottom-margin));
		/* background-color: rgb(139, 81, 81); */

		&.minimized {
			height: fit-content;
			position: sticky;
			top: unset;
			bottom: var(--top-nav-offset);
			z-index: var(--z-floating);

			justify-content: flex-end;
			align-items: center;
			box-shadow: var(--fx-shadow-s);
		}

		@media (--tablet-viewport) {
			height: 50vh;
			position: sticky;
			top: unset;
			bottom: var(--bottom-margin);
			z-index: var(--z-floating);
			box-shadow: var(--fx-shadow-s);
		}
	}
</style>
