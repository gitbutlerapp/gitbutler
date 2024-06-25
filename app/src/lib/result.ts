export type OkVariant<Ok> = {
	ok: true;
	value: Ok;
};

export type FailureVariant<Err extends Error = Error> = {
	ok: false;
	failure: Err;
};

export type Result<Ok, Err extends Error = Error> = OkVariant<Ok> | FailureVariant<Err>;

export function isOk<Ok>(subject: Result<Ok>): subject is OkVariant<Ok> {
	return subject.ok;
}

export function isFailure<Ok>(subject: Result<Ok>): subject is FailureVariant {
	return !subject.ok;
}

export function unwrap<Ok>(subject: Result<Ok>): Ok {
	if (isOk(subject)) {
		return subject.value;
	} else {
		throw subject.failure;
	}
}

export function unwrapOr<Ok, Or>(subject: Result<Ok>, or: Or): Ok | Or {
	if (isOk(subject)) {
		return subject.value;
	} else {
		return or;
	}
}

export function ok<Ok>(value: Ok): Result<Ok> {
	return { ok: true, value };
}

export function failure<Ok>(value: any): Result<Ok> {
	if (value instanceof Error) {
		return { ok: false, failure: value };
	} else {
		return { ok: false, failure: new Error(value) };
	}
}
