export class ApiError extends Error {
	constructor(
		message: string,
		readonly response: Response
	) {
		super(message);
	}
}

export type Loadable<T> =
	| { status: 'loading' | 'not-found' }
	| { status: 'found'; value: T }
	| { status: 'error'; error: Error };

export type LoadableData<T, Id> = Loadable<T> & { id: Id };
