import { resetPostHog, setPostHogUser } from '$lib/analytics/posthog';
import { resetSentry, setSentryUser } from '$lib/analytics/sentry';
import { getCloudApiClient, type User } from '$lib/backend/cloud';
import { invoke } from '$lib/backend/ipc';
import { sleep } from '$lib/utils/sleep';
import { openExternalUrl } from '$lib/utils/url';
import { BehaviorSubject, Observable, Subject, merge, shareReplay } from 'rxjs';

export class UserService {
	private cloud = getCloudApiClient();

	reset$ = new Subject<User | undefined>();
	loading$ = new BehaviorSubject(false);

	user$ = merge(
		new Observable<User | undefined>((subscriber) => {
			invoke<User | undefined>('get_user').then((user) => {
				if (user) {
					subscriber.next(user);
					setPostHogUser(user);
					setSentryUser(user);
				}
			});
		}),
		this.reset$
	).pipe(shareReplay(1));

	constructor() {}

	async setUser(user: User | undefined) {
		if (user) await invoke('set_user', { user });
		else await this.clearUser();
		this.reset$.next(user);
	}

	async clearUser() {
		await invoke('delete_user');
	}

	async logout() {
		await this.clearUser();
		this.reset$.next(undefined);
		resetPostHog();
		resetSentry();
	}

	async login(): Promise<User | undefined> {
		this.logout();
		this.loading$.next(true);
		try {
			const token = await this.cloud.login.token.create();
			openExternalUrl(token.url);

			// Assumed min time for login flow
			await sleep(4000);

			const user = await this.pollForUser(token.token);
			this.setUser(user);

			return user;
		} finally {
			this.loading$.next(false);
		}
	}

	private async pollForUser(token: string): Promise<User | undefined> {
		let apiUser: User | null;
		for (let i = 0; i < 120; i++) {
			apiUser = await this.cloud.login.user.get(token).catch(() => null);
			if (apiUser) {
				this.setUser(apiUser);
				return apiUser;
			}
			await sleep(1000);
		}
	}
}
