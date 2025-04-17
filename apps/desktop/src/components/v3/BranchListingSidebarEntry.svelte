<script lang="ts">
	import SidebarEntry from '$components/v3/SidebarEntry.svelte';
	import { BranchListingDetails, type BranchListing } from '$lib/branches/branchListing';
	import { BranchService } from '$lib/branches/branchService.svelte';
	import { GitConfigService } from '$lib/config/gitConfigService';
	import { Project } from '$lib/project/project';
	import { stackPath } from '$lib/routes/routes.svelte';
	import { UiState } from '$lib/state/uiState.svelte';
	import { UserService } from '$lib/user/userService';
	import { inject } from '@gitbutler/shared/context';
	import { gravatarUrlFromEmail } from '@gitbutler/ui/avatar/gravatar';
	import type { PullRequest } from '$lib/forge/interface/types';
	import { page } from '$app/state';

	interface Props {
		projectId: string;
		branchListing: BranchListing;
		prs: PullRequest[];
	}

	const { projectId, branchListing, prs }: Props = $props();

	const unknownName = 'unknown';
	const unknownEmail = 'example@example.com';

	const [userService, gitConfigService, project, branchService, uiState] = inject(
		UserService,
		GitConfigService,
		Project,
		BranchService,
		UiState
	);

	const user = userService.user;

	// TODO: Use information from all PRs in a stack?
	const pr = $derived(prs.at(0));

	let hasBeenSeen = $state(false);

	const branchDetailsResult = $derived(
		hasBeenSeen ? branchService.get(projectId, branchListing.name) : undefined
	);

	let lastCommitDetails = $state<{ authorName: string; lastCommitAt?: Date }>();
	let branchListingDetails = $derived(branchDetailsResult?.current.data);

	// If there are zero commits we should not show the author
	const ownedByUser = $derived(branchListingDetails?.numberOfCommits === 0);

	function onMouseDown() {
		if (branchListing.stack?.inWorkspace) {
			stackPath(project.id, branchListing.stack.id);
		} else {
			if (branchListing.stack) {
				uiState.project(projectId).branchesSelection.set({ stackId: branchListing.stack.id });
			} else {
				uiState.project(projectId).branchesSelection.set({ branchName: branchListing.name });
			}
		}
	}

	const selected = $derived(page.url.pathname === formatBranchURL(project, branchListing.name));

	function formatBranchURL(project: Project, name: string) {
		return `/${project.id}/branch/${encodeURIComponent(name)}`;
	}

	$effect(() => {
		let canceled = false;

		if (ownedByUser) {
			gitConfigService.get('user.name').then((userName) => {
				if (canceled) return;

				if (userName) {
					lastCommitDetails = { authorName: userName };
				} else {
					lastCommitDetails = undefined;
				}
			});
		} else {
			lastCommitDetails = {
				authorName: branchListing.lastCommiter.name || unknownName,
				lastCommitAt: new Date(branchListing.updatedAt)
			};
		}
	});

	let avatars = $state<{ name: string; srcUrl: string }[]>([]);

	$effect(() => {
		setAvatars(ownedByUser, branchListingDetails);
	});

	async function setAvatars(ownedByUser: boolean, branchListingDetails?: BranchListingDetails) {
		if (ownedByUser) {
			const name = (await gitConfigService.get('user.name')) || unknownName;
			const email = (await gitConfigService.get('user.email')) || unknownEmail;
			const srcUrl =
				email.toLowerCase() === $user?.email?.toLowerCase() && $user?.picture
					? $user?.picture
					: await gravatarUrlFromEmail(email);

			avatars = [{ name, srcUrl: srcUrl }];
		} else if (branchListingDetails) {
			avatars = branchListingDetails.authors
				? await Promise.all(
						branchListingDetails.authors.map(async (author) => {
							return {
								name: author.name || unknownName,
								srcUrl:
									(author.email?.toLowerCase() === $user?.email?.toLowerCase()
										? $user?.picture
										: author.gravatarUrl) ??
									(await gravatarUrlFromEmail(author.email || unknownEmail))
							};
						})
					)
				: [];
		} else {
			avatars = [];
		}
	}

	const stackBranches = $derived(branchListing.stack?.branches);
	const filteredStackBranches = $derived(
		stackBranches && stackBranches.length > 0 ? stackBranches : [branchListing.name]
	);
</script>

<SidebarEntry
	series={filteredStackBranches}
	remotes={branchListing.remotes}
	local={branchListing.hasLocal}
	applied={branchListing.stack?.inWorkspace}
	{lastCommitDetails}
	pullRequestDetails={pr && {
		title: pr.title,
		draft: pr.draft
	}}
	branchDetails={branchListingDetails && {
		commitCount: branchListingDetails.numberOfCommits,
		linesAdded: branchListingDetails.linesAdded,
		linesRemoved: branchListingDetails.linesRemoved
	}}
	onFirstSeen={() => (hasBeenSeen = true)}
	{onMouseDown}
	{selected}
	{avatars}
/>
