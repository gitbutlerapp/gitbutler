<script lang="ts">
	import type { Branch } from '@gitbutler/shared/branches/types';
	import type { PatchCommit } from '@gitbutler/shared/patches/types';

	type Props = {
		branch: Branch;
	};

	const { branch }: Props = $props();

	const patchCommits = branch.patches;

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
</script>

<div class="commit-graph-wrap">
	<p class="text-12 fact">{branch.stackSize}</p>
	<div class="commits">
		{#each patchCommits as patch}
			<div class={['commit-block', getClass(patch)]}></div>
		{/each}
	</div>
</div>

<style lang="postcss">
	.commit-graph-wrap {
		display: flex;
		gap: 8px;
		align-items: center;
		color: var(--clr-text-2);
		width: -webkit-fill-available;

		& .fact {
			color: var(--clr-text-2);
			min-width: 10px;
			text-align: right;
		}

		& .fact:empty {
			display: none;
		}

		& .commits {
			display: flex;
			gap: 1px;
			width: 100%;
			min-width: 50px;
		}

		& .commit-block {
			flex: 1;
			height: 12px;
			background-color: var(--clr-br-commit-unreviewed-bg);
		}

		& .changes-requested {
			background-color: var(--clr-br-commit-changes-requested-bg);
		}
		& .approved {
			background-color: var(--clr-br-commit-approved-bg);
		}
		& .in-discussion {
			background-color: var(--clr-br-commit-in-discussion-bg);
		}
	}
</style>
