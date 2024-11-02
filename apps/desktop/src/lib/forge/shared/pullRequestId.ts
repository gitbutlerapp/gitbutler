import { ForgeName, type PullRequestId } from '$lib/forge/interface/types';

/**
 * TODO: Can this be expressed in a better way?
 *
 * @returns True if both pull request ids are the same.
 */
export function equalPrId(left: PullRequestId, right: PullRequestId): boolean {
	if (left.type !== right.type) {
		return false;
	} else if (left.type === ForgeName.GitHub && right.type === ForgeName.GitHub) {
		return left.subject.prNumber === right.subject.prNumber;
	}
	throw 'Not implemented';
}
