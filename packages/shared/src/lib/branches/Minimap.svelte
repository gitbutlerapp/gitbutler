<script lang="ts">
	import { goto } from '$app/navigation';
	import ChangeStatus from '$lib/patches/ChangeStatus.svelte';
	import { WEB_ROUTES_SERVICE } from '$lib/routing/webRoutes.svelte';
	import { getBranchReview } from '@gitbutler/shared/branches/branchesPreview.svelte';
	import { isFound, map } from '@gitbutler/shared/network/loadable';
	import { getPatch } from '@gitbutler/shared/patches/patchCommitsPreview.svelte';
	import { reactive } from '@gitbutler/shared/reactiveUtils.svelte';
	import { inject } from '@gitbutler/core/context';
	import { CommitStatusBadge } from '@gitbutler/ui';
	import { getExternalLinkService } from '@gitbutler/ui/utils/externalLinkService';
	import { isDefined } from '@gitbutler/ui/utils/typeguards';
	import type { PatchCommit } from '@gitbutler/shared/patches/types';

	type Props = {
		ownerSlug: string;
		projectSlug: string;
		branchUuid: string;
		horizontal?: boolean;
		user: { email: string; id: number };
		openExternally?: boolean;
	};

	const {
		ownerSlug,
		projectSlug,
		branchUuid,
		horizontal = false,
		user,
		openExternally = false
	}: Props = $props();

	const routes = inject(WEB_ROUTES_SERVICE);
	const externalLinkService = getExternalLinkService();

	const branch = $derived.by(() => getBranchReview(branchUuid));
	const loadablePatchCommits = $derived(
		map(branch.current, (branch) => branch.patchCommitIds.map((id) => getPatch(branch.uuid, id))) ||
			[]
	);
	const patchCommits = $derived(
		loadablePatchCommits
			.map((patchCommit) => {
				if (isFound(patchCommit.current)) {
					return patchCommit.current.value;
				}
			})
			.filter(isDefined)
	);

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

		if (openExternally) {
			externalLinkService.open(
				routes.projectReviewBranchCommitUrl({
					ownerSlug,
					projectSlug,
					branchId: branch.current.value.branchId,
					changeId: patchCommit.changeId
				})
			);
		} else {
			goto(
				routes.projectReviewBranchCommitPath({
					ownerSlug,
					projectSlug,
					branchId: branch.current.value.branchId,
					changeId: patchCommit.changeId
				})
			);
		}
	}
</script>

{#snippet infoCard(patchCommit: PatchCommit)}
	{@const iRejected = patchCommit.reviewAll.rejected.find((entry) => entry.id === user.id)}
	{@const iAccepted = patchCommit.reviewAll.signedOff.find((entry) => entry.id === user.id)}
	{@const myReview = patchCommit.contributors.some(
		(contributor) => contributor.email === user.email || contributor.user?.id === user.id
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

<div class="minimap" class:horizontal class:no-stretch={patchCommits.length <= 5}>
	{#each patchCommits ?? [] as patch}
		<div class="erectangle-hover-area">
			<div
				role="presentation"
				class={['erectangle', getClass(patch)]}
				class:is-subject={isPageSubject(patch.changeId).current}
				onclick={() => visitPatch(patch)}
			>
				{@render infoCard(patch)}
			</div>
		</div>
	{/each}
</div>

<style lang="postcss">
	.minimap {
		display: flex;
		gap: 1px;

		&.horizontal {
			position: relative;
			top: auto;
			flex-direction: row-reverse;
			justify-content: flex-end;
			width: 100%;

			& .erectangle-hover-area {
				flex: 1;
				width: auto;
				max-width: 12px;
			}

			& .erectangle {
				width: 100%;
				height: 12px;
				border-radius: var(--radius-s);
			}

			& .erectangle-hover-area:hover .info-card {
				display: flex;
				position: absolute;
				top: unset;
				top: 18px;
				left: 0;
			}

			&.no-stretch {
				width: auto;

				& .erectangle-hover-area {
					flex: none;
					width: 12px;
				}
			}
		}

		&:not(.horizontal) {
			z-index: var(--z-lifted);
			position: fixed;
			top: 100px;
			left: 0px;

			flex-direction: column;

			& .erectangle-hover-area:hover {
				z-index: var(--z-lifted);

				& .erectangle {
					width: 100%;
				}

				& .info-card {
					display: flex;
					position: absolute;
				}
			}
		}
	}

	.erectangle-hover-area {
		display: flex;
		position: relative;
		width: 30px;
	}

	.erectangle {
		width: 10px;
		height: 16px;
		background-color: var(--clr-core-ntrl-70);
		cursor: pointer;
		transition: width var(--transition-medium);

		&.is-subject {
			cursor: default;
		}
	}

	.info-card {
		display: none;
		z-index: var(--z-lifted);
		top: -20px;

		left: 35px;
		flex-direction: column;

		width: 208px;
		border: 1px solid var(--clr-border-2);
		border-radius: var(--radius-m);

		background-color: var(--clr-bg-1);

		transition: none;
	}

	.info-section {
		display: flex;
		flex-direction: column;

		padding: 12px;

		gap: 8px;

		&.bottom {
			border-top: 1px solid var(--clr-border-2);
		}
	}

	.section-header {
		display: flex;

		align-items: center;
		gap: 8px;

		color: var(--clr-text-2);
	}

	.no-wrap {
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
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
