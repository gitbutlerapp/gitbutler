import type { Readable, Writable } from 'svelte/store';
import { Observable } from 'rxjs';

export function storeToObservable<T>(svelteStore: Writable<T> | Readable<T>): Observable<T> {
	return new Observable<T>((subscriber) => {
		return svelteStore.subscribe((val) => subscriber.next(val));
	});
}
