import { subscribeToDeltas, type Delta, listDeltas } from '$lib/backend/deltas';
import { Observable, switchMap, concat, from, scan, shareReplay } from 'rxjs';

export class DeltasService {
	deltas$: Observable<Partial<Record<string, Delta[]>>>;
	constructor(projectId: string, sessionId$: Observable<string>) {
		this.deltas$ = sessionId$.pipe(
			switchMap((sessionId) =>
				concat(
					from(listDeltas({ projectId, sessionId })),
					new Observable<Partial<Record<string, Delta[]>>>((subscriber) =>
						subscribeToDeltas(projectId, sessionId, ({ filePath, deltas }) => {
							const obj: Partial<Record<string, Delta[]>> = {};
							obj[filePath] = deltas;
							subscriber.next(obj);
						})
					)
				)
			),
			scan((acc, curr) => {
				for (const key in curr) {
					acc[key] = [...(acc[key] || []), ...(curr[key] || [])];
				}
				return { ...acc };
			}),
			shareReplay(1)
		);
	}
}
