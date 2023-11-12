import type { Readable, Writable, Unsubscriber } from 'svelte/store';
import { Observable } from 'rxjs';

export function storeToObservable<T>(svelteStore: Writable<T> | Readable<T>): Observable<T> {
	let unsubscribe: Unsubscriber;
	const observable = new Observable<T>((subscriber) => {
		unsubscribe = svelteStore.subscribe((val) => {
			subscriber.next(val);
		});
		return () => {
			unsubscribe();
		};
	});
	return observable;
}
