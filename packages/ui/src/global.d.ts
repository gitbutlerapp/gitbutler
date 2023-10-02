// Back end error comes with an error code
// See: https://www.typescriptlang.org/docs/handbook/declaration-merging.html
declare interface Error {
	code: string;
}
