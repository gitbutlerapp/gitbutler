export type LoadableData<T, Id> =
	| { type: 'loading' | 'not-found'; id: Id; error?: undefined }
	| { type: 'found'; id: Id; value: T; error?: undefined }
	| { type: 'error'; id: Id; error: Error };
