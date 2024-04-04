import { subscribeToFetches } from '$lib/backend/fetches';
import { Observable, shareReplay, startWith } from 'rxjs';

export function getFetchNotifications(projectId: string): Observable<unknown> {
	return new Observable((observer) => subscribeToFetches(projectId, () => observer.next())).pipe(
		startWith(undefined),
		shareReplay(1)
	);
}
