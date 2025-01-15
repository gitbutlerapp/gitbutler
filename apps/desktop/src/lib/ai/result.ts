export class Panic extends Error {}

export type OkVariant<Ok> = {
	ok: true;
	value: Ok;
};

export type FailureVariant<Err> = {
	ok: false;
	failure: Err;
};

export type Result<Ok, Err> = OkVariant<Ok> | FailureVariant<Err>;

export function isOk<Ok, Err>(
	subject: OkVariant<Ok> | FailureVariant<Err>
): subject is OkVariant<Ok> {
	return subject.ok;
}

export function isFailure<Ok, Err>(
	subject: OkVariant<Ok> | FailureVariant<Err>
): subject is FailureVariant<Err> {
	return !subject.ok;
}

export function ok<Ok, Err>(value: Ok): Result<Ok, Err> {
	return { ok: true, value };
}

export function failure<Ok, Err>(value: Err): Result<Ok, Err> {
	return { ok: false, failure: value };
}

export function buildFailureFromAny<Ok>(value: any): Result<Ok, Error> {
	if (value instanceof Error) {
		return failure(value);
	} else {
		return failure(new Error(String(value)));
	}
}

export function wrap<Ok, Err>(subject: () => Ok): Result<Ok, Err> {
	try {
		return ok(subject());
	} catch (e) {
		return failure(e as Err);
	}
}

export async function wrapAsync<Ok, Err>(subject: () => Promise<Ok>): Promise<Result<Ok, Err>> {
	try {
		return ok(await subject());
	} catch (e) {
		return failure(e as Err);
	}
}

export function unwrap<Ok, Err>(subject: Result<Ok, Err>): Ok {
	if (isOk(subject)) {
		return subject.value;
	} else {
		if (subject.failure instanceof Error) {
			throw subject.failure;
		} else {
			throw new Panic(String(subject.failure));
		}
	}
}

export function unwrapOr<Ok, Err, Or>(subject: Result<Ok, Err>, or: Or): Ok | Or {
	if (isOk(subject)) {
		return subject.value;
	} else {
		return or;
	}
}

export function map<Ok, Err, NewOk>(
	subject: Result<Ok, Err>,
	transformation: (ok: Ok) => NewOk
): Result<NewOk, Err> {
	if (isOk(subject)) {
		return ok(transformation(subject.value));
	} else {
		return subject;
	}
}

export function andThen<Ok, Err, NewOk>(
	subject: Result<Ok, Err>,
	transformation: (ok: Ok) => Result<NewOk, Err>
): Result<NewOk, Err> {
	if (isOk(subject)) {
		return transformation(subject.value);
	} else {
		return subject;
	}
}

export async function andThenAsync<Ok, Err, NewOk>(
	subject: Result<Ok, Err>,
	transformation: (ok: Ok) => Promise<Result<NewOk, Err>>
): Promise<Result<NewOk, Err>> {
	if (isOk(subject)) {
		return await transformation(subject.value);
	} else {
		return await Promise.resolve(subject);
	}
}
