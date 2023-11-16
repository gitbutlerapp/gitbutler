import { Session, listSessions, subscribeToSessions } from '$lib/backend/sessions';
import { Observable, from, concat, shareReplay } from 'rxjs';

export function getSessions(projectId: string): Observable<Session[]> {
	return concat(
		from(listSessions(projectId)),
		new Observable<Session[]>((subscriber) => {
			return subscribeToSessions(projectId, (session) =>
				subscriber.next([{ projectId, ...session }])
			);
		})
	).pipe(shareReplay(1));
}
