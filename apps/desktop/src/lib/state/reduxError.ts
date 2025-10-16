export type ReduxError = { name: string; message: string; code?: string };

export function isReduxError(something: unknown): something is ReduxError {
	return (
		!!something &&
		typeof something === 'object' &&
		something !== null &&
		'message' in something &&
		typeof (something as ReduxError).message === 'string' &&
		('code' in something ? typeof (something as ReduxError).code === 'string' : true)
	);
}
