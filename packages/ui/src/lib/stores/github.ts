import type { User } from '$lib/backend/cloud';
import type { GitHubIntegrationContext } from '$lib/github/types';
import type { BaseBranch } from '$lib/vbranches/types';
import { combineLatest, switchMap, type Observable, of, shareReplay, distinct } from 'rxjs';

export function getGithubContext(
	user$: Observable<User | undefined>,
	baseBranch$: Observable<BaseBranch | null>
): Observable<GitHubIntegrationContext | undefined> {
	const distinctUrl$ = baseBranch$.pipe(distinct((ctx) => ctx?.remoteUrl));
	return combineLatest([user$, distinctUrl$]).pipe(
		switchMap(([user, baseBranch]) => {
			const remoteUrl = baseBranch?.remoteUrl;
			const authToken = user?.github_access_token;
			const username = user?.github_username || '';
			if (!remoteUrl || !remoteUrl.includes('github') || !authToken) return of();

			const [owner, repo] = remoteUrl.split('.git')[0].split(/\/|:/).slice(-2);
			return of({ authToken, owner, repo, username });
		}),
		shareReplay(1)
	);
}
