export class ExternallyResolvedPromise<T> {
	resolve!: T extends undefined ? (value?: T) => void : (value: T) => void;
	reject!: (error: unknown) => void;
	promise: Promise<T>;

	constructor() {
		this.promise = new Promise<T>(
			((resolve: (value: T) => void, reject: (error: unknown) => void) => {
				this.resolve = resolve as T extends undefined ? (value?: T) => void : (value: T) => void;
				this.reject = reject;
			}).bind(this)
		);
	}
}
