<script lang="ts">
	import { Project } from '$lib/backend/projects';
	import { projectDeleteBranchesOnMergeWarningDismissed } from '$lib/config/config';
	import { getForgeRepoService } from '$lib/forge/interface/forgeRepoService';
	import InfoMessage from '$lib/shared/InfoMessage.svelte';
	import { openExternalUrl } from '$lib/utils/url';
	import { VirtualBranch } from '$lib/vbranches/types';
	import { getContext, getContextStore } from '@gitbutler/shared/context';
	import LinkButton from '@gitbutler/ui/LinkButton.svelte';

	const GITHUB_DELETE_ON_MERGE_DOCS_URL =
		'https://docs.github.com/en/repositories/configuring-branches-and-merges-in-your-repository/configuring-pull-request-merges/managing-the-automatic-deletion-of-branches';
	const BRANCH_AUTO_DELETE_DOCS =
		'https://docs.gitbutler.com/features/stacked-branches#github-configuration-for-stacked-prs';

	const branchStore = getContextStore(VirtualBranch);
	const repoService = getForgeRepoService();
	const project = getContext(Project);

	const deleteBranchesOnMergeWarningDismissed = projectDeleteBranchesOnMergeWarningDismissed(
		project.id
	);

	const repoInfoStore = $derived($repoService?.info);
	const repoInfo = $derived(repoInfoStore && $repoInfoStore);

	const branch = $derived($branchStore);
	const unarchivedSeries = $derived(branch.validSeries.filter((s) => !s.archived));
	const hasMultipleSeries = $derived(unarchivedSeries.length > 1);

	const shouldDisplayDeleteOnMergeWarning = $derived(
		!$deleteBranchesOnMergeWarningDismissed &&
			repoInfo?.deleteBranchAfterMerge === false &&
			hasMultipleSeries
	);

	function acknowledgeWarning() {
		$deleteBranchesOnMergeWarningDismissed = true;
	}

	function clickOnGitHubLink() {
		openExternalUrl(GITHUB_DELETE_ON_MERGE_DOCS_URL);
	}
	function clickOnDocsLink() {
		openExternalUrl(BRANCH_AUTO_DELETE_DOCS);
	}
</script>

{#if shouldDisplayDeleteOnMergeWarning}
	<div class="container">
		<InfoMessage
			style="neutral"
			filled
			outlined={false}
			primary="Configure GitHub"
			primaryIcon="open-link"
			on:primary={clickOnGitHubLink}
			secondary="Ok, don't show this again"
			on:secondary={acknowledgeWarning}
		>
			<svelte:fragment slot="title">Branch deletion after merge</svelte:fragment>
			<svelte:fragment slot="content">
				After a stacked PR has been merged, its branch should be deleted in order for the next PR to
				be updated to point to your target branch (<LinkButton onclick={clickOnDocsLink}>
					{#snippet children()}
						docs
					{/snippet}
				</LinkButton>).
				<br />
				You can <LinkButton onclick={clickOnGitHubLink}>
					{#snippet children()}
						configure GitHub
					{/snippet}
				</LinkButton> to do this automatically or alternatively delete the branch after merging.
			</svelte:fragment>
		</InfoMessage>
	</div>
{/if}

<style>
	.container {
		padding: 0 16px 16px;
	}
</style>
