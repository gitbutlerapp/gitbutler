import { Observable, Subscription, catchError, of } from 'rxjs';
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
	let subscription: Subscription | undefined = undefined;

	const store = writable<T | undefined>(undefined, () => {
		// This runs when the last subscriber unsubscribes
		return () => {
			// TODO: Investigate why project switching breaks without `setTimeout`
			setTimeout(() => {
				if (subscription?.closed) unsubscribe();
			}, 0);
		};
	});
	const error = writable<string>();

	subscription = observable
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
