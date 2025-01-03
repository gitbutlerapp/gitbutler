export class ApiError extends Error {
	constructor(
		message: string,
		readonly response: Response
	) {
		super(message);
	}
}

export type Loadable<T> =
	| { type: 'loading' | 'not-found' }
	| { type: 'found'; value: T }
	| { type: 'error'; error: Error };

export type LoadableData<T, Id> = Loadable<T> & { id: Id };
