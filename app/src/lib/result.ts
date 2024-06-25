export type OkVariant<Ok> = {
	ok: true;
	value: Ok;
};

export type ErrorVariant<Error> = {
	ok: false;
	error: Error;
};

export type Result<Ok, Error> = OkVariant<Ok> | ErrorVariant<Error>;

export function isOk<Ok, Error>(subject: Result<Ok, Error>): subject is OkVariant<Ok> {
	return subject.ok;
}

export function isError<Ok, Error>(subject: Result<Ok, Error>): subject is ErrorVariant<Error> {
	return !subject.ok;
}

export function unwrap<Ok, Error>(subject: Result<Ok, Error>): Ok {
	if (isOk(subject)) {
		return subject.value;
	} else {
		throw new Error(String(subject.error));
	}
}

export function unwrapOr<Ok, Error, Or>(subject: Result<Ok, Error>, or: Or): Ok | Or {
	if (isOk(subject)) {
		return subject.value;
	} else {
		return or;
	}
}

export function ok<Ok, Error>(value: Ok): Result<Ok, Error> {
	return { ok: true, value };
}

export function err<Ok, Error>(value: Error): Result<Ok, Error> {
	return { ok: false, error: value };
}
