export class ApiError extends Error {
	constructor(
		message: string,
		readonly response: Response
	) {
		super(message);
	}
}

export type SerializableError = {
	name: string;
	message: string;
	stack?: string;
};

export function toSerializable(error: unknown): SerializableError {
	if (error instanceof Error) {
		return {
			name: error.name,
			message: error.message,
			stack: error.stack
		};
	}

	return {
		name: 'Unknown error',
		message: String(error)
	};
}

export type Loadable<T> =
	| { status: 'loading' | 'not-found' }
	| { status: 'found'; value: T }
	| { status: 'error'; error: SerializableError };

export type LoadableData<T, Id> = Loadable<T> & { id: Id };
