<script lang="ts">
	import { Project } from '$lib/backend/projects';
	import {
		BranchListingDetails,
		BranchListingService,
		type BranchListing
	} from '$lib/branches/branchListing';
	import { GitConfigService } from '$lib/config/gitConfigService';
	import { getForgeListingService } from '$lib/forge/interface/forgeListingService';
	import { UserService } from '$lib/user/userService';
	import { getContext } from '@gitbutler/shared/context';
	import SidebarEntry from '@gitbutler/ui/SidebarEntry.svelte';
	import { gravatarUrlFromEmail } from '@gitbutler/ui/avatar/gravatar';
	import type { Readable } from 'svelte/store';
	import { goto } from '$app/navigation';
	import { page } from '$app/stores';

	interface Props {
		branchListing: BranchListing;
	}

	const { branchListing }: Props = $props();

	const unknownName = 'unknown';
	const unknownEmail = 'example@example.com';

	const branchListingService = getContext(BranchListingService);
	const project = getContext(Project);
	const gitConfigService = getContext(GitConfigService);

	const forgeListingService = getForgeListingService();
	const prs = $derived($forgeListingService?.prs);
	const pr = $derived($prs?.find((pr) => branchListing.containsPullRequestBranch(pr.sourceBranch)));

	const userService = getContext(UserService);
	const user = userService.user;

	let branchListingDetails = $state<Readable<BranchListingDetails | undefined>>();

	let hasBeenSeen = $state(false);

	$effect(() => {
		if (hasBeenSeen) {
			updateBranchListingDetails(branchListing.name);
		}
	});

	function updateBranchListingDetails(branchName: string) {
		branchListingDetails = branchListingService.getBranchListingDetails(branchName);
	}

	function onMouseDown() {
		if (branchListing.virtualBranch?.inWorkspace) {
			goto(`/${project.id}/board`);
		} else {
			goto(formatBranchURL(project, branchListing.name));
		}
	}

	const selected = $derived($page.url.pathname === formatBranchURL(project, branchListing.name));

	function formatBranchURL(project: Project, name: string) {
		return `/${project.id}/branch/${encodeURIComponent(name)}`;
	}

	// If there are zero commits we should not show the author
	const ownedByUser = $derived($branchListingDetails?.numberOfCommits === 0);

	let lastCommitDetails = $state<{ authorName: string; lastCommitAt?: Date }>();

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
				lastCommitAt: branchListing.updatedAt
			};
		}
	});

	let avatars = $state<{ name: string; srcUrl: string }[]>([]);

	$effect(() => {
		setAvatars(ownedByUser, $branchListingDetails);
	});

	async function setAvatars(ownedByUser: boolean, branchListingDetails?: BranchListingDetails) {
		if (ownedByUser) {
			const name = (await gitConfigService.get('user.name')) || unknownName;
			const email = (await gitConfigService.get('user.email')) || unknownEmail;
			const srcUrl =
				email.toLowerCase() === $user?.email?.toLowerCase()
					? $user?.picture
					: await gravatarUrlFromEmail(email);

			avatars = [{ name, srcUrl }];
		} else if (branchListingDetails) {
			avatars = await Promise.all(
				branchListingDetails.authors.map(async (author) => {
					return {
						name: author.name || unknownName,
						srcUrl:
							(author.email?.toLowerCase() === $user?.email?.toLowerCase()
								? $user?.picture
								: author.gravatarUrl) ?? (await gravatarUrlFromEmail(author.email || unknownEmail))
					};
				})
			);
		} else {
			avatars = [];
		}
	}

	const stackBranches = $derived(branchListing.virtualBranch?.stackBranches);
	const filteredStackBranches = $derived(
		stackBranches && stackBranches.length > 0 ? stackBranches : [branchListing.name]
	);
</script>

<SidebarEntry
	series={filteredStackBranches}
	remotes={branchListing.remotes}
	local={branchListing.hasLocal}
	applied={branchListing.virtualBranch?.inWorkspace}
	{lastCommitDetails}
	pullRequestDetails={pr && {
		title: pr.title,
		draft: pr.draft
	}}
	branchDetails={$branchListingDetails && {
		commitCount: $branchListingDetails.numberOfCommits,
		linesAdded: $branchListingDetails.linesAdded,
		linesRemoved: $branchListingDetails.linesRemoved
	}}
	onFirstSeen={() => (hasBeenSeen = true)}
	{onMouseDown}
	{selected}
	{avatars}
/>
