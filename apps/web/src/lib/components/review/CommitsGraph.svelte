<script lang="ts">
	import type { Branch, Patch } from '@gitbutler/shared/branches/types';

	type Props = {
		branch: Branch;
	};

	const { branch }: Props = $props();

	const patches = branch.patches;

	function getClass(patch: Patch) {
		if (patch.reviewAll.rejected.length > 0) {
			return 'changes-requested';
		}
		if (patch.reviewAll.signedOff.length > 0) {
			return 'approved';
		}

		return 'in-discussion';
	}
</script>

<div class="container">
	<p class="text-12 fact">{branch.stackSize}</p>
	<div class="commits">
		{#each patches as patch}
			<div class={['commit-block', getClass(patch)]}></div>
		{/each}
	</div>
</div>

<style lang="postcss">
	.container {
		display: flex;
		gap: 8px;
		align-items: center;
		color: var(--clr-text-2);
	}

	.fact {
		color: var(--clr-text-2);
		min-width: 10px;
		text-align: right;
	}

	.commits {
		display: flex;
		gap: 1px;
		width: 100%;
	}

	.commit-block {
		flex: 1;
		height: 12px;
		background-color: var(--clr-br-commit-unreviewed-bg);
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
