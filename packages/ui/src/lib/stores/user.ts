import type { User } from '$lib/backend/cloud';
import * as users from '$lib/backend/users';
import { BehaviorSubject, Observable, merge, shareReplay } from 'rxjs';

export class UserService {
	reset$ = new BehaviorSubject<undefined | User>(undefined);
	user$ = merge(
		new Observable<User | undefined>((subscriber) => {
			users.get().then((user) => {
				if (user) {
					subscriber.next(user);
					this.posthog.then((client) => client.identify(user));
					this.sentry.identify(user);
				}
			});
		}).pipe(shareReplay(1)),
		this.reset$
	);

	constructor(
		private sentry: any,
		private posthog: Promise<any>
	) {}

	async set(user: User) {
		await users.set({ user });
		this.reset$.next(user);
	}

	async logout() {
		await users.delete();
		this.reset$.next(undefined);
		// TODO: Un-identify from sentry and posthog
	}
}
