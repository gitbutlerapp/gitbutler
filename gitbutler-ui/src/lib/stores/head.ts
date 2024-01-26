import { getHead, subscribeToHead } from '$lib/backend/heads';
import { Observable, concat, distinctUntilChanged, from, shareReplay } from 'rxjs';

export function getHeads(projectId: string): Observable<string> {
	const head$ = from(getHead(projectId));
	const stream$ = new Observable<string>((subscriber) =>
		subscribeToHead(projectId, (head) => subscriber.next(head))
	);
	return concat(head$, stream$).pipe(distinctUntilChanged(), shareReplay(1));
}
