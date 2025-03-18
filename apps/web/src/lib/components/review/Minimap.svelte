<script lang="ts">
	import ChangeStatus from '$lib/components/changes/ChangeStatus.svelte';
	import { UserService } from '$lib/user/userService';
	import { getBranchReview } from '@gitbutler/shared/branches/branchesPreview.svelte';
	import { getContext } from '@gitbutler/shared/context';
	import Loading from '@gitbutler/shared/network/Loading.svelte';
	import { isFound, map } from '@gitbutler/shared/network/loadable';
	import { reactive } from '@gitbutler/shared/reactiveUtils.svelte';
	import { WebRoutesService } from '@gitbutler/shared/routing/webRoutes.svelte';
	import CommitStatusBadge from '@gitbutler/ui/CommitStatusBadge.svelte';
	import type { PatchCommit } from '@gitbutler/shared/patches/types';
	import { goto } from '$app/navigation';

	type Props = {
		ownerSlug: string;
		projectSlug: string;
		branchUuid: string;
		horizontal?: boolean;
	};

	const { ownerSlug, projectSlug, branchUuid, horizontal = false }: Props = $props();

	const routes = getContext(WebRoutesService);
	const userService = getContext(UserService);
	const user = userService.user;

	let component = $state<HTMLElement>();

	const branch = $derived(getBranchReview(branchUuid, { element: component }));
	const patchCommits = $derived(map(branch.current, (branch) => branch.patches) || []);

	function getClass(patchCommit: PatchCommit) {
		if (
			patchCommit.commentCount > 0 &&
			patchCommit.reviewAll.signedOff.length === 0 &&
			patchCommit.reviewAll.rejected.length === 0
		) {
			return 'in-discussion';
		}

		if (patchCommit.reviewAll.rejected.length > 0) {
			return 'changes-requested';
		}
		if (patchCommit.reviewAll.signedOff.length > 0) {
			return 'approved';
		}
	}

	function isPageSubject(changeId: string) {
		const current = $derived(routes.isProjectReviewBranchCommitPageSubset?.changeId === changeId);
		return reactive(() => current);
	}

	function visitPatch(patchCommit: PatchCommit) {
		if (!isFound(branch.current)) return;

		goto(
			routes.projectReviewBranchCommitPath({
				ownerSlug,
				projectSlug,
				branchId: branch.current.value.branchId,
				changeId: patchCommit.changeId
			})
		);
	}
</script>

{#snippet infoCard(patchCommit: PatchCommit)}
	{@const iRejected = patchCommit.reviewAll.rejected.find((entry) => entry.id === $user?.id)}
	{@const iAccepted = patchCommit.reviewAll.signedOff.find((entry) => entry.id === $user?.id)}
	{@const myReview = patchCommit.contributors.some(
		(contributor) => contributor.email === $user?.email || contributor.user?.id === $user?.id
	)}
	<div class="info-card">
		<div class="info-section">
			<div class="section-header">
				<ChangeStatus {patchCommit} />
				<p class="text-11">Change: {patchCommit.changeId.slice(0, 7)}</p>
			</div>
			<p class="text-13 text-semibold no-wrap">{patchCommit.title}</p>
		</div>
		{#if !myReview}
			<div class="info-section bottom">
				<div class="section-header">
					<p class="text-11">Your status:</p>
				</div>
				<CommitStatusBadge
					status={iAccepted ? 'approved' : iRejected ? 'changes-requested' : 'unreviewed'}
				/>
			</div>
		{/if}
	</div>
{/snippet}

<div bind:this={component} class="minimap" class:horizontal>
	<Loading loadable={branch.current}>
		{#snippet children(_)}
			{#each patchCommits ?? [] as patch}
				<!-- svelte-ignore a11y_no_static_element_interactions -->
				<!-- svelte-ignore a11y_click_events_have_key_events -->
				<div
					class={['erectangle', getClass(patch)]}
					class:is-subject={isPageSubject(patch.changeId).current}
					onclick={() => visitPatch(patch)}
				>
					{@render infoCard(patch)}
				</div>
			{/each}
		{/snippet}
	</Loading>
</div>

<style lang="postcss">
	.minimap {
		position: absolute;
		left: 0px;
		top: 100px;

		display: flex;
		flex-direction: column;

		&.horizontal {
			position: unset;
			width: 100%;

			flex-direction: row-reverse;

			.erectangle {
				width: 0px !important;
				flex-grow: 1;
			}

			.info-card {
				top: unset;
				bottom: -28px;

				left: 0px;
			}
		}
	}

	.erectangle {
		width: 10px;
		height: 16px;
		background-color: var(--clr-br-commit-unreviewed-bg);

		transition: width 0.5s;

		cursor: pointer;

		&:hover,
		&.is-subject {
			width: 30px;
		}

		&:hover {
			& .info-card {
				opacity: 1;
				display: flex;
			}
		}

		&.is-subject {
			cursor: default;
		}
	}

	.info-card {
		position: relative;
		z-index: var(--z-lifted);

		display: none;
		flex-direction: column;

		width: 208px;

		left: 35px;
		top: -20px;

		background-color: var(--clr-bg-1);
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-m);

		opacity: 0;

		transition: none;
		transition: opacity 0.25s ease-in-out;

		transition-behavior: allow-discrete;
		@starting-style {
			opacity: 0;
		}
	}

	.info-section {
		display: flex;
		flex-direction: column;

		gap: 8px;

		padding: 12px;

		&.bottom {
			border-top: 1px solid var(--clr-border-2);
		}
	}

	.section-header {
		display: flex;
		gap: 8px;

		align-items: center;

		color: var(--clr-text-2);
	}

	.no-wrap {
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}

	.changes-requested {
		background-color: var(--clr-br-commit-changes-requested-bg);
	}

	.approved {
		background-color: var(--clr-br-commit-approved-bg);
	}

	.in-discussion {
		background-color: var(--clr-br-commit-in-discussion-bg);
	}
</style>
