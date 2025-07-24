import { setContext, getContext as svelteGetContext } from 'svelte';

/**
 * Angular inspired injection token.
 *
 * @example
 * const STACK_SERVICE = new InjectionToken<StackService>('StackService');
 * provide(STACK_SERVICE, stackService);
 * const stackService = inject(STACK_SERVICE); // of type `StackService`
 */
export class InjectionToken<_T> {
	private readonly _desc: string;
	private readonly _symbol: symbol;

	constructor(desc: string) {
		this._desc = desc;
		this._symbol = Symbol(desc);
	}

	get description(): string {
		return this._desc;
	}

	toString(): string {
		return `InjectionToken(${this._desc})`;
	}

	get _key(): symbol {
		return this._symbol;
	}
}

/**
 * Provides a value for an injection token
 */
export function provide<T>(token: InjectionToken<T>, value: T): void {
	setContext(token._key, value);
}

/**
 * An injector for use with `InjectionToken` rather than `Constructor`.
 */
export function inject<T>(token: InjectionToken<T>): T {
	const value = svelteGetContext<T>(token._key);
	if (value === undefined) {
		throw new Error(`No provider found for ${token.toString()}`);
	}
	return value;
}

/**
 * Injects a value using an injection token with a fallback
 * Returns the default value if the token is not found
 */
export function injectOptional<T>(token: InjectionToken<T>, defaultValue: T): T {
	const value = svelteGetContext<T>(token._key);
	return value !== undefined ? value : defaultValue;
}
