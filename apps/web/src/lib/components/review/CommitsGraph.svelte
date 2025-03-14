<script lang="ts">
	import { getBranchReview } from '@gitbutler/shared/branches/branchesPreview.svelte';
	import Loading from '@gitbutler/shared/network/Loading.svelte';
	import { map } from '@gitbutler/shared/network/loadable';
	import type { PatchCommit } from '@gitbutler/shared/patches/types';

	type Props = {
		branchUuid: string;
	};

	const { branchUuid }: Props = $props();

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
</script>

<div bind:this={component} class="commit-graph-wrap">
	<Loading loadable={branch.current}>
		{#snippet children(branch)}
			<p class="text-12 fact">{branch.stackSize}</p>
			<div class="commits">
				{#each patchCommits ?? [] as patch}
					<div class={['commit-block', getClass(patch as PatchCommit)]}></div>
				{/each}
			</div>
		{/snippet}
	</Loading>
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
