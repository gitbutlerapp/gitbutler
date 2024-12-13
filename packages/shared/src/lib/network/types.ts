export class ApiError extends Error {
	constructor(
		message: string,
		readonly response: Response
	) {
		super(message);
	}
}
