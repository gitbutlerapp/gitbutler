import { subscribeToFetches } from '$lib/backend/fetches';
import { Observable } from 'rxjs';

export function getFetchNotifications(projectId: string): Observable<void> {
	return new Observable((observer) => subscribeToFetches(projectId, () => observer.next()));
}
