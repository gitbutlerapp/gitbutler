import { getHead, subscribeToHead } from '$lib/backend/heads';
import { Observable, concat, from } from 'rxjs';

export function getHeads(projectId: string): Observable<string> {
	const sessions$ = from(getHead(projectId));
	const stream$ = new Observable<string>((subscriber) =>
		subscribeToHead(projectId, (head) => subscriber.next(head))
	);
	return concat(sessions$, stream$);
}
