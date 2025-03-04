import { filterWithRest } from '$lib/utils/array';
import { gravatarUrlFromEmail } from '@gitbutler/ui/avatar/gravatar';
import type { Branch } from '$lib/branches/types';
import type { Patch } from '$lib/patches/types';
import type { UserMaybe } from '$lib/users/types';

export type Commenter = {
	avatarUrl?: string;
	email?: string;
	login?: string;
	name?: string;
};

async function getUsersWithAvatarsFromMails(userEmails: string[]) {
	return await Promise.all(
		userEmails.map(async (user) => {
			return {
				srcUrl: await gravatarUrlFromEmail(user),
				name: user
			};
		})
	);
}

export async function getUsersWithAvatars(commenters: Commenter[]) {
	return await Promise.all(
		commenters.map(async (commenter) => {
			const name = commenter.login ?? commenter.email ?? commenter.name ?? 'unknown';
			const email = commenter.email ?? 'unknown';
			return {
				srcUrl: commenter.avatarUrl ?? (await gravatarUrlFromEmail(email)),
				name
			};
		})
	);
}

async function getAvatarsForContributors(contributors: UserMaybe[]) {
	const [userContributors, emailContributors] = filterWithRest(
		contributors,
		(contributor) => !!contributor.user
	);

	return await Promise.all([
		getUsersWithAvatars(userContributors.map((contributor) => contributor.user!)),
		getUsersWithAvatarsFromMails(emailContributors.map((contributor) => contributor.email))
	]).then((result) => result.flat());
}

export async function getContributorsWithAvatars(branch: Branch) {
	return await getAvatarsForContributors(branch.contributors);
}

export async function getPatchContributorsWithAvatars(patch: Patch) {
	return await getAvatarsForContributors(patch.contributors);
}

export async function getPatchApproversWithAvatars(patch: Patch) {
	return await getUsersWithAvatars(patch.review.signedOff);
}

export async function getPatchApproversAllWithAvatars(patch: Patch) {
	return await getUsersWithAvatars(patch.reviewAll.signedOff);
}

export async function getPatchRejectorsWithAvatars(patch: Patch) {
	return await getUsersWithAvatars(patch.review.rejected);
}

export async function getPatchRejectorsAllWithAvatars(patch: Patch) {
	return await getUsersWithAvatars(patch.reviewAll.rejected);
}

export async function getPatchViewersWithAvatars(patch: Patch) {
	return await getUsersWithAvatars(patch.review.viewed);
}

export async function getPatchReviewersAllWithAvatars(patch: Patch) {
	const reviewers = patch.reviewAll.signedOff.concat(patch.reviewAll.rejected);
	return await getUsersWithAvatars(reviewers);
}

export async function getPatchViewersAllWithAvatars(patch: Patch) {
	return await getUsersWithAvatars(patch.reviewAll.viewed);
}
