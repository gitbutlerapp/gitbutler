import { Observable, catchError, of } from 'rxjs';
import { writable, type Readable, type Writable } from 'svelte/store';

export function storeToObservable<T>(svelteStore: Writable<T> | Readable<T>): Observable<T> {
	return new Observable<T>((subscriber) => {
		return svelteStore.subscribe((val) => subscriber.next(val));
	});
}

export function observableToStore<T>(
	observable: Observable<T>
): [Readable<T | undefined>, Readable<string | undefined>] {
	let unsubscribe: any = undefined;

	const store = writable<T | undefined>(undefined, () => unsubscribe);
	const error = writable<string | undefined>();

	const subscription = observable
		.pipe(
			catchError((e: any) => {
				error.set(e.message);
				return of(undefined);
			})
		)
		.subscribe((item) => {
			store.set(item);
		});
	unsubscribe = subscription.unsubscribe;
	return [store, error];
}
